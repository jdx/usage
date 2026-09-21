//! How wide the terminal is, asked of the terminal itself.
//!
//! A help page that wraps at 80 columns on a 200-column terminal is the most visible thing a
//! CLI can get wrong about its own output, and `COLUMNS` does not answer the question: no
//! POSIX shell exports it, so it is unset in almost every process that is not an interactive
//! shell's own. The only answer is to ask the kernel — `TIOCGWINSZ` on unix, the console
//! screen buffer on Windows — which is two dozen lines of FFI vendored here rather than a
//! dependency, because this crate runs on every invocation of every CLI built on it and has
//! none.
//!
//! This is the one place in the runtime that is allowed `unsafe`, which is why it is a module
//! of its own: the crate is `deny(unsafe_code)` and only this file opts out. Nothing here
//! dereferences a pointer the caller supplied or hands one out; each call fills a struct
//! this module owns and reads two integers back out of it.
//!
//! A page is rendered by whichever implementation the CLI was built with, and all of them have
//! to reach the same width on the same terminal. usage-lib calls this module rather than
//! carrying a second copy of the FFI; Go's `argv.terminalColumns` is the one twin, because it
//! cannot call this one.
#![allow(unsafe_code)]

/// The terminal's width in columns, or `None` when output is not a terminal.
///
/// Standard output is asked first and standard error second, so that a page a CLI prints to
/// stderr — the short page that accompanies a usage error — is still laid out for the
/// terminal the user is looking at when stdout has been redirected to a file.
pub fn columns() -> Option<usize> {
    const STDOUT: i32 = 1;
    const STDERR: i32 = 2;
    columns_of(STDOUT).or_else(|| columns_of(STDERR))
}

#[cfg(unix)]
fn columns_of(fd: i32) -> Option<usize> {
    use std::ffi::{c_int, c_ulong};

    /// `struct winsize`, which every unix agrees on even where the request number differs.
    #[repr(C)]
    struct Winsize {
        rows: u16,
        columns: u16,
        width_pixels: u16,
        height_pixels: u16,
    }

    // `TIOCGWINSZ` is 0x5413 on Linux's asm-generic ioctl numbering and 0x40087468 in the
    // BSD-derived numbering — which is what macOS and the BSDs use, and what the four Linux
    // architectures that kept their historical ABI use as well.
    #[cfg(all(
        target_os = "linux",
        not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc",
            target_arch = "powerpc64",
            target_arch = "sparc",
            target_arch = "sparc64",
        ))
    ))]
    const TIOCGWINSZ: c_ulong = 0x5413;
    #[cfg(target_os = "android")]
    const TIOCGWINSZ: c_ulong = 0x5413;
    #[cfg(not(any(
        target_os = "android",
        all(
            target_os = "linux",
            not(any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "mips64",
                target_arch = "mips64r6",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "sparc",
                target_arch = "sparc64",
            ))
        )
    )))]
    const TIOCGWINSZ: c_ulong = 0x4008_7468;

    unsafe extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }

    let mut size = Winsize {
        rows: 0,
        columns: 0,
        width_pixels: 0,
        height_pixels: 0,
    };
    // SAFETY: `ioctl` is handed a descriptor number and a pointer to a `winsize` this frame
    // owns and keeps alive across the call. `TIOCGWINSZ` writes that struct and nothing else,
    // and a failure — a redirected descriptor, a platform whose request number this is not —
    // returns non-zero without having written anything, which is the branch taken below.
    let result = unsafe { ioctl(fd as c_int, TIOCGWINSZ, &raw mut size) };
    if result != 0 {
        return None;
    }
    // A terminal that reports no width — some CI pseudo-terminals do — is no answer, and
    // falling through to the caller's default is better than laying a page out at zero.
    (size.columns > 0).then_some(size.columns as usize)
}

#[cfg(windows)]
fn columns_of(fd: i32) -> Option<usize> {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ScreenBufferInfo {
        size: Coord,
        cursor_position: Coord,
        attributes: u16,
        window: SmallRect,
        maximum_window_size: Coord,
    }

    // The console API names its streams with negative constants rather than descriptors.
    const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12_i32 as u32;
    const INVALID_HANDLE_VALUE: *mut c_void = -1_isize as *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn GetConsoleScreenBufferInfo(handle: *mut c_void, info: *mut ScreenBufferInfo) -> i32;
    }

    let which = match fd {
        2 => STD_ERROR_HANDLE,
        _ => STD_OUTPUT_HANDLE,
    };
    // SAFETY: `GetStdHandle` takes an integer and returns a handle the process already owns;
    // `GetConsoleScreenBufferInfo` writes the struct below, which this frame owns and keeps
    // alive across the call. A redirected stream returns `INVALID_HANDLE_VALUE` or reports
    // failure without writing, and both are handled here.
    let columns = unsafe {
        let handle = GetStdHandle(which);
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut info = ScreenBufferInfo::default();
        if GetConsoleScreenBufferInfo(handle, &raw mut info) == 0 {
            return None;
        }
        // The buffer is often taller and wider than the window; what a line has room for is
        // the visible window, inclusive of both edges.
        i32::from(info.window.right) - i32::from(info.window.left) + 1
    };
    (columns > 0).then_some(columns as usize)
}

#[cfg(not(any(unix, windows)))]
fn columns_of(_fd: i32) -> Option<usize> {
    None
}

#[cfg(test)]
mod tests {
    /// Under `cargo test` standard output is the developer's terminal or CI's pipe, so the
    /// only thing this can assert is that asking is safe and answers something sane.
    #[test]
    fn asking_the_terminal_never_answers_zero() {
        assert_ne!(super::columns(), Some(0));
    }
}
