mod utils;
use utils::{literals, readelf, relocations, symbols};
use utils::{relocations::RelocationType, symbols::SymbolType};

use md5;
use object;
use object::{Object, ObjectSection, ObjectSymbol, SectionIndex};
use std::collections::{HashMap, HashSet};
use std::{error::Error, fs, io::Write, process::Command};

fn wrap_name(func: &str) -> String {
    let name_prefix = format!("{:x}", md5::compute(func.as_bytes()));
    let name_prefix_8 = name_prefix.chars().take(8).collect::<String>();
    format!("__{}__{}", name_prefix_8, func)
}

// Generate interposition: asm.s -> $obj_pre.o
fn add_prefix(obj: &String) -> Vec<String> {
    let module_name = "module"; //
    let pub_funcs = readelf::get_pub_funcs(&obj).unwrap();
    // println!("{:?}", pub_funcs);
    let mut asm_repeat_body = String::from("");
    for func in &pub_funcs {
        let mut repeat = String::from(literals::FUNPRE);
        repeat = repeat
            .replace("{s}", &wrap_name(func))
            .replace("{modulename}", &wrap_name(module_name));
        asm_repeat_body.push_str(&repeat);
    }

    let mut obj_pre = String::from(literals::OBJPRE);
    obj_pre = obj_pre.replace("{s}", &wrap_name(module_name));

    let asm = format!(
        "{}{}{}{}",
        literals::ASM_PRE,
        asm_repeat_body,
        obj_pre,
        literals::ASM_SUF
    );
    fs::write("asm.s", asm).unwrap();
    let obj_2 = obj.replace(".o", "_pre.o");
    let mut assemble_cmd = String::from(literals::ASM_CMD);
    assemble_cmd = assemble_cmd
        .replace("{asm}", "asm.s")
        .replace("{elf}", &obj_2);

    let output = Command::new("bash")
        .arg("-c")
        .arg(assemble_cmd)
        .output()
        .unwrap();
    if !output.status.success() {
        panic!("Assembler failed!");
    }
    println!("ASM: {:?}", output);
    pub_funcs
}

// link interposition and original objects
fn link_objects(objs: &Vec<String>) {
    let input = objs.join(" ");
    let output = "out.elf";
    let mut link_cmd = String::from(literals::LINK_CMD);
    link_cmd = link_cmd.replace("{input}", &input);
    link_cmd = link_cmd.replace("{output}", &output);
    // println!("{:?}", link_cmd);
    let output = Command::new("bash")
        .arg("-c")
        .arg(link_cmd)
        .output()
        .unwrap();
    if !output.status.success() {
        panic!("Linker failed!");
    }
    println!("LNK: {:?}", output);
}

// parse the linked object into dynamic loadable image
fn process_binary(obj: &String, g_funcs: Vec<String>) -> Result<(), Box<dyn Error>> {
    let bin_data = fs::read(obj)?;
    let obj_file = object::File::parse(&*bin_data)?;
    let code_section = obj_file.section_by_name(".text").unwrap();
    let data_section = obj_file.section_by_name(".data").unwrap();
    let bss_section = obj_file.section_by_name(".bss").unwrap();
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
            || name.ends_with("module") // exclude symbol "module" 
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
    image.extend_from_slice(&(g_funcs.len()).to_le_bytes()[0..4]);
    // number of got entries
    image.extend_from_slice(&num_table.to_le_bytes()[0..4]);
    // number of relocations
    image.extend_from_slice(&num_relocs.to_le_bytes()[0..4]);

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
    image.extend_from_slice(&sym_table_len.to_le_bytes()[0..4]); 
    image.extend_from_slice(&code_section.data().unwrap().len().to_le_bytes()[0..4]); 
    image.extend_from_slice(&data_section.data().unwrap().len().to_le_bytes()[0..4]); 
    image.extend_from_slice(&bss_section.data().unwrap().len().to_le_bytes()[0..4]); 
    // symbol number
    image.extend_from_slice(&sym_table.len().to_le_bytes()[0..4]);

    let mut hash_set: HashSet<String> = HashSet::new();
    // Write Relocation table
    for reloc in &vec_relocations {
        let mut p = 0;
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
        image.extend_from_slice(&p.to_le_bytes()[0..4]);
        image.extend_from_slice(&q.to_le_bytes()[0..4]);
    }
    // Write every global function's index
    for func in g_funcs {
        let idx = sym_table_idx.get(&func).unwrap();
        image.extend_from_slice(&idx.to_le_bytes()[0..4]);
    }

    sym_table_len = 0;
    // Write Symbol table
    for (i, sym) in sym_table.iter().enumerate() {
        if i > 0 {
            let mut type_data = match symbol_types[sym] {
                SymbolType::Local => 0,
                SymbolType::Exported => 1,
                SymbolType::External => 2,
            };
            let mut add = symbol_addresses[sym];
            let mut off = code_section.size();
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
            image.extend_from_slice(&x.to_le_bytes()[0..4]);
            image.extend_from_slice(&add.to_le_bytes()[0..4]);
        } else {
            // module name, reserved
            let x = (3 << 28) | (sym_table_len);
            let add = 0i32;
            image.extend_from_slice(&x.to_le_bytes()[0..4]);
            image.extend_from_slice(&add.to_le_bytes()[0..4]);
            sym_table_len += sym.len() + 1;
        }
    }
    // write Symbol Names in a compact manner
    for (i, sym) in sym_table.iter().enumerate() {
        if i == 0 {
            image.extend_from_slice(&sym.as_bytes().to_vec());
            image.extend_from_slice(&vec![0]);
            continue;
        }
        if let SymbolType::External | SymbolType::Exported = symbol_types[sym] {
            image.extend_from_slice(&sym.as_bytes().to_vec());
            image.extend_from_slice(&vec![0]);
        }
    }
    // Align to 4
    if image.len() % 4 != 0 {
        image.extend_from_slice(&vec![0; 4 - image.len() % 4]);
    }
    image.extend_from_slice(&code_section.data()?);
    image.extend_from_slice(&data_section.data()?);
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
    let mut objs: Vec<String> = Vec::new();
    let raw_objs: Vec<String> = vec![String::from("module.o")];
    let mut gfuncs = Vec::new();
    for obj in &raw_objs {
        let funcs = add_prefix(obj);
        gfuncs.extend_from_slice(&funcs);         
        
    } 
    for obj in &raw_objs {
        objs.push(obj.replace(".o", "_pre.o"));
        objs.push(obj.clone());
    }

    link_objects(&objs);
    process_binary(&String::from("out.elf"), gfuncs);
}
