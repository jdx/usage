use once_cell::sync::Lazy;
use std::path::PathBuf;

pub use std::env::*;

pub static HOME: Lazy<PathBuf> = Lazy::new(|| {
    var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
});
pub static XDG_CACHE_HOME: Lazy<PathBuf> = Lazy::new(|| {
    var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| HOME.join(".cache"))
});
pub static CACHE_DIR: Lazy<PathBuf> = Lazy::new(|| XDG_CACHE_HOME.join("usage"));

pub fn var_true(key: &str) -> bool {
    matches!(var(key), Ok(v) if v == "1" || v == "true")
}
