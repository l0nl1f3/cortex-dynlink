mod utils;
use utils::{literals, readelf, relocations};
use utils::{relocations::RelocationType, symbols::SymbolType};

use md5;
use object;
use object::{Object, ObjectSection, ObjectSymbol, SectionIndex};
use std::collections::{HashMap, HashSet};
use std::{error::Error, fs, io::Write, process::Command};

// use crate::{TEST, TEST2, TEST3};

fn trampoline_entry_name(func: &str) -> String {
    let name_prefix = format!("{:x}", md5::compute(func.as_bytes()));
    let name_prefix_8 = name_prefix.chars().take(8).collect::<String>();
    format!("__{}__{}", name_prefix_8, func)
}

/// For a given object file, for each contained public function,
/// generate a trampoline such that R9 will be updated before calling
/// into the actual function.
///
/// The trampolines have the following layout:
/// __hash1_func1:
///     push    {r9, lr}
///     movw    r11, #0  // r11 will hold func1's runtime address
///     movt    r11, #0
///     b       common_trampoline
/// __hash2_func2:
///     push    {r9, lr}
///     movw    r11, #0
///     movt    r11, #0
///     b       common_trampoline
/// __hash3__func3:
///     ...
/// common_trampoline:
///     movw    r9, #0    // switch R9
///     movt    r9, #0
///     blx     r11       // call into function
///     pop     {r9, pc}
fn compile_trampoline(obj_path: &str, module_name: &str) {
    let pub_funcs = readelf::get_pub_funcs(obj_path).unwrap();

    let func_trampolines = pub_funcs.iter().fold(String::new(), |mut folded, func| {
        folded.push_str(&format!(
            crate::FUNPRE!(),
            s = trampoline_entry_name(func),
            modulename = trampoline_entry_name(module_name)
        ));
        folded
    });

    let common_trampoline = format!(crate::OBJPRE!(), s = trampoline_entry_name(module_name));

    let asm = format!(
        "{}{}{}{}",
        literals::ASM_HEAD,
        func_trampolines,
        common_trampoline,
        literals::ASM_TAIL
    );

    fs::write("asm.s", asm).unwrap();

    // TODO: change _pre
    let trampo_path = obj_path.replace(".o", "_pre.o");

    let assemble_cmd = format!(crate::ASM_CMD!(), asm = "asm.s", elf = trampo_path);

    // Invoke compiler to compile the generated asm file into an object file.
    let output = Command::new("bash")
        .arg("-c")
        .arg(assemble_cmd)
        .output()
        .unwrap();
    println!("ASM: {:?}", output);
    if !output.status.success() {
        panic!("Assembler failed!");
    }
}

// link interposition and original objects
fn link_objects(objs: &Vec<String>) {
    let input = objs.join(" ");
    let output = "out.elf";
    let link_cmd = format!(crate::LINK_CMD!(), input = input, output = output);

    let output = Command::new("bash")
        .arg("-c")
        .arg(link_cmd)
        .output()
        .unwrap();
    println!("LNK: {:?}", output);
    if !output.status.success() {
        panic!("Linker failed!");
    }
}

