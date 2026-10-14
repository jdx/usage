//! Appending text without compiling a copy of the append at every call site.

/// Append each of `texts`.
///
/// One out-of-line loop instead of a `push_str` per piece at every call site, each of which
/// inlines its own capacity check and copy.
#[cfg(any(feature = "complete", feature = "diagnostics"))]
#[inline(never)]
pub(crate) fn push_all(out: &mut String, texts: &[&str]) {
    for text in texts {
        out.push_str(text);
    }
}

/// Append one character, out of line: a routine that appends from a dozen places would
/// otherwise carry a copy of the UTF-8 encoding and capacity check at each of them.
#[cfg(feature = "complete")]
#[inline(never)]
pub(crate) fn push_char(out: &mut String, c: char) {
    out.push(c);
}

/// How many characters `text` holds, counted in one place. `str::chars().count()` expands at
/// each call to a widened byte-counting loop, and help counts widths from a dozen of them.
#[inline(never)]
pub(crate) fn char_count(text: &str) -> usize {
    text.chars().count()
}

/// Whether two names are the same ignoring ASCII case. One copy of the comparison for every
/// place a completer or a type is looked up by name, which is a dozen of them.
#[cfg(feature = "complete")]
#[inline(never)]
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `text` with every `from` replaced by `to`. One copy of the search for the three places a
/// completion answer rewrites a character, rather than a `str::replace` expanded at each.
#[cfg(feature = "complete")]
#[inline(never)]
pub(crate) fn replace_char(text: &str, from: char, to: &str) -> String {
    text.replace(from, to)
}

/// `text` cut at its first `=`, which is not part of either half. One copy of the search for the
/// places a `--name=value` word is taken apart, rather than a character search expanded at each.
#[cfg(feature = "complete")]
#[inline(never)]
pub(crate) fn split_once_eq(text: &str) -> Option<(&str, &str)> {
    // Bytes, because `=` is one: a multi-byte character never contains it.
    let at = text.bytes().position(|byte| byte == b'=')?;
    Some((&text[..at], &text[at + 1..]))
}
