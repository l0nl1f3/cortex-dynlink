#![no_main]
#![no_std]

// mod utils {
extern {
    pub fn adc(a:u32, b:u32) -> u32;
}   
// }

pub static mut A: u32 = 12345678;
pub static mut B: u32 = 23456789;

#[no_mangle]
pub fn test() -> bool {
    unsafe {
        adc(A, B) == A + B + 10
    }    
}