// parse the linked object into dynamic loadable image
fn process_binary(obj: &String, g_funcs: Vec<String>) -> Result<(), Box<dyn Error>> {
    let bin_data = fs::read(obj)?;
    let obj_file = object::File::parse(&*bin_data)?;
    let code_section = obj_file.section_by_name(".text").unwrap().data()?;
    let data_section = obj_file.section_by_name(".data").unwrap().data()?;
    let bss_section = obj_file.section_by_name(".bss").unwrap().data()?;
    let symbols = obj_file.symbols();

    let mut symbol_types: HashMap<String, SymbolType> = HashMap::new();
    let mut symbol_sections: HashMap<String, SectionIndex> = HashMap::new();
    let mut symbol_addresses: HashMap<String, u64> = HashMap::new();

    // get symbol type (Exported, External, Local, None), section index, and address
    for symbol in symbols {
        let name = String::from(symbol.name().unwrap());
        if name.is_empty() // exclude symbol associate with section (STT_SECTION)
            || name.starts_with("$t")
            || name.starts_with("$d")  
            || name.ends_with("module")
        // exclude symbol "module"
        {
            continue;
        }
        let symbol_type = match (symbol.is_global(), symbol.is_undefined(), symbol.kind()) {
            (true, false, _) => Some(SymbolType::Exported),
            (true, _, _) => Some(SymbolType::External),
            (_, _, object::SymbolKind::File) => None,
            (_, _, _) => Some(SymbolType::Local),
        };
        if let Some(symbol_type) = symbol_type {
            symbol_types.insert(name.clone(), symbol_type);
        }
        if let Some(section_index) = symbol.section_index() {
            symbol_sections.insert(name.clone(), section_index);
        }
        symbol_addresses.insert(name.clone(), symbol.address());
    }

    // switch to low-level read api, something not right in the unified read.
    let vec_relocations: Vec<relocations::Relocation> = relocations::get_relocations(obj).unwrap();
    let mut names: HashMap<String, u32> = HashMap::new(); // symbol name -> symbol index in relocation table
    let mut num_relocs = 0;
    // Relocation Table: process global variables
    for reloc in &vec_relocations {
        if let RelocationType::MOVT_BREL | RelocationType::MOVW_BREL_NC = reloc.r_type {
            // global variable: multiple pair of MOVW and MOVT, only keep one
            if names.contains_key(&reloc.name) {
                continue;
            }
            names.insert(reloc.name.clone(), num_relocs);
            num_relocs += 1;
        }
    }
    let num_table = num_relocs;
    // Relocation Table: process global functions
    for reloc in &vec_relocations {
        if let RelocationType::ABS32 = reloc.r_type {
            // function call: 1 ABS32, keep all
            let offset = reloc.r_offset;
            names.insert(reloc.name.clone(), offset.into());
            num_relocs += 1;
        }
    }

    let mut image: Vec<u8> = Vec::new();
    // number of global functions
    image.extend(&(g_funcs.len()).to_le_bytes());
    // number of got entries
    image.extend(&num_table.to_le_bytes());
    // number of relocations
    image.extend(&num_relocs.to_le_bytes());

    let mut sym_table: Vec<String> = Vec::new();

    sym_table.push(String::from("module"));
    // Symbol Table: process names
    for (k, v) in &symbol_types {
        // exclude unused local symbol
        // only used local symbols and external/exported symbols needs further processing
        if names.contains_key(k) {
            sym_table.push(k.clone());
            continue;
        }
        if let SymbolType::External | SymbolType::Exported = v {
            sym_table.push(k.clone());
        }
    }

    let mut sym_table_len = sym_table.len() * 8;
    let mut sym_table_idx: HashMap<String, i32> = HashMap::new(); // symbol name -> index in Symbol Names
                                                                  // calculate Symbol Table length
    for (i, sym) in sym_table.iter().enumerate() {
        if i == 0 {
            // module name, always="module" now, reserved
            sym_table_len += sym.len() + 1;
            continue;
        }
        if let SymbolType::External | SymbolType::Exported = symbol_types[sym] {
            sym_table_len += sym.len() + 1;
        }
        sym_table_idx.insert(sym.clone(), i as i32);
    }
    // align to 4 bytes
    sym_table_len += 3;
    sym_table_len -= sym_table_len % 4;

    // raw symbol tabel size
    image.extend(&sym_table_len.to_le_bytes());
    image.extend(&code_section.len().to_le_bytes());
    image.extend(&data_section.len().to_le_bytes());
    image.extend(&bss_section.len().to_le_bytes());
    // symbol number
    image.extend(&sym_table.len().to_le_bytes());

    let mut hash_set: HashSet<String> = HashSet::new();
    // Write Relocation table
    for reloc in &vec_relocations {
        let p;
        match reloc.r_type {
            RelocationType::ABS32 => {
                // p = where to store function address
                p = reloc.r_offset;
            }
            RelocationType::MOVT_BREL => {
                // p = index in data section
                p = *names.get(&reloc.name).unwrap();
                if hash_set.contains(&reloc.name) {
                    continue;
                }
                hash_set.insert(reloc.name.clone());
            }
            _ => {
                continue;
            }
        }
        // q = index in Symbol Names
        let q = sym_table_idx.get(&reloc.name).unwrap();
        image.extend(&p.to_le_bytes()[0..4]);
        image.extend(&q.to_le_bytes()[0..4]);
    }
    // Write every global function's index
    image.extend(
        g_funcs
            .iter()
            .flat_map(|x| sym_table_idx.get(x).unwrap().to_le_bytes())
            .collect::<Vec<_>>(),
    );

    sym_table_len = 0;
    // Write Symbol table
    for (i, sym) in sym_table.iter().enumerate() {
        if i > 0 {
            let mut type_data = match symbol_types[sym] {
                SymbolType::Local => 0,
                SymbolType::Exported => 1,
                SymbolType::External => 2,
            };
            let mut add = symbol_addresses[sym] as usize;
            let mut off = code_section.len();
            if type_data == 2 {
                off = 0;
            }
            // if sym is a function
            if let Some(SectionIndex(1)) = symbol_sections.get(sym) {
                type_data += 4;
                off = 0;
            }
            add -= off;
            let x = (type_data << 28) | (sym_table_len as u32);
            if (type_data & 3) == 0 {
                add = 0;
            } else {
                sym_table_len += sym.len() + 1;
            }
            // x = type_data<<28 | (index in Symbol Names)
            // if sym is a function, add=where to store function address
            // if sym is a variable, add=index in code section=symbol_address-len(code_section)
            // if sym is external, add=0
            image.extend(&x.to_le_bytes());
            image.extend(&add.to_le_bytes());
        } else {
            // module name, reserved
            let x = (3 << 28) | (sym_table_len);
            let add = 0i32;
            image.extend(&x.to_le_bytes());
            image.extend(&add.to_le_bytes());
            sym_table_len += sym.len() + 1;
        }
    }

    let sym_names: Vec<_> = sym_table
        .iter()
        .flat_map(|sym| {
            if let Some(SymbolType::External) | Some(SymbolType::Exported) | None =
                symbol_types.get(sym)
            {
                format!("{}{}", sym, " ").as_bytes().to_vec()
            } else {
                "".as_bytes().to_vec()
            }
        })
        .collect();
    image.extend(sym_names);

    if image.len() % 4 != 0 {
        image.extend(&vec![0; 4 - image.len() % 4]);
    }
    image.extend(code_section);
    image.extend(data_section);
    // Write image to specific file
    let mut file = fs::File::create("../dl-lib/src/lib/binary.rs")?;

    file.write_fmt(format_args!(
        "pub static BUF: [u8; {}] = {:?};\n",
        image.len(),
        image
    ))?;
    println!("{:?}", image);
    Ok(())
}

// Statically link the raw_objects[] into single dynamic library.
fn main() {
    let input_obj_paths: Vec<String> = vec![String::from("module.o")];

    // Compile trampoline for each input object file.
    for path in &input_obj_paths {
        // TODO: change fixed "module"
        compile_trampoline(path, "module");
    }

    let glb_funcs: Vec<_> = input_obj_paths
        .iter()
        .flat_map(|path| readelf::get_pub_funcs(path).unwrap())
        .collect();

    let trampoline_paths: Vec<_> = input_obj_paths
        .iter()
        .map(|path| path.replace(".o", "_pre.o"))
        .collect();

    let mut linker_input_path = input_obj_paths;
    linker_input_path.extend(trampoline_paths.into_iter());

    link_objects(&linker_input_path);
    process_binary(&String::from("out.elf"), glb_funcs).unwrap();
}
