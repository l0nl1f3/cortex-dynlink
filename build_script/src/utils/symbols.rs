use object::{ObjectSymbol, Symbol};

#[derive(Debug)]
pub enum SymbolType {
    Exported,
    Local,
    External,
}

pub fn get_symbol_type(symbol: Symbol) -> Option<SymbolType> {
    match (symbol.is_global(), symbol.is_undefined(), symbol.kind()) {
        (true, false, _) => Some(SymbolType::Exported),
        (true, _, _) => Some(SymbolType::External),
        (_, _, object::SymbolKind::File) => None,
        (_, _, _) => Some(SymbolType::Local),
    }
}
