//! Appending text without compiling a copy of the append at every call site.

/// Append each of `texts`.
///
/// One out-of-line loop instead of a `push_str` per piece at every call site, each of which
/// inlines its own capacity check and copy.
#[inline(never)]
pub(crate) fn push_all(out: &mut String, texts: &[&str]) {
    for text in texts {
        out.push_str(text);
    }
}

/// Append one character, out of line: a routine that appends from a dozen places would
/// otherwise carry a copy of the UTF-8 encoding and capacity check at each of them.
#[inline(never)]
pub(crate) fn push_char(out: &mut String, c: char) {
    out.push(c);
}
