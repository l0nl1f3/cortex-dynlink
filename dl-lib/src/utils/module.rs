extern crate alloc;
use alloc::{string::String, vec::Vec};
use core::alloc::{GlobalAlloc, Layout};
use core::{iter::zip, mem, slice};

use super::template;
use crate::ALLOCATOR;
use cortex_m_semihosting::dbg;

#[repr(C)]
#[derive(Debug)]
pub struct ModuleHeader {
    pub n_funcs: usize,
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
    pub s_name: String,
}

fn parse_symtable(n_symbol: &usize, data: &Vec<u8>) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for i in 0..*n_symbol {
        let p = i * 8;
        let x = u32::from_le_bytes(data[p..p + 4].try_into().unwrap());
        let index = usize::from_le_bytes(data[p + 4..p + 8].try_into().unwrap());
        let s_type = ((x & (7 << 28)) >> 28) as u8;
        let n_pos = (x & !(7 << 28)) as usize;
        // let s_name be the String between q and next 0 in data
        let mut s_name = String::new();
        // local varable needs no name
        if s_type & 3 != 0 {
            let mut q = 8 * *n_symbol + n_pos;
            while data[q] != 0 {
                s_name.push(data[q].into());
                q += 1;
            }
        }
        symbols.push(Symbol {
            s_type,
            index,
            s_name,
        });
    }
    symbols
}

const HEADER_LEN: usize = mem::size_of::<ModuleHeader>();

/// Loaded Module
#[derive(Debug, Clone)]
pub struct Module {
    pub sym_table: Vec<Symbol>,
    pub text_begin: usize,
    pub data_begin: usize,
    pub got_begin: usize,
}

impl Module {
    // search symbol by name
    fn get_symbol(&self, name: &str) -> Option<&Symbol> {
        self.sym_table.iter().find(|s| s.s_name == name)
    }
}

/// given start address and length, extract the region [start, start + length) to a vector
/// the assign start + length to start
fn acquire_vec(start: &mut usize, length: usize) -> Vec<u8> {
    let p_start = *start as *const u8;
    let slice = unsafe { slice::from_raw_parts(p_start, length) };
    *start += length;
    slice.to_vec()
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
    modify(&mut slice[0..4], (v & 0xffff) as u16); // movw
    modify(&mut slice[4..8], (v >> 16) as u16); // movt
}

