arm-none-eabi-gcc -fPIE -msingle-pic-base -mcpu=cortex-m4 -mthumb -fomit-frame-pointer -fno-inline -fno-section-anchors -mno-pic-data-is-text-relative -mlong-calls -O2 -c $1.c
cp $1.o ../../build_script/module.o