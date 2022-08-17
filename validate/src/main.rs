use std::env::join_paths;

use clap::Parser;

#[derive(Parser, Default, Debug)]
struct Args {
    // all or case name
    #[clap(short, long)]
    casename: String,
}

fn main() {
    let args = Args::parse();
    // list ../testcase
    let mut paths: Vec<_> = std::fs::read_dir("../testcase").unwrap()
                                                    .map(|r| r.unwrap())
                                                    .collect();
    paths.sort_by_key(|r| r.path());
    
    for case in paths {
        let path = case.path();
        let name = (&path).file_name().unwrap().to_str().unwrap();    
        if args.casename != "all" && name != args.casename {
            continue;
        }
        println!("validate {}:", &name);
        // let o1 = std::process::Command::new("bash").current_dir(&path).arg("-c").arg("cargo clean").output().unwrap();
        let o2 = std::process::Command::new("bash").current_dir(&path).arg("-c").arg("cargo build -Zbuild-std=core --release").output().unwrap();
        if o2.status.success() {
            println!("\tbuild ok.");
        } else {
            println!("\tbuild failed.");
            continue;
        }
        
        let path_str = path.to_str().unwrap();
        // list path_str/target/thumbv7em-none-eabi/release/deps
        let paths: Vec<_> = std::fs::read_dir(format!("{}/target/thumbv7em-none-eabi/release/deps", path_str)).unwrap()
                                                    .map(|r| r.unwrap())
                                                    .collect();
        // starts with name and ends with .o
        let object = paths.iter().filter(|r| {
            r.path().file_name().unwrap().to_str().unwrap().starts_with(name)
        }).collect::<Vec<_>>();
        let object = object.iter().filter(|r| {
            r.path().file_name().unwrap().to_str().unwrap().ends_with(".o")
        }).collect::<Vec<_>>();
        if object.len() != 1 {
            println!("\t multiple/none object file.");
            continue;
        }
        let o3 = std::process::Command::new("bash").arg("-c")
        .arg(format!("cp {}  ../build_script/module.o", object[0].path().to_str().unwrap())).output().unwrap();
        let o4 = std::process::Command::new("bash").current_dir("../build_script").arg("-c").arg("cargo run").output().unwrap();
        if o3.status.success() && o4.status.success() {
            println!("\tdl ok.");
        } else {
            println!("\tdl failed.");
        }
    }   

}
