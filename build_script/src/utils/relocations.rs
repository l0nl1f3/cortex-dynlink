use std::error::Error;
use std::collections::HashMap;
use std::fs;
use object::read::elf::{FileHeader,Sym,Rel, SectionHeader};
use object::elf::FileHeader32;
use object::Endianness;

#[derive(Debug,Clone)] 
pub enum RelocationType {
    MOVW_BREL_NC,
    MOVT_BREL,
    CALL,
    ABS32,
    NONE
}

fn get_relocation_type(r_type: u32) -> RelocationType {
    match r_type {
        87 => RelocationType::MOVW_BREL_NC,
        88 => RelocationType::MOVT_BREL,
        10 => RelocationType::CALL,
        2 => RelocationType::ABS32,
        _ =>  RelocationType::NONE,
        // panic!("Unknown relocation type")
    }
}

#[derive(Debug)]
pub struct Relocation {
    pub r_offset: u32,
    pub r_value: u32,
    pub r_info: u32,
    pub r_type: RelocationType,
    pub name: String,
}

// get relocation entries and extract some necessary information from file obj
pub fn get_relocations(obj:&String) -> Result<Vec<Relocation>, Box<dyn Error> > {
  let file = fs::File::open(obj)?;
  let data = match unsafe { memmap2::Mmap::map(&file) } {
      Ok(mmap) => mmap,
      Err(err) => {return Err(Box::new(err)); }
  };
  let elf = FileHeader32::<Endianness>::parse(&*data)?;
  let endian = elf.endian()?;
  let sections = elf.sections(endian, &*data)?;
  let mut vec_relocations: Vec<Relocation> = Vec::new();
  for (index, section) in sections.iter().enumerate() {
    // println!("{:?} {:?}", index, section);
    if let SHT_REL = section.sh_type(endian) {
        let relocations = section.rel(endian, &*data)?;
        if let None = relocations {
            continue;
        }
        let (relocations, link) = relocations.unwrap();
        let symbols = sections
            .symbol_table_by_index(endian, &*data, link);
        for relocation in relocations {
            let r_offset = relocation.r_offset(endian);
            let r_type = get_relocation_type(relocation.r_type(endian));
            let r_info = relocation.r_info(endian);
            let sym = relocation.r_sym(endian);
            let name = symbols.and_then(|symbols| {
                symbols
                    .symbol(sym as usize)
                    .and_then(|symbol| symbol.name(endian, symbols.strings()))
            }).unwrap().to_vec();
            let name = String::from_utf8(name).unwrap();
            
            let value = symbols.and_then(|symbols| {
                symbols
                    .symbol(sym as usize)
                    .and_then(|symbol| Ok(symbol.st_value(endian)))
            }).unwrap();
            // print_rel_symbol(p, endian, symbols, sym);
            // print_type_of(&name);
            
            // println!("{}, offset={}, type={:?}, value={}", name, r_offset, r_type, value);
            if !name.ends_with("module") {
                vec_relocations.push(Relocation {
                    r_offset,
                    r_value: value,
                    r_type,
                    r_info,
                    name
                });
            }
        }
    }
  }
  return Ok(vec_relocations);
}
