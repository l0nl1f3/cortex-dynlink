#![feature(alloc_error_handler)]
#![no_main]
#![no_std]

extern crate alloc;
use panic_halt as _;

use self::alloc::{string::String, vec, vec::Vec};
use core::alloc::Layout;
use core::mem;

use alloc_cortex_m::CortexMHeap;
use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::{dbg, hprintln};

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
    let test_ptr = utils::dl_entry_by_name(&module, &String::from("test"));
    let test: fn(u8) -> bool = unsafe { mem::transmute(test_ptr as *const ()) };
    call_func_u8(test);
    
    dbg!(utils::dl_val_by_name(
        &module,
        &String::from("GLOBAL_8"),
        |x| u8::from_le_bytes(x.try_into().unwrap())
    ));

    dbg!(utils::dl_val_by_name(
        &module,
        &String::from("GLOBAL_Y"),
        |x| u32::from_le_bytes(x.try_into().unwrap())
    ));
    loop {}
}

fn test_extern() {
    // bin_def: generated from testcase/extern_symbols_1a
    let bin_def: Vec<u8> = vec![
        1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 104, 0, 0, 0, 52, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 4,
        0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 48, 0, 0, 0, 0, 7, 0, 0, 16, 0, 0, 0,
        0, 52, 0, 0, 80, 1, 0, 0, 0, 68, 0, 0, 80, 33, 0, 0, 0, 109, 111, 100, 117, 108, 101, 0,
        95, 90, 78, 49, 55, 101, 120, 116, 101, 114, 110, 95, 115, 121, 109, 98, 111, 108, 115, 95,
        49, 97, 49, 67, 49, 55, 104, 51, 99, 97, 48, 56, 53, 54, 57, 54, 100, 50, 101, 101, 50, 57,
        99, 69, 0, 95, 95, 50, 50, 53, 101, 56, 97, 51, 102, 95, 95, 97, 100, 99, 0, 97, 100, 99,
        0, 45, 233, 0, 66, 64, 242, 0, 11, 192, 242, 0, 11, 0, 240, 0, 184, 64, 242, 0, 9, 192,
        242, 0, 9, 216, 71, 189, 232, 0, 130, 112, 71, 8, 68, 64, 242, 0, 2, 192, 242, 0, 2, 73,
        70, 137, 88, 8, 68, 112, 71, 212, 212, 10, 0, 0, 0,
    ];
    // bin_call: generated from testcase/extern_symbols_1
    let bin_call: Vec<u8> = vec![
        1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 172, 0, 0, 0, 112, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 6,
        0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 108, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0,
        0, 0, 0, 0, 48, 0, 0, 0, 0, 7, 0, 0, 16, 4, 0, 0, 0, 51, 0, 0, 80, 33, 0, 0, 0, 56, 0, 0,
        32, 0, 0, 0, 0, 60, 0, 0, 80, 1, 0, 0, 0, 77, 0, 0, 16, 0, 0, 0, 0, 109, 111, 100, 117,
        108, 101, 0, 95, 90, 78, 49, 54, 101, 120, 116, 101, 114, 110, 95, 115, 121, 109, 98, 111,
        108, 115, 95, 49, 49, 66, 49, 55, 104, 98, 100, 54, 55, 53, 57, 99, 52, 97, 100, 48, 97,
        50, 49, 100, 98, 69, 0, 116, 101, 115, 116, 0, 97, 100, 99, 0, 95, 95, 48, 57, 56, 102, 54,
        98, 99, 100, 95, 95, 116, 101, 115, 116, 0, 95, 90, 78, 49, 54, 101, 120, 116, 101, 114,
        110, 95, 115, 121, 109, 98, 111, 108, 115, 95, 49, 49, 65, 49, 55, 104, 54, 97, 99, 99, 98,
        50, 55, 53, 50, 55, 56, 49, 49, 98, 49, 101, 69, 0, 0, 0, 0, 45, 233, 0, 66, 64, 242, 0,
        11, 192, 242, 0, 11, 0, 240, 0, 184, 64, 242, 0, 9, 192, 242, 0, 9, 216, 71, 189, 232, 0,
        130, 112, 71, 128, 181, 111, 70, 130, 176, 64, 242, 0, 0, 192, 242, 0, 0, 73, 70, 8, 88,
        64, 242, 4, 2, 192, 242, 0, 2, 137, 88, 11, 74, 144, 71, 1, 144, 255, 231, 1, 152, 64, 242,
        0, 1, 192, 242, 0, 1, 74, 70, 81, 88, 64, 242, 4, 3, 192, 242, 0, 3, 210, 88, 17, 68, 64,
        26, 10, 56, 176, 250, 128, 240, 64, 9, 2, 176, 128, 189, 0, 0, 0, 0, 78, 97, 188, 0, 21,
        236, 101, 1,
    ];
    let def = utils::dl_load(bin_def, None);
    let call = utils::dl_load(bin_call, Some(vec![def]));

    let test_ptr = utils::dl_entry_by_name(&call, &String::from("test"));
    let test: fn() -> bool = unsafe { mem::transmute(test_ptr as *const ()) };
    call_func(test);
    dbg!(utils::dl_val_by_name(
        &call,
        &String::from("_ZN16extern_symbols_11A17h6accb27527811b1eE"),
        |x| u32::from_le_bytes(x.try_into().unwrap())
    ));
    dbg!(utils::dl_val_by_name(
        &call,
        &String::from("_ZN16extern_symbols_11B17hbd6759c4ad0a21dbE"),
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
