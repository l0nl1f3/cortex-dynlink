#![no_main]
#![no_std]


#[no_mangle]
pub fn test() -> u32 {
    let x = [1, 2, 3];
    let mut y = 0;
    for i in 0..3 {
        y += x[i];
    }
    y
}
