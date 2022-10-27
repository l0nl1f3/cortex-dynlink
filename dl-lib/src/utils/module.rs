extern crate alloc;
use alloc::{string::String, vec, vec::Vec};
use core::alloc::{GlobalAlloc, Layout};
use core::{mem, slice};

use super::{instr, template};
use crate::{ALLOCATOR, LR_RANGE_TO_BASE};

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
    pub index1: usize,
    pub index2: usize,
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
            index1: index,
            index2: 0,
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
    pub text_end: usize,
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

/// Generate plt
/// The plt consist of two parts, manual calls and cross boundary calls
/// The first part is for calls from the core, which doesn't require the recovery of r9 after function
/// The second part is the dynamic plt (switch case), which starts from only one svc instruction as default
fn generate_plt(func_entries: Vec<[u8; 4]>, block_size: usize, got_begin: usize) -> Vec<u8> {
    let cur_obj_base = got_begin.to_le_bytes();
    let mut plt = func_entries
        .iter()
        .flat_map(|entry| {
            let mut non_case_body = template::NO_RECOV_FUNC_CALL.to_vec();
            for i in 0..4 {
                non_case_body[12 + i] = entry[i];
                non_case_body[16 + i] = cur_obj_base[i];
            }
            non_case_body
        })
        .collect::<Vec<_>>();
    for entry in &func_entries {
        let function_entry = entry;
        let mut default = Vec::new();
        default.extend(instr::svc());
        default.extend(cur_obj_base);
        default.extend(function_entry);
        for _ in 0..block_size - default.len() {
            default.push(0);
        }
        plt.extend(default);
    }
    plt
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
    let got_begin = allocated_got_ptr as usize;
    // copy data section to RAM
    let allocated_data_ptr = malloc(header.l_data, 4);
    let allocated_data = unsafe { slice::from_raw_parts_mut(allocated_data_ptr, header.l_data) };
    let data = acquire_vec(&mut start, header.l_data);
    allocated_data.copy_from_slice(&data);

    // generate plt and copy to RAM
    let case_block_size = 60;
    let non_case_block_size = 20;
    let plt = generate_plt(
        {
            glb_funcs
                .iter()
                .map(|idx| (sym_table[*idx].index1 + text_begin).to_le_bytes())
                .collect::<Vec<_>>()
        },
        case_block_size,
        got_begin,
    );
    let allocated_plt = malloc(plt.len(), 4);
    unsafe {
        slice::from_raw_parts_mut(allocated_plt, plt.len()).copy_from_slice(&plt);
    }

    let data_begin = allocated_data_ptr as usize;

    for (offset, symt_idx) in relocs {
        let sym = &sym_table[symt_idx];
        match sym.s_type & 3 {
            0 | 1 => {
                // Exported / Local
                let got_index = usize::from_le_bytes(unsafe {
                    *text_begin_ptr.offset(offset as isize).cast::<[u8; 4]>()
                });
                let entry = sym.index1
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
                            let entry = symbol.index2.to_le_bytes();
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
    let plt_1_len = non_case_block_size * header.n_funcs;
    for (i, idx) in glb_funcs.iter().enumerate() {
        // TODO
        sym_table[*idx].index1 = allocated_plt as usize + non_case_block_size * i + 1;
        sym_table[*idx].index2 = allocated_plt as usize + case_block_size * i + plt_1_len + 1;
    }
    Module {
        sym_table,
        text_begin,
        text_end: text_begin + header.l_text,
        data_begin,
        got_begin,
    }
}

/// Given symbol name and the module it belongs to, returns the entry address of function
pub fn dl_entry_by_name(module: &Module, name: &str) -> usize {
    module.get_symbol(name).expect("Symbol not found").index1
}

/// Given symbol name (whose type is T) and the module it belongs to
/// given function to convert little-endian bytes to T
/// return a copy to the symbol
pub fn dl_val_by_name<T, F>(module: &Module, name: &str, bytes_to_t: F) -> T
where
    F: Fn(&[u8]) -> T,
{
    let offset = module.get_symbol(name).expect("Symbol not found").index1;
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

/// Given sp of exception stack frame, extend the plt according to lr
/// and return to lr
/// Pseudocode of dynamic PLT:
/// foo belongs to obj2
/// .plt.foo
///     switch lr
///         case lr_1
///             ldr r9, =obj2.static_base
///             ldr r12, =foo.entry
///             blx r12
///             ldr r9, =obj?.static_base (call site)
///             bx lr_1
///     ...
///         case lr_n
///             ldr r9, =obj2.static_base
///             ldr r12, =foo.entry
///             blx r12
///             ldr r9, =obj?.static_base (call site)
///             bx lr_n
///     ...
///         default
///             svc #0
///  .plt.foo (compile time)
///     switch lr
///         default
///             svc #0
///
/// switch case assembly level
/// case1:    
///     ldr r12, =lr_1
///     cmp lr, r12
///     beq +2
///     b case2
/// case1_body:
///     ldr r9, =obj2.static_base(const)
///     ldr r12, =foo.entry(const)
///     blx r12
///     ldr r9, =obj?.static_base (call site)
///     bx lr_1
/// case2:
///     ldr r12, =lr_2
///     cmp lr, r12
///     beq +2
///     b case3   
///     ...
/// case2_body:
///     ...
/// casen:
///     ldr r12, =lr_n
///     cmp lr r12
///     beq +2
///     b case2
/// casen_body:
///    ...
/// default:
///     svc #0     
///.word
///     def_static_base
///     func_entry
///
/// svc handler includes:
///     1. extend plt
///         move default to new place
///         place new case in default's old place
///     2. execute new case, return to lr
/// call_static_base are determined from which range lr belongs to

pub unsafe extern "C" fn svcall_handler(sp: *mut usize) {
    let lr = *sp.offset(5);
    let pc = *sp.offset(6);
    let pc_ptr = pc as *const u8;
    let mut case = vec![];
    case.extend(instr::nop());
    case.extend(instr::nop());
    case.extend(instr::ldr(12, lr));
    case.extend(instr::cmp_lr_r12());
    case.extend(instr::beq(2));
    let def_static_base = usize::from_le_bytes(*pc_ptr.offset(0).cast::<[u8; 4]>());
    let func_entry = usize::from_le_bytes(*pc_ptr.offset(4).cast::<[u8; 4]>());
    let mut call_static_base = 0;
    for m in LR_RANGE_TO_BASE.iter() {
        if m.contains(lr) {
            call_static_base = m.base();
            break;
        }
    }
    let mut case_body = vec![];
    case_body.extend(instr::ldr(9, def_static_base));
    case_body.extend(instr::ldr(12, func_entry));
    case_body.extend(instr::blx(12));
    case_body.extend(instr::ldr(9, call_static_base));
    case_body.extend(instr::ldr(12, lr));
    case_body.extend(instr::bx(12));
    case_body.extend(instr::nop());

    let next_default = malloc(case.len() + 4 + case_body.len(), 4);
    let prev_default = (pc - 4) as *mut u8;
    let dist = (next_default as i32) - (prev_default as i32) - (case.len() as i32) - 2;
    case.extend(instr::b_w(dist));
    case.extend(case_body);
    // copy default to new place

    for i in 0..case.len() {
        *next_default.offset(i as isize) = *prev_default.offset(i as isize);
    }
    slice::from_raw_parts_mut(prev_default, case.len()).copy_from_slice(&case);
}
