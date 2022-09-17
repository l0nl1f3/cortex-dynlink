#![no_main]
#![no_std]

pub struct Pair {
    first: u32,
    second: u32,
}

impl Pair {
    pub fn multiply(&self, x: u32) -> Pair {
        Pair {
            first: self.first * x,
            second: self.second * x,
        }
    }
}
#[no_mangle]
pub fn test(x: u32) -> Pair {
    let p0 = Pair { first: 1, second: 2 };
    let p1 = p0.multiply(x);
    p1
}

