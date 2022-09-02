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
        if s_type & 3 != 0 {
            while data[q] != 0 {
                s_name.push(data[q].into());
                q += 1;
            }
            q += 1;
        }
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
    pub sym_table: Vec<Symbol>,
    pub text_begin: usize,
    pub data_begin: usize,
}

impl Module {
    // search symbol by name
    fn get_symbol(&self, name: &str) -> Option<&Symbol> {
        self.sym_table.iter().find(|s| s.s_name == name)
    }
}

fn acquire_vec(buf: &[u8], begin: &mut usize, length: usize) -> Vec<u8> {
    let slice = buf[*begin..*begin + length].to_vec();
    *begin += length;
    slice
}

/// allocate n bytes from the heap and return a pointer to the beginning of the allocated memory
fn malloc(n: usize, align: usize) -> *mut u8 {
    unsafe { ALLOCATOR.alloc(Layout::from_size_align(n, align).unwrap()) }
}

fn modify(slice: &mut [u8], v: u16) {
    let imm4 = (v >> 12) as u8;
    let i = (v >> 11 & 1) as u8;
    let imm3 = (v >> 8 & 7) as u8;
    let imm8 = (v & 255) as u8;
    slice[0] = slice[0] | imm4;
    slice[1] = slice[1] | i << 2;
    slice[2] = imm8;
    slice[3] = slice[3] | imm3 << 4;
}

/// Given opcode of the following
///     movw #0
///     movt #0
/// modify it to
///     movw v % 2^16
///     movt v / 2^16
fn modify_pair(slice: &mut [u8], v: usize) {
    modify(&mut slice[0..4], (v & 0xffff) as u16);
    modify(&mut slice[4..8], (v >> 16) as u16);
}

/// Given binary image and dependencies of loaded modules, load module from buf
/// for external symbols, dependencies are assume to have their definition
///
pub fn dl_load(buf: Vec<u8>, dependencies: Option<Vec<Module>>) -> Module {
    let header_ptr = (&buf[..HEADER_LEN]).as_ptr() as *const [u8; HEADER_LEN];
    let header: ModuleHeader = unsafe { mem::transmute(*header_ptr) };
    let mut begin = HEADER_LEN;
    let relocs = acquire_vec(&buf, &mut begin, header.n_reloc as usize * 8);
    let glb_funcs = acquire_vec(&buf, &mut begin, header.n_funcs * 4);
    let raw_sym_table = acquire_vec(&buf, &mut begin, header.l_symt);

    if begin % 4 > 0 {
        begin += 4 - begin % 4;
    }
    let text = acquire_vec(&buf, &mut begin, header.l_text);
    let data = acquire_vec(&buf, &mut begin, header.l_data);
    let mut sym_table = parse_symtable(&header.n_symbol, &raw_sym_table);

    let allocated_text_ptr = malloc(header.l_text, 4);
    let allocated_text = unsafe { slice::from_raw_parts_mut(allocated_text_ptr, header.l_text) };
    allocated_text.copy_from_slice(&text);

    let allocated_data_ptr = malloc(header.l_data, 4);
    let allocated_data = unsafe { slice::from_raw_parts_mut(allocated_data_ptr, header.l_data) };
    allocated_data.copy_from_slice(&data);

    let text_begin = allocated_text_ptr as usize;
    let data_begin = allocated_data_ptr as usize;

    for i in 0..header.n_funcs {
        let p = i * 4;
        let idx = usize::from_le_bytes(glb_funcs[p..p + 4].try_into().unwrap());
        let entry = text_begin + sym_table[idx].index;
        modify_pair(&mut allocated_text[4 * p + 4..4 * p + 12], entry);
    }

    let w = 16 * header.n_funcs;
    modify_pair(&mut allocated_text[w..w + 8], data_begin);

    for i in 0..header.n_reloc {
        let offset = usize::from_le_bytes(relocs[i * 8..i * 8 + 4].try_into().unwrap());
        let symt_idx = usize::from_le_bytes(relocs[i * 8 + 4..i * 8 + 8].try_into().unwrap());
        let sym = &sym_table[symt_idx];
        match sym.s_type & 3 {
            0 | 1 => {
                if sym.s_type > 3 {
                    // symbol is a function
                    let entry = (sym.index + text_begin).to_le_bytes();
                    for j in 0..4 {
                        allocated_text[offset + j] = entry[j];
                    }
                }
            }
            2 => {
                // Resolve external symbol
                dbg!(&sym.s_name);
                if sym.s_type > 3 {
                    if let Some(ref dependencies) = dependencies {
                        for dependency in dependencies {
                            let symbol = dependency.get_symbol(&sym.s_name);
                            if let Some(symbol) = symbol {
                                let entry = symbol.index.to_le_bytes();
                                for j in 0..4 {
                                    allocated_text[offset + j] = entry[j];
                                }
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
        let idx = usize::from_le_bytes(glb_funcs[p..p + 4].try_into().unwrap());
        sym_table[idx].index = 16 * i + 1 + text_begin;
    }
    Module {
        sym_table,
        text_begin,
        data_begin,
    }
}

/// Given symbol name and the module it belongs to, returns the entry address of function
pub fn dl_entry_by_name(module: &Module, name: &String) -> usize {
    module.get_symbol(name).expect("Symbol not found").index
}

/// Given symbol name (whose type is T) and the module it belongs to
/// given function to convert little-endian bytes to T
/// return a copy to the symbol
pub fn dl_val_by_name<T, F>(module: &Module, name: &String, bytes_to_t: F) -> T
where
    F: Fn(&[u8]) -> T,
{
    let offset = module.get_symbol(name).expect("Symbol not found").index;
    let size_of = mem::size_of::<T>();
    unsafe {
        let p = module.data_begin as *const u8;
        let mut v: Vec<u8> = Vec::new();
        for j in 0..size_of {
            v.push(*p.offset((offset + j) as isize));
        }
        bytes_to_t(&v)
    }
}
