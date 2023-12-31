#[macro_use]
extern crate log;

use std::path::PathBuf;

use miette::Result;

use cli::Cli;

mod cli;
mod env;
mod hash;
mod shebang;

fn main() -> Result<()> {
    env_logger::init();

    let args = std::env::args().collect::<Vec<String>>();
    // if let Some("__USAGE__") = args.get(2).map(|s| s.as_str()) {
    //     return split_script(&args[1]);
    // } else if let Some(script) = args.get(1) {
    if let Some(script) = args.get(1) {
        if script.starts_with("./") || script.starts_with("/") {
            let script: PathBuf = script.into();
            if script.starts_with("./") && script.exists() {
                return shebang::execute(&script, &args);
            }
        }
    }
    Cli::run(&args)
}
