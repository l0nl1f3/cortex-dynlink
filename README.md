# cortex-dynlink

Dynamic Linking for ARM MCU

## testcase

This folder contains some demo cases for the project. Run the following command to build objects.

```
cargo build -Zbuild-std=core --release
```

Then copy the output .o into build_script/module.o, use

```
cargo run 
```

to run build_script and convert the module.o into dynamic loadable image. The converted code section is also in out.elf.  

The process can be simplified into running the following command in validate/ 

```
cargo run -- -c <CASE_NAME>
```

Defaultly, the image binary will be formatted and written to dl-lib/src/lib/binary.rs for convienience.

## Run on MCU

dl-lib is the code running on MCU that takes over loading modules that were created by build_script. To run this on QEMU:

```
cargo run
```

Then press `c` to start. . If no debug required, switch the comment in dl-lib/.cargo/config.toml from Line 18 to Line 16. 

The `dl-lib/src/lib` provides the following three basic interfaces:

```Rust
// load module from buf
// for external symbols, dependencies are assume to have their definition
// dependencies=none indicates no external symbol
pub fn dl_load(buf: Vec<u8>, dependencies: Option<Vec<Module>>) -> Module
// dl_entry_by_name: find the address of function by name
pub fn dl_entry_by_name(module: &Module, name: &String) -> usize 
// dl_val_by_bame: find the value of variable by name, return value in little endian bytes
pub fn dl_val_by_name(module: &Module, name: &String, bytes: usize) -> Vec<u8>
```
The `main` function provides example for loading the module in binary.rs and run function `test`. You can also run call `test_extern` to test support for extern functions, the module used here are build from `testcase/extern_symbols_1` and `testcase/extern_symbols_1a`. 

