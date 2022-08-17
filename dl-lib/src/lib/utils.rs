
extern crate alloc;
use core::{mem, slice};
use core::alloc::{GlobalAlloc, Layout};
use alloc::{vec::Vec, string::String};
use cortex_m_semihosting::dbg;

use crate::ALLOCATOR;

#[repr(C)]
#[derive(Debug)]
pub struct ModuleHeader {
  pub n_funcs: usize,
  pub n_table: usize,
  pub n_reloc: usize,
  pub l_symt: usize,
  pub l_text: usize,
  pub l_data: usize, 
  pub l_bss: usize,
  pub n_symbol: usize,
}

#[derive(Debug, Clone)]
pub struct Symbol {
  pub s_type: u8,
  pub index: usize,
  pub n_pos: usize,
  pub s_name: String,
}

#[derive(Debug, Clone)]
pub struct SymbolTable {
  pub symbols: Vec<Symbol>,
  // pub names: Vec<u8>,

}

fn parse_symtable(n_symbol: &usize, data: &Vec<u8>) -> SymbolTable {
  let mut symbols = Vec::new();
  let mut p = 0;
  let mut q = 8 * *n_symbol;
  for _ in 0..*n_symbol {
    let x = u32::from_le_bytes([data[p + 0], data[p+1], data[p+2], data[p+3]]);
    let s_type = ((x & (7 << 28)) >> 28) as u8;
    let n_pos = (x & !(7 << 28)) as usize;
    let index = usize::from_le_bytes([data[p+4], data[p+5], data[p+6], data[p+7]]);
    p += 8;
    // let s_name be the String between q and next 0 in data
    let mut s_name = String::new();
    while data[q] != 0 {
      s_name.push(data[q].into());
      q += 1;
    }
    q += 1;
    symbols.push(Symbol {
      s_type,
      index,
      n_pos,
      s_name,
    });
    
  }
  // let names = Vec::from(&data[p..]);
  SymbolTable {
    symbols
    // names,
  }
}

const HEADER_LEN : usize = mem::size_of::<ModuleHeader>();

// #[link_section = "text"]
// static mut mem_pool: [u8; 32768] = [0u8; 32768];

#[derive(Debug, Clone)]
pub struct Module {
  pub symt: SymbolTable,
  pub text_begin: usize,
  pub got_begin: usize,
}

impl Module {
  fn get_symbol(&self, name: &str) -> Option<&Symbol> {
    self.symt.symbols.iter().find(|s| s.s_name == name)
  }
}
// pub fn dl_open(file_name:&String) -> Vec<u8> {
//   let mut file = File::open(file_name).unwrap();
//   let mut buffer = Vec::new();
//   file.read_to_end(&mut buffer).unwrap();
//   buffer
// }

fn acquire_slice(buf: &[u8], left: usize, length:usize) -> (usize, Vec<u8>) {
  return (left + length, buf[left..left+length].to_vec());
}

fn malloc(n:usize, align:usize)  -> *mut u8 {
  unsafe {ALLOCATOR.alloc(Layout::from_size_align(n, align).unwrap())}
}

fn modify(slice: &mut[u8], v: u16) {
  let imm4  = (v >> 12) as u8;
  let i = (v >> 11 & 1) as u8;
  let imm3 = (v >> 8 & 7) as u8;
  let imm8 = (v & 255) as u8;
  slice[0] = (slice[0] & !0xF) | imm4;
  slice[1] = (slice[1] & !4) | i << 2;
  slice[2] = imm8;
  slice[3] = (slice[3] & !112) | imm3 << 4;
}
// fn modify(b:u8, a:u8, d:u8, c:u8, v:u16) -> (u8, u8, u8, u8) {
//   let imm4  = (v >> 12) as u8;
//   let i = (v >> 11 & 1) as u8;
//   let imm3 = (v >> 8 & 7) as u8;
//   let imm8 = (v & 255) as u8;
//   let na = (a & !4) | i << 2;
//   let nb  = (b & !0xF) | imm4;
//   let nc = (c & !112) | imm3 << 4;
//   let nd = imm8;
//   (nb, na, nd, nc)
// }

