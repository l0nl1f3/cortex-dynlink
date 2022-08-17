#![no_main]
#![no_std]


pub fn sum2(x: i32, y: i32) -> i32 {
    x + y * 2
}
pub fn sum3(x: i32, y: i32, z: i32) -> i32 {
    x + y + z
}
pub fn sum4(x: i32, y: i32, z: i32, w: i32) -> i32 {
    x + y + z + w
}
pub fn sum5(x: i32, y: i32, z: i32, w: i32, v: i32) -> i32 {
    x + y + z + w + v
}
// use cortex_m_semihosting::hprintln;
#[no_mangle]
pub fn test(x: i32, y: i32) -> i32 {
    let s1 = sum2(x, y);
    let s2 = sum3(y, y, x);
    return s1 + s2;
}

