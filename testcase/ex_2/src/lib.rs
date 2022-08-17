#![no_main]
#![no_std]

pub fn inc(x: u32) -> u32 {
    x + 1
}

pub fn dec(x: u32) -> u32 {
    x - 1
}

#[no_mangle]
pub fn test() -> bool {
    return inc(9) == dec(11)
}


