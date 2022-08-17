extern crate alloc;
use alloc::{string::String, vec::Vec};
use core::alloc::{GlobalAlloc, Layout};
use core::{mem, slice};
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

fn parse_symtable(n_symbol: &usize, data: &Vec<u8>) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut p = 0;
    let mut q = 8 * *n_symbol;
    for _ in 0..*n_symbol {
        let x = u32::from_le_bytes([data[p + 0], data[p + 1], data[p + 2], data[p + 3]]);
        let s_type = ((x & (7 << 28)) >> 28) as u8;
        let n_pos = (x & !(7 << 28)) as usize;
        let index = usize::from_le_bytes([data[p + 4], data[p + 5], data[p + 6], data[p + 7]]);
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
    symbols
}

const HEADER_LEN: usize = mem::size_of::<ModuleHeader>();

#[derive(Debug, Clone)]
pub struct Module {
    pub symt: Vec<Symbol>,
    pub text_begin: usize,
    pub got_begin: usize,
}

impl Module {
    // search symbol by name
    fn get_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symt.iter().find(|s| s.s_name == name)
    }
}

fn acquire_slice(buf: &[u8], left: usize, length: usize) -> (usize, Vec<u8>) {
    return (left + length, buf[left..left + length].to_vec());
}

// allocate n bytes from the heap and return a pointer to the beginning of the allocated memory
fn malloc(n: usize, align: usize) -> *mut u8 {
    unsafe { ALLOCATOR.alloc(Layout::from_size_align(n, align).unwrap()) }
}

// Modify movt/movw immediate
fn modify(slice: &mut [u8], v: u16) {
    let imm4 = (v >> 12) as u8;
    let i = (v >> 11 & 1) as u8;
    let imm3 = (v >> 8 & 7) as u8;
    let imm8 = (v & 255) as u8;
    slice[0] = (slice[0] & !0xF) | imm4;
    slice[1] = (slice[1] & !4) | i << 2;
    slice[2] = imm8;
    slice[3] = (slice[3] & !112) | imm3 << 4;
}

// load module from buf
// for external symbols, dependencies are assume to have their definition
pub fn dl_load(buf: Vec<u8>, dependencies: Option<Vec<Module>>) -> Module {
    let header_prefix = &buf[..HEADER_LEN];
    let p = header_prefix.as_ptr() as *const [u8; HEADER_LEN];
    let header: ModuleHeader = unsafe { mem::transmute(*p) };
    let (left, reloc) = acquire_slice(&buf, HEADER_LEN, header.n_reloc as usize * 8);
    let (left, gfunc) = acquire_slice(&buf, left, header.n_funcs * 4);
    let (left, raw_symt) = acquire_slice(&buf, left, header.l_symt);
    let (left, mut text) = acquire_slice(&buf, left, header.l_text);
    let (_left, data) = acquire_slice(&buf, left, header.l_data);
    let mut symt = parse_symtable(&header.n_symbol, &raw_symt);

    let total_len = header.l_text + 4 * header.n_table;
    let code_ptr = malloc(total_len, 4);
    let code = unsafe { slice::from_raw_parts_mut(code_ptr, total_len) };

    let text_begin = 4 * header.n_table;
    let text_begin_address = &code[text_begin] as *const u8 as usize;
    for i in 0..header.n_funcs {
        let p = i * 4;
        let idx = usize::from_le_bytes([gfunc[p], gfunc[p + 1], gfunc[p + 2], gfunc[p + 3]]);
        let entry = text_begin_address + symt[idx].index;
        let w = 16 * i + 4;
        modify(&mut text[w + 0..w + 4], (entry & 0xffff) as u16);
        modify(&mut text[w + 4..w + 8], (entry >> 16) as u16);
    }

    let w = 16 * header.n_funcs;
    let r9 = code_ptr as usize;
    modify(&mut text[w + 0..w + 4], (r9 & 0xffff) as u16);
    modify(&mut text[w + 4..w + 8], (r9 >> 16) as u16);
    for i in 0..text.len() {
        code[text_begin + i] = text[i];
    }

    for i in 0..header.n_reloc {
        let l = i * 8;
        let r = l + 4;
        let o1 = usize::from_le_bytes([reloc[l], reloc[l + 1], reloc[l + 2], reloc[l + 3]]);
        let o2 = usize::from_le_bytes([reloc[r], reloc[r + 1], reloc[r + 2], reloc[r + 3]]);
        let s = &symt[o2];
        match s.s_type & 3 {
            1 => {
                // Resolve exported symbol
                if o1 < header.n_table.into() {
                    // symbol is a variable
                    let idx = s.index;
                    for j in 0..4 {
                        code[idx + j] = data[idx + j];
                    }
                } else {
                    // symbol is a function
                    let loc = text_begin + o1;
                    let data = (s.index + text_begin_address).to_le_bytes();
                    for j in 0..4 {
                        code[loc + j] = data[j];
                    }
                }
            }
            2 => {
                // Resolve external symbol
                dbg!(&s.s_name);
                if let Some(ref dependencies) = dependencies {
                    for dependency in dependencies {
                        let symbol = dependency.get_symbol(&s.s_name);
                        if let Some(symbol) = symbol {
                            let data = symbol.index.to_le_bytes();
                            for j in 0..4 {
                                code[text_begin + o1 + j] = data[j];
                            }
                        }
                    }
                }
            }
            _ => {
                // ignored
            }
        }
    }

    for i in 0..header.n_funcs {
        let p = i * 4;
        let idx = usize::from_le_bytes([gfunc[p], gfunc[p + 1], gfunc[p + 2], gfunc[p + 3]]);
        symt[idx].index = 16 * i + 1 + text_begin_address;
    }
    Module {
        symt: symt,
        text_begin: text_begin_address,
        got_begin: r9,
    }
}

// dl_entry_by_name: find the address of function by name
pub fn dl_entry_by_name(module: &Module, name: &String) -> usize {
    module.get_symbol(name).expect("Symbol not found").index
}

// dl_val_by_bame: find the value of variable by name, return value in little endian bytes
pub fn dl_val_by_name(module: &Module, name: &String, bytes: usize) -> Vec<u8> {
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