pub fn dl_load(buf:Vec<u8>, dependencies: Option<Module>) -> Module {
  let header_prefix = &buf[..HEADER_LEN];
  let p= header_prefix.as_ptr() as *const [u8; HEADER_LEN];
  let header: ModuleHeader = unsafe { mem::transmute(*p) };
  // let mut module: Module;
  // dbg!(&header);
  let (left, reloc) = acquire_slice(&buf, HEADER_LEN, header.n_reloc as usize * 8);
  // dbg!()
  let (left, gfunc) = acquire_slice(&buf, left, header.n_funcs * 4);
  let (left, raw_symt) = acquire_slice(&buf, left, header.l_symt);
  let (left, mut text) = acquire_slice(&buf, left, header.l_text);
  // dbg!(left, header.l_text);
  // dbg!(&text);
  let (_left, data) = acquire_slice(&buf, left, header.l_data);
  let mut symt = parse_symtable(&header.n_symbol, &raw_symt);

  let total_len = header.l_text + 4 * header.n_table;
  let code_ptr = malloc(total_len, 4);
  let code =  unsafe {slice::from_raw_parts_mut(code_ptr, total_len)};

  let text_begin = 4 * header.n_table;
  let text_begin_address = &code[text_begin] as *const u8 as usize;
  // dbg!(reloc.len());
  // Fi=(16i+4,16i+8)
  // O=16n+0,16n+1
  for i in 0..header.n_funcs {
    // dbg!(gfunc[i]);
    let p = i * 4;
    let idx = usize::from_le_bytes([gfunc[p], gfunc[p+1], gfunc[p+2], gfunc[p+3]]);
    let entry = text_begin_address + symt.symbols[idx].index;
    let w = 16 * i + 4;
    modify(&mut text[w+0..w+4], (entry & 0xffff) as u16);
    modify(&mut text[w+4..w+8], (entry >> 16) as u16);
    // dbg!(text[w+0]);
    // dbg!(text[w+1]);
    // dbg!(text[w+2]);
    // dbg!(text[w+3]);
    // symt.symbols[idx].index = 16 * i + 1;
  }

  let w = 16 * header.n_funcs;
  let r9 = code_ptr as usize;
  
  modify(&mut text[w+0..w+4], (r9 & 0xffff) as u16);
  modify(&mut text[w+4..w+8], (r9 >> 16) as u16);
  
  for i in 0..text.len() {
    code[text_begin + i] = text[i];
  }
  // dbg!(&symt);
  for i in 0..header.n_reloc {
    let l = i * 8;
    let r = l + 4;
    let o1 = usize::from_le_bytes([reloc[l], reloc[l+1], reloc[l+2], reloc[l+3]]);
    let o2 = usize::from_le_bytes([reloc[r], reloc[r+1], reloc[r+2], reloc[r+3]]);
    let s = &symt.symbols[o2];
    // dbg!(i, o1, o2);
    // dbg!(&s);
    match s.s_type & 3{
      1 => { // Exported
        if o1 < header.n_table.into() {
          let loc = o1 * 4;
          let idx = s.index;
          for j in 0..4 {
            code[loc + j] = data[idx + j];
          }
        } else {
          let loc = text_begin + o1;
          let data = (s.index + text_begin_address).to_le_bytes();
          for j in 0..4 {
            code[loc + j] = data[j];
          }
        }
      },
      2 => { // External
        // Resolve Foreign Symbol
        dbg!(&s.s_name);
        if let Some(ref dependencies) = dependencies {
          let symbol = dependencies.get_symbol(&s.s_name);
          if let Some(symbol) = symbol {
            let data = symbol.index.to_le_bytes();
            dbg!(&symbol);
            dbg!(o1);
            for j in 0..4 {
              code[text_begin + o1 + j] = data[j];
            }
          }
        }
        // dbg!("Unresolved!")
      },
      _ => {
        // ignored
      }
    }
  }
  
  for i in 0..header.n_funcs {
    let p = i * 4;
    let idx = usize::from_le_bytes([gfunc[p], gfunc[p+1], gfunc[p+2], gfunc[p+3]]);
    symt.symbols[idx].index = 16 * i + 1 + text_begin_address;
    // dbg!(&symt.symbols[idx]);
  }
  // dbg!(&symt);
  Module { symt: symt, text_begin: text_begin_address, got_begin: r9 }
  // )
  // hprintln!("{} {}", left, buf.len());
  // hprintln!("{:?}", module);
}

pub fn dl_entry_by_name(module:&Module, name: &String) -> usize {
  module.get_symbol(name).expect("Symbol not found").index
}

// trait serialize: Sized{
//   const len: usize = mem::size_of::<Self>;
// }
// impl serialize for u16 {};

pub fn dl_val_by_name(module:&Module, name:&String, bytes: usize) -> Vec<u8> {
  let offset = module.get_symbol(name).expect("Symbol not found").index;
  unsafe {
    let p = module.got_begin as *const u8;
    let mut v: Vec<u8> = Vec::new();
    for j in 0..bytes {
      v.push(*p.offset((offset + j) as isize));
    }
    v
  }
}