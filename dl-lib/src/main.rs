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
    let bytes = lib::binary::BUF;
    let module = utils::dl_load(bytes.to_vec(), None);
    let test_ptr = utils::dl_entry_by_name(&module, "test");
    let test: fn(u8) -> bool = unsafe { mem::transmute(test_ptr as *const ()) };
    call_func_u8(test);

    dbg!(utils::dl_val_by_name(&module, "GLOBAL_8", |x| {
        u8::from_le_bytes(x.try_into().unwrap())
    }));

    dbg!(utils::dl_val_by_name(&module, "GLOBAL_Y", |x| {
        u32::from_le_bytes(x.try_into().unwrap())
    }));

    test_extern();
    loop {}
}

fn test_extern() {
    // bin_def: generated from testcase/extern_symbols_1a
    let bin_def: Vec<u8> = vec![
        1, 0, 0, 0, 0, 0, 0, 0, 89, 0, 0, 0, 52, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1, 0,
        0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 45, 0, 0, 80, 33, 0, 0, 0, 49, 0, 0, 80, 1, 0, 0, 0, 95, 90,
        78, 49, 55, 101, 120, 116, 101, 114, 110, 95, 115, 121, 109, 98, 111, 108, 115, 95, 49, 97,
        49, 67, 49, 55, 104, 101, 98, 48, 52, 55, 98, 100, 99, 55, 101, 54, 54, 56, 51, 101, 100,
        69, 0, 97, 100, 99, 0, 95, 95, 50, 50, 53, 101, 56, 97, 51, 102, 95, 95, 97, 100, 99, 0,
        45, 233, 0, 66, 64, 242, 0, 11, 192, 242, 0, 11, 0, 240, 0, 184, 64, 242, 0, 9, 192, 242,
        0, 9, 216, 71, 189, 232, 0, 130, 112, 71, 64, 242, 0, 2, 8, 68, 192, 242, 0, 2, 89, 248, 2,
        32, 16, 68, 112, 71, 212, 212, 10, 0, 0, 0,
    ];
    // bin_call: generated from testcase/extern_symbols_1
    let bin_call: Vec<u8> = vec![
        1, 0, 0, 0, 1, 0, 0, 0, 154, 0, 0, 0, 92, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 88,
        0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 80, 1, 0, 0, 0, 17, 0, 0, 80, 33, 0, 0, 0, 22, 0,
        0, 16, 4, 0, 0, 0, 66, 0, 0, 16, 0, 0, 0, 0, 110, 0, 0, 32, 0, 0, 0, 0, 95, 95, 48, 57, 56,
        102, 54, 98, 99, 100, 95, 95, 116, 101, 115, 116, 0, 116, 101, 115, 116, 0, 95, 90, 78, 49,
        54, 101, 120, 116, 101, 114, 110, 95, 115, 121, 109, 98, 111, 108, 115, 95, 49, 49, 66, 49,
        55, 104, 97, 99, 52, 57, 48, 50, 98, 48, 54, 49, 99, 99, 99, 101, 56, 48, 69, 0, 95, 90,
        78, 49, 54, 101, 120, 116, 101, 114, 110, 95, 115, 121, 109, 98, 111, 108, 115, 95, 49, 49,
        65, 49, 55, 104, 51, 98, 55, 101, 52, 51, 53, 50, 57, 56, 97, 100, 102, 54, 101, 49, 69, 0,
        97, 100, 99, 0, 45, 233, 0, 66, 64, 242, 0, 11, 192, 242, 0, 11, 0, 240, 0, 184, 64, 242,
        0, 9, 192, 242, 0, 9, 216, 71, 189, 232, 0, 130, 112, 71, 176, 181, 2, 175, 64, 242, 4, 4,
        64, 242, 0, 5, 192, 242, 0, 4, 192, 242, 0, 5, 89, 248, 4, 16, 89, 248, 5, 0, 6, 74, 144,
        71, 89, 248, 4, 16, 89, 248, 5, 32, 17, 68, 64, 26, 10, 56, 176, 250, 128, 240, 64, 9, 176,
        189, 0, 191, 0, 0, 0, 0, 78, 97, 188, 0, 21, 236, 101, 1,
    ];
    let def = utils::dl_load(bin_def, None);
    let call = utils::dl_load(bin_call, Some(vec![def]));

    let test_ptr = utils::dl_entry_by_name(&call, "test");
    let test: fn() -> bool = unsafe { mem::transmute(test_ptr as *const ()) };
    call_func(test);
    dbg!(utils::dl_val_by_name(
        &call,
        "_ZN16extern_symbols_11A17h3b7e435298adf6e1E",
        |x| u32::from_le_bytes(x.try_into().unwrap())
    ));
    dbg!(utils::dl_val_by_name(
        &call,
        "_ZN16extern_symbols_11B17hac4902b061ccce80E",
        |x| u32::from_le_bytes(x.try_into().unwrap())
    ));

    loop {}
}

// Out of memory
#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    asm::bkpt();

    loop {}
}
