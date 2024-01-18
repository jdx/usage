pub use std::env::*;

use once_cell::sync::Lazy;

pub static USAGE_CMD: Lazy<String> =
    Lazy::new(|| var("USAGE_CMD").unwrap_or_else(|_| "usage".to_string()));
