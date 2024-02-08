#[macro_use]
extern crate log;
#[macro_use]
extern crate miette;
#[macro_use]
extern crate xx;

use std::path::PathBuf;

use miette::Result;

use cli::Cli;

mod cli;
mod env;
mod errors;
mod hash;
mod shebang;

pub fn run(args: &[String]) -> Result<()> {
    trace!("args: {:?}", args);
    // if let Some("__USAGE__") = args.get(2).map(|s| s.as_str()) {
    //     return split_script(&args[1]);
    // } else if let Some(script) = args.get(1) {
    if let Some(script) = args.get(1) {
        if script.starts_with("./") || script.starts_with('/') {
            let script: PathBuf = script.into();
            if script.starts_with("./") && script.exists() {
                return shebang::execute(&script, args);
            }
        }
    }
    Cli::run(args)
}
