pub const ASM_PRE: &str = "    .syntax unified
    .arch armv7-m

    .text
    .thumb
";
pub const ASM_SUF: &str = "
    
    .end
";

pub const FUNPRE: &str = "
.thumb_func
.align 1
.globl {s}
.type {s}, %function
.extern {modulename}
.type {s}, %function
{s}:
push    {r9, lr}
movw r11, #0
movt r11, #0
b {modulename}
.size {s}, . - {s}
";

pub const OBJPRE: &str = "
.thumb_func
.align 1
.globl {s}
.type {s}, %function
{s}:
movw r9, #0
movt r9, #0
blx r11
pop {r9, pc}
bx lr 
.size {s}, . - {s}
";

pub const LINK_CMD: &str = "ld.lld -Tcode_before_data.ld --unresolved-symbols=ignore-in-object-files --emit-relocs {input} -o {output}";
pub const ASM_CMD: &str = "clang -c {asm} -o {elf} --target=thumbv7em-none-eabi";
