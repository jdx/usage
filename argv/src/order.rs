//! One sort for every list the cold paths put in order.
//!
//! Help rows, completion candidates and "did you mean" suggestions are short lists sorted once
//! per process. `slice::sort_*` would instantiate a full pattern-defeating quicksort for each
//! element type and comparator, several kilobytes apiece, in every binary that renders help.
//! Here the merge runs over indices behind a `dyn` comparator, so there is one copy of it however
//! many element types are sorted, and each type pays only for a loop of swaps.

use core::cmp::Ordering;

/// Sort `items` by `compare`, keeping equal items in their original order.
#[inline(never)]
pub(crate) fn sort_by<T>(items: &mut [T], compare: &mut dyn FnMut(&T, &T) -> Ordering) {
    let order = {
        let items = &*items;
        stable_order(items.len(), &mut |a, b| compare(&items[a], &items[b]))
    };
    permute(items, order);
}

/// The positions of `len` items in sorted order: a bottom-up merge sort over indices.
#[inline(never)]
fn stable_order(len: usize, compare: &mut dyn FnMut(usize, usize) -> Ordering) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    let mut merged = order.clone();
    let mut width = 1;
    while width < len {
        let mut start = 0;
        while start < len {
            let mid = (start + width).min(len);
            let end = (start + 2 * width).min(len);
            let (mut left, mut right) = (start, mid);
            for slot in &mut merged[start..end] {
                // Left wins ties, which is what keeps the sort stable.
                let take_left =
                    right == end || (left < mid && compare(order[left], order[right]).is_le());
                if take_left {
                    *slot = order[left];
                    left += 1;
                } else {
                    *slot = order[right];
                    right += 1;
                }
            }
            start = end;
        }
        core::mem::swap(&mut order, &mut merged);
        width *= 2;
    }
    order
}

/// Move the item at `order[k]` to position `k`, following each cycle with swaps.
fn permute<T>(items: &mut [T], mut order: Vec<usize>) {
    const DONE: usize = usize::MAX;
    for start in 0..items.len() {
        if order[start] == DONE {
            continue;
        }
        let mut at = start;
        loop {
            let from = order[at];
            order[at] = DONE;
            if from == start {
                break;
            }
            items.swap(at, from);
            at = from;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted<T: Clone>(items: &[T], mut compare: impl FnMut(&T, &T) -> Ordering) -> Vec<T> {
        let mut out = items.to_vec();
        sort_by(&mut out, &mut compare);
        out
    }

    #[test]
    fn agrees_with_the_standard_stable_sort() {
        // A cheap deterministic generator, so every length and duplicate pattern is exercised
        // without a dependency.
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        for len in 0..70 {
            let items: Vec<(u8, usize)> = (0..len)
                .map(|position| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    ((seed % 5) as u8, position)
                })
                .collect();
            let mut expected = items.clone();
            expected.sort_by_key(|item| item.0);
            assert_eq!(sorted(&items, |a, b| a.0.cmp(&b.0)), expected, "{len}");
        }
    }

    #[test]
    fn sorts_items_that_are_not_copy() {
        let items = ["pear", "apple", "fig", "apple"].map(String::from);
        assert_eq!(
            sorted(&items, |a, b| b.len().cmp(&a.len())),
            ["apple", "apple", "pear", "fig"]
        );
    }
}
