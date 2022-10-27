#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(asm_sym)]
#![no_main]
#![no_std]
#![warn(dead_code)]
#![warn(unused_imports)]

extern crate alloc;
use alloc::{vec, vec::Vec};
use panic_halt as _;

use crate::utils::module::{dl_entry_by_name, dl_val_by_name};

use core::{alloc::Layout, mem};

use alloc_cortex_m::CortexMHeap;
use core::arch::asm;
use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;

mod utils;
use utils::{instr, module};
// this is the allocator the application will use
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

struct Range {
    start: usize,
    end: usize,
    base: usize,
}
impl Range {
    fn contains(&self, addr: usize) -> bool {
        self.start <= addr && addr < self.end
    }
    fn base(&self) -> usize {
        self.base
    }
}
static mut lr_range_to_base: Vec<Range> = vec![];

fn init_heap() {
    let heap_start = cortex_m_rt::heap_start() as usize;
    let heap_end = 0x2001_8000;
    let heap_size = heap_end - heap_start;
    unsafe { ALLOCATOR.init(cortex_m_rt::heap_start() as usize, heap_size) }
}

extern "C" {
    static _binary_module_call_bin_start: u8;
    static _binary_module_call_bin_end: u8;
    static _binary_module_call_bin_size: u8;
    static _binary_module_def_bin_start: u8;
    static _binary_module_def_bin_end: u8;
    static _binary_module_def_bin_size: u8;
}

fn call_func_arg(func: fn(u32) -> u32, arg: u32) -> u32 {
    func(arg)
}

#[entry]
fn main() -> ! {
    init_heap();
    let p_start = unsafe { &_binary_module_def_bin_start as *const u8 };
    let module_def = module::dl_load(p_start, None);
    unsafe {
        lr_range_to_base.push(Range {
            start: module_def.text_begin,
            end: module_def.text_end,
            base: module_def.got_begin,
        });
    }
    let p_start = unsafe { &_binary_module_call_bin_start as *const u8 };
    let module_call = module::dl_load(p_start, Some(vec![module_def.clone()]));
    unsafe {
        lr_range_to_base.push(Range {
            start: module_call.text_begin,
            end: module_call.text_end,
            base: module_call.got_begin,
        });
    }
    let entry = dl_entry_by_name(&module_call, "test");
    let f = unsafe { mem::transmute::<usize, fn(u32) -> u32>(entry) };
    dbg!(call_func_arg(f, 1));
    let x = dl_val_by_name(&module_def, "GLOBAL_X", |x| {
        u8::from_le_bytes(x.try_into().unwrap())
    });
    dbg!(x);
    loop {}
}

// Out of memory
#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    asm::bkpt();

    loop {}
}

#[naked]
#[export_name = "SVCall"]
pub unsafe extern "C" fn svccall_trampoline() {
    asm!(
        "tst lr, #4",
        "ite eq",
        "mrseq r0, MSP",
        "mrsne r0, PSP",
        "bl {svcall_handler}",
        "movw r1, #0xFFF9",
        "movt r1, #0xFFFF",
        "bx r1",
        svcall_handler = sym module::svcall_handler,
        options(noreturn)
    )
}
