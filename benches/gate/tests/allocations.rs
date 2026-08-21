//! How many times a parse reaches the allocator, at mise's scale.
//!
//! usage-argv's own test proves it allocates *nothing*; this is the layer above, where a
//! value becomes a `String` and a command becomes a struct. That layer has to allocate, so
//! the numbers here are a measurement rather than a zero — but the shape of them is a
//! property worth pinning: a parse should cost what the *invocation* binds, and nothing
//! for the 210 commands it did not go near.
//!
//! Run with `--nocapture` for the numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::OsStr;

use clap::Parser as _;
use shadow_mise::Cli;

struct Counting;

thread_local! {
    /// Armed per thread, and counted per thread with it.
    ///
    /// Both halves have to be thread-local: the harness runs tests in parallel, so a
    /// global count picked up whatever another test was allocating at the time. That read
    /// as a parse allocating 24 times when it allocates 4, intermittently, depending on
    /// which tests overlapped.
    ///
    /// `const`-initialized so reading them cannot allocate, which inside a global
    /// allocator would recurse.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Whether the calling thread is the one being measured.
///
/// `try_with` rather than `with`: during thread teardown the local is gone, and an
/// allocation then must not panic.
fn armed() -> bool {
    ARMED.try_with(Cell::get).unwrap_or(false)
}

fn tally() {
    if armed() {
        let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        tally();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        tally();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Allocations made while `f` runs, on this thread.
fn count(f: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|n| n.set(0));
    ARMED.with(|a| a.set(true));
    f();
    ARMED.with(|a| a.set(false));
    ALLOCATIONS.with(Cell::get)
}

/// The fewest allocations a parse of these words was seen to make.
///
/// Warmed first and then measured several times, taking the least. The first parse on a
/// fresh thread also pays for whatever the standard library and the allocator set up on
/// first use — enough to read as 60 allocations rather than 3, which is what an earlier
/// version of this test reported before it warmed up. None of that is the parser's, and
/// none of it is paid twice.
fn parsing(words: &[&str]) -> usize {
    let argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let once = |argv: &Vec<&OsStr>| {
        Cli::parse_from(argv).expect("should parse");
    };
    for _ in 0..3 {
        count(|| once(&argv));
    }
    (0..3).map(|_| count(|| once(&argv))).min().unwrap_or(0)
}

/// Allocations made while attempting a parse, including an intentional terminal response.
fn attempting(words: &[&str]) -> usize {
    let argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let once = |argv: &Vec<&OsStr>| drop(Cli::parse_from(argv));
    for _ in 0..3 {
        count(|| once(&argv));
    }
    (0..3).map(|_| count(|| once(&argv))).min().unwrap_or(0)
}

#[test]
fn nothing_bound_allocates_nothing() {
    // The strongest form of the property: 211 commands, 711 flags, and a command line with
    // no words in it costs the allocator nothing at all. mise intentionally answers that line
    // with `MissingArgsHelp`; the error only borrows the static command table, so there is still
    // no tree to build and no per-command state to fill in.
    let bare = attempting(&[]);
    println!("allocations, bare `mise`: {bare}");
    assert_eq!(
        bare, 0,
        "a parse with nothing to bind should allocate nothing"
    );
}

#[test]
fn a_parse_costs_what_it_binds() {
    // Two invocations of different depth, so what is measured is the words rather than how
    // far into the command tree they reach.
    let shallow = parsing(&["use", "-g", "node@20"]);
    let deep = parsing(&["settings", "set", "experimental", "true"]);
    println!("allocations: `use -g node@20` {shallow}, `settings set …` {deep}");

    // Loose on purpose: what this catches is a clone-per-flag or a per-command
    // allocation creeping in, not a change of one or two in how a value is stored.
    for (what, n) in [("use", shallow), ("settings set", deep)] {
        assert!(
            n <= 16,
            "{n} allocations to bind a handful of words in `{what}` is more than that needs"
        );
    }
}

#[test]
fn clap_pays_for_the_whole_cli() {
    // The comparison the gate is for. clap builds its command tree on the way to parsing,
    // and the tree is the CLI: 211 commands' worth of `String`s and `Vec`s, every time.
    let words = ["mise", "use", "-g", "node@20"];
    let argv: Vec<std::ffi::OsString> = words.iter().map(std::ffi::OsString::from).collect();
    let once = |argv: &Vec<std::ffi::OsString>| {
        shadow_mise_clap::Cli::try_parse_from(argv).expect("should parse");
    };
    for _ in 0..3 {
        count(|| once(&argv));
    }
    let clap = (0..3)
        .map(|_| count(|| once(&argv)))
        .min()
        .expect("three runs");
    println!("allocations, clap `use -g node@20`: {clap}");

    // No upper bound worth asserting: the number is what it is, and the point is its order
    // of magnitude against the handful above. The lower bound keeps this test honest if
    // the shadow ever stops building its tree at runtime, which would make the comparison
    // meaningless rather than favourable.
    assert!(
        clap > 1_000,
        "{clap} allocations is too few for a tree of 211 commands — is this measuring what \
         it claims to?"
    );
}
