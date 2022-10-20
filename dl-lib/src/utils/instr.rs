use alloc::{vec, vec::Vec};

pub fn b_w(imm24: i32) -> Vec<u8> {
    let instr: [u8; 4] = [0xfe, 0xf7, 0x96, 0xbd];
    let imm11 = ((imm24 >> 1) & 0x7ff) as u16;
    let imm11_h = (imm11 >> 8) as u8;
    let imm11_l = (imm11 & 0xff) as u8;
    let imm10 = ((imm24 >> 12) & 0x3ff) as u16;
    let imm10_h = (imm10 >> 8) as u8;
    let imm10_l = (imm10 & 0xff) as u8;
    let s = ((imm24 >> 24) & 0x1) as u8;
    let i1 = (((imm24 >> 22) & 0x1) as u8) ^ s ^ 1;
    let i2 = (((imm24 >> 23) & 0x1) as u8) ^ s ^ 1;
    vec![
        imm10_l,
        imm10_h | (s << 2) | (0x1e << 3),
        (imm11_l & 0xff) as u8,
        imm11_h | (i2 << 3) | (i1 << 5) | 0x90,
    ]
}

pub fn mov_t_w(is_t: bool, reg: u8, v: u16) -> Vec<u8> {
    let imm4 = (v >> 12) as u8;
    let i = (v >> 11 & 1) as u8;
    let imm3 = (v >> 8 & 7) as u8;
    let imm8 = (v & 255) as u8;
    vec![
        if is_t { 0xc0 } else { 0x40 } | imm4,
        0xf2 | i << 2,
        imm8,
        reg | (imm3 << 4),
    ]
}

pub fn blx(reg: u8) -> Vec<u8> {
    vec![0x47, 0xf0, 0x00, reg]
}

pub fn bx(reg: u8) -> Vec<u8> {
    vec![0x47, 0xf0, 0x30, reg]
}

pub fn cmp_lr_r12() -> Vec<u8> {
    vec![0xe6, 0x45]
}

pub fn beq_4() -> Vec<u8> {
    vec![0x02, 0xd0]
}

pub fn svc() -> Vec<u8> {
    vec![0x00, 0xdf]
}

pub fn nop() -> Vec<u8> {
    vec![0x00, 0xbf]
}
