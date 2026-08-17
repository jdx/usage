//! Dumping usage-lib's rendered help pages, so another implementation can be
//! measured against them.
//!
//! `usage-go` renders help from static tables where usage-lib renders it from a
//! spec through a template. Reimplemented rules drift, so the Go suite compares
//! all of mise's pages against usage-lib's — the same standard
//! `benches/gate/tests/help.rs` holds usage-argv to, which can call
//! [`usage::docs::cli::render_help`] directly because it is Rust.
//!
//! A Go test cannot. It could shell out to the CLI and strip the frame miette
//! draws around a help page delivered as an error, but a reference that *might*
//! have been reflowed on its way out is worse than no reference at all: the test
//! would fail, and the obvious response would be to change the implementation to
//! match text nobody rendered. So the pages are dumped here, once, unwrapped and
//! in one pass over the tree.
//!
//! This lives in xtask rather than in the CLI on purpose. It is a maintainer's
//! tool for checking one implementation against another, not an output format
//! anybody asked for.

use std::collections::BTreeMap;
use std::path::Path;

use usage::{Spec, SpecCommand};

/// One command's two pages.
#[derive(serde::Serialize)]
struct Pages {
    /// What `-h` prints.
    short: String,
    /// What `--help` prints.
    long: String,
}

/// Write every command's pages as JSON, keyed by the path a user would type,
/// with the root under the empty string.
pub fn dump(spec_path: &Path) {
    let kdl = match std::fs::read_to_string(spec_path) {
        Ok(kdl) => kdl,
        Err(e) => super::fail(&format!("reading {}: {e}", spec_path.display())),
    };
    let spec: Spec = match kdl.parse() {
        Ok(spec) => spec,
        Err(e) => super::fail(&format!("parsing {}: {e}", spec_path.display())),
    };

    // This JSON is a checked-in oracle, so its wrapping must not depend on the
    // terminal where a maintainer regenerates it. Keep this in sync with
    // usage-lib's default width.
    std::env::set_var("COLUMNS", "80");

    let mut out: BTreeMap<String, Pages> = BTreeMap::new();
    walk(&spec, &spec.cmd, &mut Vec::new(), &mut out);

    match serde_json::to_string_pretty(&out) {
        Ok(json) => println!("{json}"),
        Err(e) => super::fail(&format!("serializing the pages: {e}")),
    }
}

fn walk(spec: &Spec, cmd: &SpecCommand, path: &mut Vec<String>, out: &mut BTreeMap<String, Pages>) {
    out.insert(
        path.join(" "),
        Pages {
            short: usage::docs::cli::render_help(spec, cmd, false),
            long: usage::docs::cli::render_help(spec, cmd, true),
        },
    );
    for (name, sub) in &cmd.subcommands {
        // An alias is keyed beside the canonical name in this map; the page belongs
        // to the command, and rendering it twice would say the tree is bigger than
        // it is.
        if name != &sub.name {
            continue;
        }
        path.push(name.clone());
        walk(spec, sub, path, out);
        path.pop();
    }
}
