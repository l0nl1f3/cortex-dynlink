pub static NO_RECOV_FUNC_CALL: [u8; 20] = [
    0xdf, 0xf8, 0x08, 0xc0, // ldr.w r12, [pc, #8]
    0xdf, 0xf8, 0x08, 0x90, // ldr.w r9, [pc, #8]
    0x60, 0x47, // bx r12
    0x70, 0x47, // bx lr
    0x00, 0x00, 0x00, 0x00, // function entry
    0x00, 0x00, 0x00, 0x00, // object2 static base
];

// a. blx .plt.test // lr=a
// b. ....
// c. blx test // lr=c
// d. ....
// e. bx lr ?  // -> c

// a. blx .plt.test // lr=a
// b. ....
// c. bx test // lr=a -> unexpectedly leaves plt
// d. ....    // skipped
// e. bx lr ?

// d. recover r9
