#![feature(alloc_error_handler)]
#![no_main]
#![no_std]

extern crate alloc;
use panic_halt as _;

use self::alloc::{vec, vec::Vec};
use core::alloc::Layout;
use core::mem;

use alloc_cortex_m::CortexMHeap;
use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;

mod lib;
use lib::utils;

// this is the allocator the application will use
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

fn call_func(test: fn() -> bool) {
    let r = test();
    dbg!(r);
}

fn call_func_u8(test: fn(u8) -> bool) {
    let r = test(1);
    dbg!(r);
}
#[entry]
fn main() -> ! {
    let heap_start = cortex_m_rt::heap_start() as usize;
    let heap_end = 0x2001_8000;
    let heap_size = heap_end - heap_start;
    unsafe { ALLOCATOR.init(cortex_m_rt::heap_start() as usize, heap_size) }
    // let bytes = lib::binary::BUF;
    // let module = utils::dl_load(bytes.to_vec(), None);
    // let test_ptr = utils::dl_entry_by_name(&module, "test");
    // let test: fn(u8) -> bool = unsafe { mem::transmute(test_ptr as *const ()) };
    // call_func_u8(test);

    // dbg!(utils::dl_val_by_name(&module, "GLOBAL_X", |x| {
    //     u8::from_le_bytes(x.try_into().unwrap())
    // }));

    // dbg!(utils::dl_val_by_name(&module, "GLOBAL_Y", |x| {
    //     u32::from_le_bytes(x.try_into().unwrap())
    // }));

    test_extern();
    loop {}
}

fn test_extern() {
    // bin_def: generated from testcase/extern_symbols_1a
    let bin_def: Vec<u8> = vec![
        1, 0, 0, 0, 1, 0, 0, 0, 53, 0, 0, 0, 52, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 48,
        0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 80, 1, 0, 0, 0, 16, 0, 0, 80, 33, 0, 0, 0, 20, 0,
        0, 16, 0, 0, 0, 0, 95, 95, 99, 102, 57, 102, 51, 102, 100, 101, 95, 95, 105, 110, 99, 0,
        105, 110, 99, 0, 71, 76, 79, 66, 65, 76, 95, 88, 0, 45, 233, 0, 66, 64, 242, 0, 11, 192,
        242, 0, 11, 0, 240, 0, 184, 64, 242, 0, 9, 192, 242, 0, 9, 216, 71, 189, 232, 0, 130, 112,
        71, 3, 75, 89, 248, 3, 48, 26, 104, 16, 68, 24, 96, 112, 71, 0, 191, 0, 0, 0, 0, 20, 0, 0,
        0,
    ];
    // bin_call: generated from testcase/extern_symbols_1
    let bin_call: Vec<u8> = vec![
        1, 0, 0, 0, 3, 0, 0, 0, 84, 0, 0, 0, 84, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 72,
        0, 0, 0, 1, 0, 0, 0, 76, 0, 0, 0, 0, 0, 0, 0, 80, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0,
        16, 0, 0, 0, 0, 9, 0, 0, 32, 0, 0, 0, 0, 13, 0, 0, 80, 1, 0, 0, 0, 30, 0, 0, 16, 4, 0, 0,
        0, 39, 0, 0, 80, 33, 0, 0, 0, 71, 76, 79, 66, 65, 76, 95, 90, 0, 105, 110, 99, 0, 95, 95,
        48, 57, 56, 102, 54, 98, 99, 100, 95, 95, 116, 101, 115, 116, 0, 71, 76, 79, 66, 65, 76,
        95, 89, 0, 116, 101, 115, 116, 0, 45, 233, 0, 66, 64, 242, 0, 11, 192, 242, 0, 11, 0, 240,
        0, 184, 64, 242, 0, 9, 192, 242, 0, 9, 216, 71, 189, 232, 0, 130, 112, 71, 8, 181, 9, 75,
        89, 248, 3, 48, 152, 71, 8, 74, 8, 75, 89, 248, 2, 32, 89, 248, 3, 48, 18, 104, 27, 104,
        16, 68, 192, 26, 176, 250, 128, 240, 64, 9, 8, 189, 0, 191, 0, 0, 0, 0, 4, 0, 0, 0, 8, 0,
        0, 0, 10, 0, 0, 0, 31, 0, 0, 0,
    ];
    let def = utils::dl_load(bin_def, None);
    dbg!(&def);
    let call = utils::dl_load(bin_call, Some(vec![def]));
    dbg!(&call);
    let test_ptr = utils::dl_entry_by_name(&call, "test");
    let test: fn(u8) -> bool = unsafe { mem::transmute(test_ptr as *const ()) };
    call_func_u8(test);
    dbg!(utils::dl_val_by_name(&call, "GLOBAL_Y", |x| {
        u32::from_le_bytes(x.try_into().unwrap())
    }));
    dbg!(utils::dl_val_by_name(&call, "GLOBAL_Z", |x| {
        u32::from_le_bytes(x.try_into().unwrap())
    }));

    loop {}
}

// Out of memory
#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    asm::bkpt();

    loop {}
}
