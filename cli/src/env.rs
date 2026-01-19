pub use std::env::*;

pub fn var_true(key: &str) -> bool {
    matches!(var(key), Ok(v) if v == "1" || v == "true")
}
