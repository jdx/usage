//! A plugin-aware host, end to end.
//!
//! This is the example `docs/rust/dynamic-commands.md` shows, kept here so it compiles. The
//! shape it demonstrates: parse first, against the static tables alone, so a built-in command
//! runs without any plugin being discovered, loaded, or parsed. Plugins load only on the paths
//! that need them — the catch-all, help, and completion.

use std::ffi::{OsStr, OsString};
use usage_dynamic::{Catalog, Outcome, ParseOutput, Spec};
use usage_rs::complete::{render, CompletionRequest};
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

/// Discovery is the application's: scan a directory, read a lockfile, whatever fits.
fn plugin_catalog() -> Catalog<'static> {
    let mut builder = Catalog::builder(Ex::app());
    for spec in discover_plugin_specs() {
        builder = builder.root(spec);
    }
    builder.build().unwrap()
}

fn discover_plugin_specs() -> Vec<Spec> {
    vec!["name \"formatter\"\nbin \"formatter\"\n".parse().unwrap()]
}
fn build() {}
fn run_plugin(_name: &str, _output: &ParseOutput) {}
fn fallback(_words: &[OsString]) {}

fn main() {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();

    // A completion request is not a command anybody runs, so it is recognized before the
    // parse. It is also interactive: loading plugins here is affordable, and it is what puts
    // their names in the answer.
    if let Some(request) = CompletionRequest::parse(&argv) {
        let catalog = plugin_catalog();
        let answer = futures::executor::block_on(catalog.app().unwrap().complete_request(&request));
        print!("{}", render(&answer, request.shell));
        return;
    }

    let words: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    match Ex::parse_from(&words) {
        // A built-in command runs with no plugin loaded. This is the hot path, and nothing on
        // it knows plugins exist.
        Ok(Ex {
            command: Commands::Build,
        }) => build(),

        // The catch-all fired: now, and only now, load plugins.
        Ok(Ex {
            command: Commands::External(captured),
        }) => {
            let catalog = plugin_catalog();
            match catalog.parse_external("", &captured) {
                Ok(Some(Outcome::Parsed(parsed))) => run_plugin(&parsed.name, &parsed.output),
                Ok(Some(Outcome::Help(help))) => print!("{}", help.page),
                Ok(Some(Outcome::Version(version))) => println!("{}", version.version),
                // A name no loaded plugin answers to, or argv the spec model cannot
                // represent. The words are untouched either way: handle them however this
                // application handled unrecognized commands before it had plugins.
                _ => fallback(&captured),
            }
        }

        // Help is a cold path too. Render it through the catalog so plugins appear on the
        // page; `usage::help::find` turns the command the parser stopped at into the path
        // `help` takes.
        Err(Error::Help { cmd, long }) => {
            let catalog = plugin_catalog();
            let path = usage_rs::help::find(Ex::spec(), cmd)
                .map(|(path, _)| path[1..].join(" "))
                .unwrap_or_default();
            print!("{}", catalog.app().unwrap().help(&path, long).unwrap());
        }
        Err(Error::Version { .. }) => println!("ex {}", env!("CARGO_PKG_VERSION")),
        Err(err) => {
            eprint!("{}", Ex::render_failure(&words, &err));
            std::process::exit(2);
        }
    }
}
