pub const ASM_HEAD: &str = "    .syntax unified
    .arch armv7-m

    .text
    .thumb
";
pub const ASM_TAIL: &str = "
    
    .end
";

#[macro_export]
macro_rules! ASM_CMD {
    () => {
        r"clang -c {asm} -o {elf} --target=thumbv7em-none-eabi"
    };
}

#[macro_export]
macro_rules! FUNPRE {
    () => {
        r"
    .thumb_func
    .align 4
    .globl {s}
    .type {s}, %function
    .extern {modulename}
    .type {s}, %function
    {s}:
    push    {{r9, lr}}
    movw r11, #0
    movt r11, #0
    b {modulename}
    .size {s}, . - {s}
    "
    };
}

#[macro_export]
macro_rules! OBJPRE {
    () => { r"
    .thumb_func
    .align 4
    .globl {s}
    .type {s}, %function
    {s}:
    movw r9, #0
    movt r9, #0
    blx r11
    pop {{r9, pc}}
    bx lr 
    .size {s}, . - {s}
    " };
}

#[macro_export]
macro_rules! LINK_CMD{
    () => {
        r"ld.lld -Tcode_before_data.ld --unresolved-symbols=ignore-in-object-files --emit-relocs {input} -o {output}"
    };
}
