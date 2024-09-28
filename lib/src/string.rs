pub use std::string::*;

pub(crate) fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_string()
}