/// Given binary image and dependencies of loaded modules, load module from address p_start to p_end
/// for external symbols, dependencies are assume to have their definition
/// This function consist of the following steps
/// 1. copy code section and data section to the heap and record both section address in the Module structure
/// 2. modify the trampolines to correct runtime addresses
/// 3. apply function relocations
/// 4. modify the entry in symbol table to redirect external function calls
///
///  The procedure will place GOT, data section and trampoline codes (function indirections) in the RAM
///  Other codes won't be copied to the RAM
pub fn dl_load(p_start: *const u8, dependencies: Option<Vec<Module>>) -> Module {
    let header: ModuleHeader = unsafe { mem::transmute(*p_start.cast::<[u8; HEADER_LEN]>()) };
    let mut start = HEADER_LEN + p_start as usize;

    let relocs: Vec<_> = acquire_vec(&mut start, header.n_reloc as usize * 8)
        .chunks(8)
        .map(|slice| {
            let offset = usize::from_le_bytes(slice[0..4].try_into().unwrap());
            let idx = usize::from_le_bytes(slice[4..8].try_into().unwrap());
            (offset, idx)
        })
        .collect();
    let glb_funcs: Vec<_> = acquire_vec(&mut start, header.n_funcs * 4)
        .chunks(4)
        .map(|idx_slice| usize::from_le_bytes(idx_slice.try_into().unwrap()))
        .collect();

    let raw_sym_table = acquire_vec(&mut start, header.l_symt);
    let mut sym_table = parse_symtable(&header.n_symbol, &raw_sym_table);
    if start % 4 != 0 {
        start += 4 - start % 4;
    }
    let text_begin = start;
    let text_begin_ptr = start as *const u8;
    start += header.l_text;
    // extract trampoline code and copy to RAM
    // trampoline code is a prefix of the text section and is of length 16 * (n_funcs + 1)

    // create GOT on RAM
    let allocated_got_ptr = malloc(header.n_reloc * 4, 4);
    let allocated_got = unsafe { slice::from_raw_parts_mut(allocated_got_ptr, header.n_reloc * 4) };

    // copy data section to RAM
    let allocated_data_ptr = malloc(header.l_data, 4);
    let allocated_data = unsafe { slice::from_raw_parts_mut(allocated_data_ptr, header.l_data) };
    let data = acquire_vec(&mut start, header.l_data);
    allocated_data.copy_from_slice(&data);
    // Generate plt
    // The plt consist of two parts, manual calls and cross boundary calls
    // The difference between them is that the former doesn't requires the recovery of R9 after the execution of function
    // The plt contains (number of funtions+number of function calls) entries
    let mut plt_1: Vec<u8> = Vec::new();
    let got_begin = allocated_got_ptr as usize;
    let cur_obj_base = got_begin.to_le_bytes();
    for g in &glb_funcs {
        let f = &sym_table[*g];
        let mut plt_1_call = template::NO_RECOV_FUNC_CALL.to_vec();
        let function_entry = (text_begin + f.index).to_le_bytes();
        for i in 0..4 {
            plt_1_call[12 + i] = function_entry[i];
            plt_1_call[16 + i] = cur_obj_base[i];
        }
        plt_1.extend(plt_1_call);
    }
    let mut plt_2: Vec<u8> = Vec::new();
    for (offset, symt_idx) in &relocs {
        let sym = &sym_table[*symt_idx];
        if let 2 = sym.s_type & 3 {
            if let Some(ref dependencies) = dependencies {
                for dependency in dependencies {
                    let symbol = dependency.get_symbol(&sym.s_name);
                    if let Some(symbol) = symbol {
                        let mut plt_2_call = template::RECOV_FUNC_CALL.to_vec();
                        let function_entry = (text_begin + symbol.index).to_le_bytes();
                        let foreign_obj_static_base = dependency.got_begin.to_le_bytes();
                        let return_address = (text_begin + offset + 4).to_le_bytes();
                        for i in 0..4 {
                            plt_2_call[20 + i] = function_entry[i];
                            plt_2_call[24 + i] = foreign_obj_static_base[i];
                            plt_2_call[28 + i] = return_address[i];
                        }
                        plt_2.extend(plt_2_call);
                    }
                }
            }
        }
    }
    plt_1.extend(plt_2);
    let allocated_plt = malloc(plt_1.len(), 4);
    unsafe {
        slice::from_raw_parts_mut(allocated_plt, plt_1.len()).copy_from_slice(&plt_1);
    }

    let data_begin = allocated_data_ptr as usize;

    let trampo_entries: Vec<_> = (0..header.n_funcs).map(|i| 20 * i).collect();

    for (offset, symt_idx) in relocs {
        let sym = &sym_table[symt_idx];
        match sym.s_type & 3 {
            0 | 1 => {
                // Exported / Local
                let got_index = usize::from_le_bytes(unsafe {
                    *text_begin_ptr.offset(offset as isize).cast::<[u8; 4]>()
                });
                let entry = sym.index
                    + if sym.s_type > 4 {
                        text_begin
                    } else {
                        data_begin
                    };
                let entry = entry.to_le_bytes();
                for j in 0..4 {
                    allocated_got[got_index + j] = entry[j];
                }
            }
            2 => {
                // External
                if let Some(ref dependencies) = dependencies {
                    for dependency in dependencies {
                        let symbol = dependency.get_symbol(&sym.s_name);
                        if let Some(symbol) = symbol {
                            let entry = symbol.index.to_le_bytes();
                            let got_index = usize::from_le_bytes(unsafe {
                                *text_begin_ptr.offset(offset as isize).cast::<[u8; 4]>()
                            });
                            for j in 0..4 {
                                allocated_got[got_index + j] = entry[j];
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for (idx, trampo_entry) in zip(glb_funcs, trampo_entries) {
        // TODO
        sym_table[idx].index = allocated_plt as usize + trampo_entry + 1;
    }
    Module {
        sym_table,
        text_begin,
        data_begin,
        got_begin,
    }
}

/// Given symbol name and the module it belongs to, returns the entry address of function
pub fn dl_entry_by_name(module: &Module, name: &str) -> usize {
    module.get_symbol(name).expect("Symbol not found").index
}

/// Given symbol name (whose type is T) and the module it belongs to
/// given function to convert little-endian bytes to T
/// return a copy to the symbol
pub fn dl_val_by_name<T, F>(module: &Module, name: &str, bytes_to_t: F) -> T
where
    F: Fn(&[u8]) -> T,
{
    let offset = module.get_symbol(name).expect("Symbol not found").index;
    let size_of = mem::size_of::<T>();
    unsafe {
        let data_begin = module.data_begin as *const u8;
        let mut v: Vec<u8> = Vec::new();
        for j in 0..size_of {
            v.push(*data_begin.offset((offset + j) as isize));
        }
        bytes_to_t(&v)
    }
}
