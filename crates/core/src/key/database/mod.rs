pub mod ac;
pub mod access;
pub mod all;
pub mod ap;
pub mod az;
pub mod cg;
pub mod fc;
pub mod ml;
pub mod pa;
pub mod tb;
pub mod ti;
pub mod ts;
pub mod us;
pub mod vs;
pub mod wasm;

pub fn ml(ns: &str, db: &str) -> Vec<u8> {
    [&[b'k'], ns.as_bytes(), db.as_bytes(), &[b'y']].concat()
}

pub fn wasm(ns: &str, db: &str) -> Vec<u8> {
    [&[b'k'], ns.as_bytes(), db.as_bytes(), &[b'z']].concat()
}

pub fn api(ns: &str, db: &str) -> Vec<u8> {
    [&[b'k'], ns.as_bytes(), db.as_bytes(), &[b'a']].concat()
}
