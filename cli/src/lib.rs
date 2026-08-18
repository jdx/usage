#[macro_use]
extern crate log;
extern crate miette;
extern crate xx;

use miette::Result;

/// The reference implementation's completion candidates, readable as data.
///
/// Re-exported rather than the whole `cli` module: the conformance comparison needs this one
/// answer, and nothing else in here is a promise to anybody.
pub use cli::complete_word::{
    answer as complete_answer, candidates as complete_candidates, CandidateAnswer,
};
pub use cli::Cli;

mod cli;
// Nothing but coverage now: each command declares its own effect, so what is left is the
// check that none of them forgot to.
#[cfg(test)]
mod command_effects;
pub mod env;
mod schema;
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
            // The same line `--version` prints, from the same place. These used to be two
            // copies of the crate name, which is not what this binary is called.
            println!("{}", cli::version());
            return Ok(());
        } else if script == "--usage-spec" {
            return usage_spec::generate();
        } else if script == "--completions" && args.len() > 2 {
            return usage_spec::complete(args.get(2).unwrap());
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
