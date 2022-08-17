#![no_main]
#![no_std]

#[no_mangle] 
pub static mut GLOBAL_X:i32 = 20;
#[no_mangle]
pub static mut GLOBAL_Y:i32 = 30;
// #[no_mangle]
// pub static mut GLOBAL_128:u32 = 0xaaaaaaaa;

#[no_mangle]
pub fn test() -> bool {
    unsafe {
        GLOBAL_X += 10;
        GLOBAL_Y += 11;
        // GLOBAL_128 += 1;
        return GLOBAL_X + 11 == GLOBAL_Y;
    } 
}
