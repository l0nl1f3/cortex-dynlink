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

/// For a given object file, and its public functions,
/// generate a binary image that can be parsed by dl-lib
/// The binary has the following layout, numbers have width=4 and are in little-endian order
///
/// ---
/// num_global_functions, num_tables, num_relocs, raw_symbol_table_length
/// code section length, data section length, bss section length
/// func1's index in symbol table
/// func2's index in symbol table
/// ...
/// reloc1 offset, reloc1 index in symbol table
/// reloc2 offset, reloc2 index in symbol table
/// ...
/// symbol1 index in symbol names, symbol1 address
/// symbol2 index in symbol names, symbol2 address
/// ...
/// symbol names = symbol1.name 0 symbol2.name 0 ...
/// data section
/// code section
/// bss section
/// ---
fn make_image(obj: &String, glb_funcs: Vec<String>) -> Result<Vec<u8>, Box<dyn Error>> {
    let bin_data = fs::read(obj)?;
    let obj_file = object::File::parse(&*bin_data)?;
    let code_section = obj_file.section_by_name(".text").unwrap().data()?;
    let data_section = obj_file.section_by_name(".data").unwrap().data()?;
    let bss_section = obj_file.section_by_name(".bss").unwrap().data()?;
    let filtered_symbols: Vec<_> = obj_file
        .symbols()
        .filter(|s| {
            let name = s.name().unwrap();
            !name.is_empty()
                && !name.starts_with("$t")
                && !name.starts_with("$d")
                && !name.ends_with("module")
        })
        .collect();

    let mut type_by_name: HashMap<String, SymbolType> = HashMap::new();
    let mut section_by_name: HashMap<String, SectionIndex> = HashMap::new();
    let mut address_by_name: HashMap<String, u64> = HashMap::new();

    // get symbol type (Exported, External, Local, None), section index, and address
    for symbol in filtered_symbols {
        let name = String::from(symbol.name().unwrap());
        let symbol_type = match (symbol.is_global(), symbol.is_undefined(), symbol.kind()) {
            (true, false, _) => Some(SymbolType::Exported),
            (true, _, _) => Some(SymbolType::External),
            (_, _, object::SymbolKind::File) => None,
            (_, _, _) => Some(SymbolType::Local),
        };
        if let Some(symbol_type) = symbol_type {
            type_by_name.insert(name.clone(), symbol_type);
            section_by_name.insert(name.clone(), symbol.section_index().unwrap());
            address_by_name.insert(name.clone(), symbol.address());
        }
    }

    // switch to low-level read api, something not right in the unified read.
    let vec_relocations = relocations::get_known_relocations(obj).unwrap();
    let (variables, functions): (_, Vec<_>) = vec_relocations.into_iter().partition(|x| {
        if let RelocationType::MOVT_BREL | RelocationType::MOVW_BREL_NC = x.r_type {
            true
        } else {
            false
        }
    });
    let var_reloc_names: HashSet<_> = variables.iter().map(|var| var.name.clone()).collect();
    let func_reloc_names: HashSet<_> = functions.iter().map(|func| func.name.clone()).collect();
    let num_table = var_reloc_names.len();
    let num_relocs = num_table + functions.len();

    let mut image: Vec<u8> = Vec::new();
    image.extend(&glb_funcs.len().to_le_bytes()[0..4]);
    image.extend(&num_table.to_le_bytes()[0..4]);
    image.extend(&num_relocs.to_le_bytes()[0..4]);

    let mut sym_names: Vec<String> = Vec::new();
    sym_names.push(String::from("module"));
    // Symbol Table: process names
    for (k, v) in &type_by_name {
        // exclude unused local symbol
        // only used local symbols and external/exported symbols needs further processing
        if var_reloc_names.contains(k) || func_reloc_names.contains(k) {
            sym_names.push(k.clone());
        } else if let SymbolType::External | SymbolType::Exported = v {
            sym_names.push(k.clone());
        }
    }

    let flat_sym_names: Vec<_> = sym_names
        .iter()
        .flat_map(|name| {
            if let Some(SymbolType::External) | Some(SymbolType::Exported) | None =
                type_by_name.get(name)
            {
                format!("{}\0", name).as_bytes().to_vec()
            } else {
                "".as_bytes().to_vec()
            }
        })
        .collect();
    let sym_table_len = sym_names.len() * 8 + flat_sym_names.len();

    let sym_table_idx: HashMap<String, u32> = sym_names
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx as u32))
        .collect();

    image.extend(&sym_table_len.to_le_bytes()[0..4]);
    image.extend(&code_section.len().to_le_bytes()[0..4]);
    image.extend(&data_section.len().to_le_bytes()[0..4]);
    image.extend(&bss_section.len().to_le_bytes()[0..4]);
    image.extend(&sym_names.len().to_le_bytes()[0..4]);

    let mut hash_set: HashSet<String> = HashSet::new();
    let vec_relocations = relocations::get_known_relocations(obj).unwrap();
    // Write Relocation table
    let mut num_table = 0;
    for reloc in &vec_relocations {
        let offset;
        match reloc.r_type {
            RelocationType::ABS32 => {
                offset = reloc.r_offset;
            }
            RelocationType::MOVT_BREL => {
                offset = num_table;
                num_table += 1;
                if hash_set.contains(&reloc.name) {
                    continue;
                }
                hash_set.insert(reloc.name.clone());
            }
            _ => {
                continue;
            }
        }
        let idx = sym_table_idx.get(&reloc.name).unwrap();
        image.extend(&offset.to_le_bytes()[0..4]);
        image.extend(&idx.to_le_bytes()[0..4]);
    }
    // Write every global function's index
    image.extend(
        glb_funcs
            .iter()
            .flat_map(|name| sym_table_idx.get(name).unwrap().to_le_bytes())
            .collect::<Vec<_>>(),
    );

    let mut sym_table_len = 0;
    // Write Symbol table
    for (i, sym) in sym_names.iter().enumerate() {
        if i > 0 {
            let mut type_data = match type_by_name[sym] {
                SymbolType::Local => 0,
                SymbolType::Exported => 1,
                SymbolType::External => 2,
            };
            let mut address = address_by_name[sym];
            let mut address_offset = code_section.len() as u64;
            if type_data == 2 {
                address_offset = 0;
            }
            // if sym is a function
            if let Some(SectionIndex(1)) = section_by_name.get(sym) {
                type_data += 4;
                address_offset = 0;
            }
            address -= address_offset;
            let x = (type_data << 28) | (sym_table_len as u32);
            if (type_data & 3) == 0 {
                address = 0;
            } else {
                sym_table_len += sym.len() + 1;
            }
            image.extend(&x.to_le_bytes()[0..4]);
            image.extend(&address.to_le_bytes()[0..4]);
        } else {
            // module name, reserved
            let x = (3 << 28) | (sym_table_len);
            let add = 0i32;
            image.extend(&x.to_le_bytes()[0..4]);
            image.extend(&add.to_le_bytes()[0..4]);
            sym_table_len += sym.len() + 1;
        }
    }

    image.extend(flat_sym_names);

    if image.len() % 4 != 0 {
        image.extend(vec![0; 4 - image.len() % 4]);
    }
    image.extend(code_section);
    image.extend(data_section);
    image.extend(bss_section);
    // Write image to specific file

    Ok(image)
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

    let image = make_image(&String::from("out.elf"), glb_funcs).unwrap();
    // handling results
    let mut file = fs::File::create("../dl-lib/src/lib/binary.rs").expect("Open binary.rs failed");

    file.write_fmt(format_args!(
        "pub static BUF: [u8; {}] = {:?};\n",
        image.len(),
        image
    ))
    .expect("Write binary.rs failed");
    println!("{:?}", image);
}
