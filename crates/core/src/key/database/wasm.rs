use crate::sql::Ident;

pub fn new(ns: &str, db: &str, wasm: &Ident, version: &str) -> Vec<u8> {
    [&[b'k'], ns.as_bytes(), db.as_bytes(), &[b'z'], wasm.as_bytes(), version.as_bytes()].concat()
}

pub fn list(ns: &str, db: &str) -> Vec<u8> {
    [&[b'k'], ns.as_bytes(), db.as_bytes(), &[b'z']].concat()
}

pub fn binary_key(ns: &str, db: &str, wasm: &Ident, version: &str) -> Vec<u8> {
    // Construct key with a distinct suffix for the binary blob
    [&[b'k'], ns.as_bytes(), db.as_bytes(), &[b'z'], wasm.as_bytes(), version.as_bytes(), b"_bin"].concat()
}
