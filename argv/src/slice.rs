//! Sub-slices that cannot panic.
//!
//! Every `&s[n..]` on a path that runs for each token carries a panic location, a call to
//! the bounds-failure routine and the string naming the file, about sixty bytes apiece.
//! The parser's indices are all in range by construction, so these say the same thing
//! without the failure path: an index past the end gives the empty slice.

/// `&s[n..]`, empty when `n` is past the end.
#[inline(always)]
pub(crate) fn after<T>(s: &[T], n: usize) -> &[T] {
    match s.get(n..) {
        Some(rest) => rest,
        None => &[],
    }
}

/// `&s[..n]`, the whole slice when `n` is past the end.
#[inline(always)]
pub(crate) fn before<T>(s: &[T], n: usize) -> &[T] {
    match s.get(..n) {
        Some(head) => head,
        None => s,
    }
}
