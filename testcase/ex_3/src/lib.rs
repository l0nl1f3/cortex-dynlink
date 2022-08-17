#![no_main]
#![no_std]

#[no_mangle] 
pub static mut GLOBAL_X:u8 = 20;
#[no_mangle]
pub static mut GLOBAL_Y:u32 = 31;
#[no_mangle]
pub static mut GLOBAL_8:u8 = 10;

#[no_mangle]
pub fn test(add:u8) -> bool {
    unsafe {
        GLOBAL_8 += add;
        return (GLOBAL_X + GLOBAL_8) as u32 == GLOBAL_Y;
    } 
}
