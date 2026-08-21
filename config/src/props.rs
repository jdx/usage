//! What `#[derive(usage::Config)]` generates, and how flattened groups compose.
//!
//! A derive expansion sees one struct. A settings struct that flattens another — pitchfork
//! keeps eight groups in eight structs — therefore joins tables through this trait's
//! associated const, the same way `usage::Cli` joins a flattened group's flags: the child
//! declares its own slice, and the parent concatenates at compile time. Nothing is assembled
//! at run time, and a prop's id is its position in the joined slice.

use crate::read::Fold;
use crate::registry::PropMeta;

/// A group of settings declared in code.
///
/// Implemented by `#[derive(usage::Config)]`, not by hand: the derive is what keeps
/// [`Props::PROPS`] and [`Props::read_at`] describing the same fields in the same order,
/// which is the invariant everything here leans on.
pub trait Props: Sized {
    /// This group's settings, in declaration order.
    ///
    /// A flattened child's props follow the parent's own, so an id is a position in the
    /// parent's joined slice — which is why reading takes a `base`.
    const PROPS: &'static [PropMeta];

    /// Read this group's fields from a fold, its props starting at `base`.
    ///
    /// `None` means a field could not be read and the fold has recorded why. Every field is
    /// still visited first — the errors are a list, not the first thing found — so a caller
    /// checks [`Fold::finish`] before treating `None` as anything but "already reported".
    #[doc(hidden)]
    fn read_at(fold: &mut Fold<'_>, base: u16) -> Option<Self>;
}

/// Join groups of prop metadata into one slice, at compile time.
///
/// The settings counterpart of `usage_argv::spec::concat_flag_metas`, for the same reason: a
/// flattened struct's props belong in the parent's registry, and the parent's macro expansion
/// has only a type to reach them through.
///
/// `N` must be the summed length of `groups`. Two groups declaring the same key are refused
/// here, at compile time — the parent and the struct it flattens each declared it, a collision
/// neither expansion can see.
pub const fn concat_props<const N: usize>(groups: &[&[PropMeta]]) -> [PropMeta; N] {
    let mut out = [PropMeta::new("", crate::ty::Ty::Any); N];
    let mut at = 0;
    let mut g = 0;
    while g < groups.len() {
        let group = groups[g];
        let mut i = 0;
        while i < group.len() {
            out[at] = group[i];
            at += 1;
            i += 1;
        }
        g += 1;
    }
    assert!(
        at == N,
        "`N` must be the summed length of the groups, or the registry would describe a \
         setting that does not exist"
    );
    let mut a = 0;
    while a < N {
        let mut b = a + 1;
        while b < N {
            assert!(
                !str_eq(out[a].key, out[b].key),
                "two flattened groups declare the same setting key, so one of them could \
                 never be reached: give one of them another key or prefix"
            );
            b += 1;
        }
        a += 1;
    }
    out
}

/// Whether two strings are equal, in a const context.
const fn str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::Ty;

    #[test]
    fn groups_join_in_order_and_ids_are_positions() {
        static OWN: &[PropMeta] = &[PropMeta::new("jobs", Ty::Uint)];
        static CHILD: &[PropMeta] = &[
            PropMeta::new("task.output", Ty::String),
            PropMeta::new("task.jobs", Ty::Uint),
        ];
        const N: usize = OWN.len() + CHILD.len();
        static JOINED: [PropMeta; N] = concat_props(&[OWN, CHILD]);
        assert_eq!(JOINED[0].key, "jobs");
        assert_eq!(JOINED[1].key, "task.output");
        assert_eq!(JOINED[2].key, "task.jobs");
    }
}
