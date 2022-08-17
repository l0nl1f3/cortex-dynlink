#![no_main]
#![no_std]

pub static mut C: u32 = 10;

#[no_mangle]
pub fn adc(A: u32, B: u32) -> u32 {
    unsafe {
        A + B + C
    }
}



