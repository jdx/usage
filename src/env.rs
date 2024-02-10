// pub use std::env::*;
// use std::path::PathBuf;
//
// use once_cell::sync::Lazy;

// #[cfg(target_os = "macos")]
// pub static USAGE_BIN: Lazy<PathBuf> = Lazy::new(|| {
//     var_os("USAGE_BIN")
//         .map(PathBuf::from)
//         .or_else(|| current_exe().ok())
//         .unwrap_or_else(|| "usage".into())
// });

// /// On linux, env::current_exe() follows symlinks which causes problems when updating the CLI.
// #[cfg(not(target_os = "macos"))]
// pub static USAGE_BIN: Lazy<PathBuf> = Lazy::new(|| {
//     var_os("USAGE_BIN")
//         .map(PathBuf::from)
//         .unwrap_or_else(|| "usage".into())
// });
