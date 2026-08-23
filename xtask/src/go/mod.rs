//! Turning a spec into Go programs that declare the same CLI in another framework.
//!
//! One emitter per framework, and everything they share lives here: reading the
//! spec, quoting Go, and the report of what a framework could not express.
//!
//! Why generate them at all. usage-go's numbers are taken against tables generated
//! from mise's committed spec, and a comparison against a program somebody wrote by
//! hand is a comparison between two transcriptions rather than between two parsers —
//! the hand-written cobra program these replaced was a third of mise's size, and read
//! as though cobra were three times faster than it is. Generated from one spec by one
//! traversal, every row of `go/README.md`'s table describes the same CLI.
//!
//! Emitted as Go rather than through `shadow.rs`, which writes Rust: cobra and
//! urfave build their trees with statements, kong wants a struct per command, and
//! threading a language through that emitter would obscure all of it.
//!
//! What a framework cannot express is counted and printed rather than passed over.
//! A shadow that quietly dropped half the spec would measure a smaller CLI and
//! flatter the framework it was declaring.

use super::*;

pub mod cobra;
pub mod kong;
pub mod urfave;

/// Which framework's vocabulary to write the CLI in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Cobra,
    Urfave,
    Kong,
}

impl Dialect {
    pub fn as_str(self) -> &'static str {
        match self {
            Dialect::Cobra => "cobra",
            Dialect::Urfave => "urfave",
            Dialect::Kong => "kong",
        }
    }

    /// The file each emitter writes, named after the package rather than `main.go`:
    /// these are library packages, imported by one benchmark binary that links all
    /// four frameworks and times them in the same process.
    fn file(self) -> &'static str {
        match self {
            Dialect::Cobra => "cobra.go",
            Dialect::Urfave => "urfave.go",
            Dialect::Kong => "kong.go",
        }
    }
}

/// Write a shadow of `spec_path`'s CLI, in `dialect`, into `out_dir`.
pub fn generate(spec_path: &Path, out_dir: &Path, dialect: Dialect) {
    let kdl = match std::fs::read_to_string(spec_path) {
        Ok(kdl) => kdl,
        Err(e) => fail(&format!("reading {}: {e}", spec_path.display())),
    };
    let spec: Spec = match kdl.parse() {
        Ok(spec) => spec,
        Err(e) => fail(&format!("parsing {}: {e}", spec_path.display())),
    };

    let mut skipped = Skipped::default();
    let source = match dialect {
        Dialect::Cobra => cobra::render(&spec, spec_path, &mut skipped),
        Dialect::Urfave => urfave::render(&spec, spec_path, &mut skipped),
        Dialect::Kong => kong::render(&spec, spec_path, &mut skipped),
    };

    if let Err(e) = std::fs::create_dir_all(out_dir) {
        fail(&format!("creating {}: {e}", out_dir.display()));
    }
    let path = out_dir.join(dialect.file());
    if let Err(e) = std::fs::write(&path, source) {
        fail(&format!("writing {}: {e}", path.display()));
    }
    println!("{} shadow: {}", dialect.as_str(), path.display());
    skipped.report(dialect);
}

/// What a spec property turned into, or why it did not.
///
/// Collected rather than warned about one at a time: a spec of mise's size drops
/// enough on the floor that a reader needs the totals, and silence would read as
/// "everything was expressible".
#[derive(Default)]
pub struct Skipped {
    counts: BTreeMap<&'static str, usize>,
}

impl Skipped {
    pub fn note(&mut self, what: &'static str) {
        *self.counts.entry(what).or_default() += 1;
    }

    fn report(&self, dialect: Dialect) {
        let what = dialect.as_str();
        if self.counts.is_empty() {
            println!("  nothing dropped: {what} expressed the whole spec");
            return;
        }
        println!("  dropped, because {what} cannot express it:");
        for (what, n) in &self.counts {
            println!("    {what}: {n}");
        }
    }
}

/// The subcommands of `cmd`, in declaration order, without the alias keys.
///
/// An alias is keyed beside the canonical name in that map, and declaring the command
/// once per key would say the tree is bigger than it is.
pub fn subcommands(cmd: &SpecCommand) -> impl Iterator<Item = (&String, &SpecCommand)> {
    cmd.subcommands
        .iter()
        .filter(|(name, sub)| *name == &sub.name)
}

/// A Go string literal.
pub fn go_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A `[]string{…}` literal.
pub fn string_slice(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| go_string(s)).collect();
    format!("[]string{{{}}}", quoted.join(", "))
}
