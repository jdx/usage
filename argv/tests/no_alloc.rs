//! Proves the claim in the crate docs: a parse allocates nothing.
//!
//! A counting global allocator wraps the system one and can be armed around a
//! region of code. Anything the test itself needs — the argv vector, the
//! collected results — is built while disarmed, so what the counter sees is only
//! the parse.
//!
//! This is a test rather than a benchmark because "zero" is a property, not a
//! measurement. If a future refactor introduces a `to_vec()` in the hot path, no
//! benchmark threshold would catch it as reliably as this does.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use usage_argv::{Arg, Command, DoubleDash, Event, Flag, Parser};

struct Counting;

thread_local! {
    /// Armed per thread, not globally: the test harness runs each test on its own
    /// thread while the main thread waits, prints, and collects results — and a
    /// global flag counted *its* allocations too. That made this test fail
    /// intermittently depending on timing, which coverage instrumentation was
    /// enough to change.
    ///
    /// `const`-initialized so reading it cannot allocate, which inside a global
    /// allocator would recurse.
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Whether the calling thread is the one being measured.
///
/// `try_with` rather than `with`: during thread teardown the local is gone, and
/// an allocation then must not panic.
fn armed() -> bool {
    ARMED.try_with(Cell::get).unwrap_or(false)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if armed() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if armed() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Run `f` with this thread's counter armed, and report how many allocations it
/// made.
///
/// Only this thread is counted, so neither a sibling test nor the harness itself
/// can contribute — both of which have produced phantom allocations here.
fn count_allocations(f: impl FnOnce()) -> usize {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ARMED.with(|a| a.set(true));
    f();
    ARMED.with(|a| a.set(false));
    ALLOCATIONS.load(Ordering::Relaxed)
}

static QUIET: Flag = Flag {
    key: 1,
    longs: &["quiet"],
    shorts: b"q",
    global: true,
    ..Flag::BOOL
};
static JOBS: Flag = Flag {
    key: 2,
    longs: &["jobs"],
    shorts: b"j",
    global: true,
    ..Flag::VALUE
};
static FORCE: Flag = Flag {
    key: 3,
    longs: &["force"],
    shorts: b"f",
    ..Flag::BOOL
};
static COLOR: Flag = Flag {
    key: 4,
    longs: &["color"],
    negate: Some("no-color"),
    ..Flag::BOOL
};
static TOOL: Arg = Arg {
    key: 10,
    name: "tool",
    ..Arg::VAR
};
static TASK: Arg = Arg {
    key: 11,
    name: "task",
    ..Arg::REQUIRED
};
static PASSTHROUGH: Arg = Arg {
    key: 12,
    name: "args",
    double_dash: DoubleDash::Preserve,
    ..Arg::VAR
};
static SET: Command = Command {
    name: "set",
    flags: &[&FORCE],
    args: &[&TASK],
    key: 200,
    ..Command::EMPTY
};
static SETTINGS: Command = Command {
    name: "settings",
    subcommands: &[&SET],
    key: 201,
    ..Command::EMPTY
};
static INSTALL: Command = Command {
    name: "install",
    aliases: &["i"],
    flags: &[&FORCE],
    args: &[&TOOL],
    key: 202,
    ..Command::EMPTY
};
static RUN: Command = Command {
    name: "run",
    args: &[&TASK, &PASSTHROUGH],
    key: 203,
    ..Command::EMPTY
};
static ROOT: Command = Command {
    name: "ex",
    flags: &[&QUIET, &JOBS, &COLOR],
    subcommands: &[&INSTALL, &SETTINGS, &RUN],
    key: 204,
    ..Command::EMPTY
};

/// Drive a parse to completion without allocating: the events are inspected and
/// discarded rather than collected.
fn drain(argv: &[&OsStr]) -> usize {
    let mut parser = Parser::new(&ROOT, argv);
    let mut seen = 0;
    while let Some(event) = parser.next_event() {
        match event {
            Ok(Event::Command(_) | Event::Flag { .. } | Event::Arg { .. }) => seen += 1,
            Err(_) => seen += 1,
        }
    }
    seen
}

#[test]
fn parsing_never_allocates() {
    // First, prove the counter is not vacuous. If this were broken, every
    // assertion below would pass no matter what the parser did.
    let deliberate = count_allocations(|| {
        let v: Vec<u8> = Vec::with_capacity(64);
        std::hint::black_box(v);
    });
    assert!(
        deliberate > 0,
        "the allocation counter did not observe a deliberate allocation"
    );

    // Every command line is built before the counter is armed, so what it sees
    // is only the parse.
    let succeeding: Vec<Vec<&OsStr>> = vec![
        vec![],
        vec![OsStr::new("install")],
        vec![
            OsStr::new("i"),
            OsStr::new("node@20"),
            OsStr::new("go@1.22"),
        ],
        vec![OsStr::new("--quiet"), OsStr::new("install")],
        vec![OsStr::new("install"), OsStr::new("--quiet")],
        vec![OsStr::new("-qj8"), OsStr::new("install")],
        vec![OsStr::new("--jobs=8"), OsStr::new("install")],
        vec![OsStr::new("--no-color")],
        vec![OsStr::new("settings"), OsStr::new("set"), OsStr::new("x")],
        vec![
            OsStr::new("run"),
            OsStr::new("build"),
            OsStr::new("--"),
            OsStr::new("--flag-for-the-task"),
        ],
    ];

    // Errors are values carrying borrowed slices rather than formatted messages,
    // so failing does not allocate either. Rendering a failure for a human would,
    // and that is a cold path in another crate.
    let failing: Vec<Vec<&OsStr>> = vec![
        vec![OsStr::new("--nope")],
        vec![OsStr::new("--jobs")],
        vec![OsStr::new("-z")],
        vec![OsStr::new("install"), OsStr::new("--jobs")],
    ];

    for argv in succeeding.iter().chain(failing.iter()) {
        let allocations = count_allocations(|| {
            drain(argv);
        });
        assert_eq!(
            allocations, 0,
            "parsing {argv:?} allocated {allocations} time(s)"
        );
    }
}
