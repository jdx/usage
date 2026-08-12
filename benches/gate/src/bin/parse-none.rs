//! Everything the other gate binaries do except parse.
//!
//! Subtracting this is what turns a process measurement into a parser measurement:
//! roughly half of a small binary's instructions are the dynamic loader and libc
//! starting up, and neither parser is responsible for those.

use std::ffi::OsString;

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    println!("{}", !refs.is_empty());
}
