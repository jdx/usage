//! A plugin-aware host, end to end: discovery, completion, dispatch, and help.
//!
//! This is the example `docs/rust/dynamic-commands.md` shows, kept here so that it compiles.

use std::ffi::{OsStr, OsString};
use usage_dynamic::{Catalog, Outcome, ParseOutput, Spec};
use usage_rs::{Cli, Error, Subcommands};

#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    /// Built into ex
    Build,
    #[usage(external_subcommand)]
    External(Vec<OsString>),
}

fn load_plugin_spec(_name: &str) -> Spec {
    "name \"formatter\"\nbin \"formatter\"\n".parse().unwrap()
}
fn build() {}
fn run_plugin(_name: &str, _output: &ParseOutput) {}
fn fallback(_words: &[OsString]) {}

fn main() {
    let catalog = Catalog::builder(Ex::app())
        .root(load_plugin_spec("formatter"))
        .build()
        .unwrap();
    let app = catalog.app().unwrap();

    // A completion request is not a command anybody runs, so it is answered before the parse.
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Some(answer) = futures::executor::block_on(app.completion_request(&argv)) {
        print!("{answer}");
        return;
    }

    let words: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    match Ex::parse_from(&words) {
        Ok(Ex {
            command: Commands::Build,
        }) => build(),
        Ok(Ex {
            command: Commands::External(captured),
        }) => {
            match catalog.parse_external("", &captured) {
                Ok(Some(Outcome::Parsed(parsed))) => run_plugin(&parsed.name, &parsed.output),
                Ok(Some(Outcome::Help(help))) => print!("{}", help.page),
                Ok(Some(Outcome::Version(version))) => println!("{}", version.version),
                // `Outcome` is `#[non_exhaustive]`, so this arm is required — and it is where
                // the two cases the host answers for itself land: a name nobody catalogued,
                // and a token the spec model cannot represent. The argv is intact in both, so
                // dispatch it the way this host dispatched unrecognized words all along.
                _ => fallback(&captured),
            }
        }
        // The host's own help and version, rendered from the merged tree so that runtime
        // commands appear on the page.
        Err(Error::Help { cmd, long }) => {
            let path = usage_rs::help::find(Ex::spec(), cmd)
                .map(|(path, _)| path[1..].join(" "))
                .unwrap_or_default();
            print!("{}", app.help(&path, long).unwrap());
        }
        Err(err) => {
            eprint!("{}", Ex::render_failure(&words, &err));
            std::process::exit(2);
        }
    }
}
