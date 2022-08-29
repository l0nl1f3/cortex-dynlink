use object::{Object, ObjectSymbol};
use std::error::Error;
use std::fs;

// pub fn get_symbols(obj:&String) -> Result<HashMap<String, HashMap<String, u64> >, Box<dyn Error>> {
//   let bin_data = fs::read(obj)?;
//   let obj_file = object::File::parse(&*bin_data)?;
//   let mut results: HashMap<String, HashMap<String, u64> > = HashMap::new();
//   for symbol in obj_file.symbols() {
//     let symbol_info: HashMap<String, u64> = HashMap::new();
//     symbol_info.insert("type".to_string(), symbol.st_info());
//   }
//   Ok(results)
// }

pub fn get_pub_funcs(obj: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let bin_data = fs::read(obj)?;
    let obj_file = object::File::parse(&*bin_data)?;
    let mut results: Vec<String> = Vec::new();
    for sym in obj_file.symbols() {
        let is_global = sym.is_global();
        let is_function = sym.kind() == object::SymbolKind::Text;
        if is_global && is_function {
            results.push(sym.name().unwrap().to_string());
            // println!("{:?}", sym.name());
        }
    }
    Ok(results)
}
