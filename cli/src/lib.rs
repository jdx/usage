#[macro_use]
extern crate log;
extern crate miette;
extern crate xx;

use std::path::PathBuf;

use miette::Result;

pub use cli::Cli;

mod cli;
pub mod env;
mod hash;
mod shebang;
mod usage_spec;

#[cfg(test)]
mod test;

pub fn run(args: &[String]) -> Result<()> {
    // trace!(
    //     "args: {:?}",
    //     args.iter().map(|s| s[..100].to_string()).collect_vec()
    // );
    // if let Some("__USAGE__") = args.get(2).map(|s| s.as_str()) {
    //     return split_script(&args[1]);
    // } else if let Some(script) = args.get(1) {
    if let Some(script) = args.get(1) {
        if script.to_lowercase() == "-v" {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(());
        } else if script == "--usage-spec" {
            return usage_spec::generate();
        } else if script == "--completions" && args.len() > 2 {
            return usage_spec::complete(args.get(2).unwrap());
        } else if script.starts_with("./") || script.starts_with('/') {
            let script: PathBuf = script.into();
            if script.starts_with("./") && script.exists() {
                return shebang::execute(&script, args);
            }
        }
    }
    let result = Cli::run(args);
    if let Err(err) = &result {
        if let Some(_err) = err.downcast_ref::<usage::error::UsageErr>() {
            eprintln!("{err:?}");
            std::process::exit(181);
        }
    };

    result
}
