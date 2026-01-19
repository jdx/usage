pub use std::string::*;

pub(crate) fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_line_single_line() {
        assert_eq!(first_line("hello world"), "hello world");
    }

    #[test]
    fn test_first_line_multiple_lines() {
        assert_eq!(first_line("first\nsecond\nthird"), "first");
    }

    #[test]
    fn test_first_line_empty() {
        assert_eq!(first_line(""), "");
    }

    #[test]
    fn test_first_line_only_newline() {
        assert_eq!(first_line("\n"), "");
    }

    #[test]
    fn test_first_line_crlf() {
        assert_eq!(first_line("first\r\nsecond"), "first");
    }
}
