pub const ASM_PRE:&str = "    .syntax unified
    .arch armv7-m

    .text
    .thumb
";
pub const ASM_SUF:&str = "
    
    .end
";

pub const ASM_REPEAT_TMPL:&str = "
.thumb_func
.align 1
.globl {s}
.type {s}, %function
.extern {actname}
.type {actname}, %function
{s}:
push    {r9, lr}
push    {r0, r1}
mov     r1, #0x1c
ldr     r1, [r1]
mov     r0, pc
blx     r1
mov     r9, r0
pop     {r0, r1}
bl      {actname}
pop     {r9, pc}

.size   {s}, . - {s}
";


pub const FUNPRE:&str = "
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

pub const OBJPRE:&str = "
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