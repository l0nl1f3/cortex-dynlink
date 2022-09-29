cargo build
llvm-objcopy --input-target binary --output-target elf32-littlearm --rename-section .data=.rodata --binary-architecture arm $1.bin $1.o
ld.lld -Tlink.ld target/thumbv7em-none-eabi/debug/libdl_lib.a $1.o -o executable
/home/zhouyi/.local/xPacks/@xpack-dev-tools/qemu-arm/7.0.0-1.1/.content/bin/qemu-system-gnuarmeclipse --board STM32F4-Discovery -nographic -semihosting-config enable=on,target=native -S -gdb tcp::3333 -kernel executable
