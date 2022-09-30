#![feature(alloc_error_handler)]
#![no_main]
#![no_std]
#![warn(dead_code)]
#![warn(unused_imports)]

extern crate alloc;
use panic_halt as _;

use crate::utils::module::{dl_entry_by_name, dl_val_by_name};

use self::alloc::{vec, vec::Vec};
use core::alloc::Layout;
use core::mem;

use alloc_cortex_m::CortexMHeap;
use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;

mod utils;
use utils::module;
// this is the allocator the application will use
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

fn init_heap() {
    let heap_start = cortex_m_rt::heap_start() as usize;
    let heap_end = 0x2001_8000;
    let heap_size = heap_end - heap_start;
    unsafe { ALLOCATOR.init(cortex_m_rt::heap_start() as usize, heap_size) }
}

extern "C" {
    static _binary_module_bin_start: u8;
    static _binary_module_bin_end: u8;
    static _binary_module_bin_size: u8;
}

fn call_func_arg(func: fn(u32) -> u32, arg: u32) -> u32 {
    func(arg)
}

#[entry]
fn main() -> ! {
    init_heap();
    let p_start = unsafe { &_binary_module_bin_start as *const u8 };
    let module = module::dl_load(p_start, None);
    let entry = dl_entry_by_name(&module, "test");
    let f = unsafe { mem::transmute::<usize, fn(u32) -> u32>(entry) };
    dbg!(call_func_arg(f, 1));
    let x = dl_val_by_name(&module, "GLOBAL_X", |x| {
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
