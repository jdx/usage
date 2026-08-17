//! Emitting Go parse tables from a spec.
//!
//! The Go side of usage has no derive macro to emit its tables, because Go has no
//! macros: what a Rust CLI gets from `#[derive(Cli)]` at compile time, a Go CLI
//! gets from this, at build time, through `go:generate`. The output is a plain Go
//! file an author checks in and a reviewer can read.
//!
//! # What it emits, and what it does not
//!
//! Binding tables only — which token becomes which flag or argument. Help text,
//! choices, defaults, `env`, and every other thing that needs a value's type are
//! deliberately absent, for the same reason they are absent from the Rust hot
//! path: a successful parse never touches them, and a table that carried them
//! would put mise's several hundred kilobytes of help strings in front of the
//! parser. They belong in a second, cold table, which is a separate piece of work.
//!
//! # Why package-level `var` and not `const`
//!
//! Go has no `const` for composite data. What it does have is a linker that
//! statically initializes package-level variables holding plain data, which is
//! the property the whole design rests on: `go tool nm` reports these symbols as
//! type `D`, and the generated package has no `init` function. So a 211-command
//! table costs bytes in the binary and no instructions at startup — the thing
//! cobra and kong each pay a million or more for.
//!
//! Commands are emitted as separate variables rather than one nested literal
//! because `default_subcommand` has to point at a node inside the tree, and a
//! composite literal cannot refer to its own interior.

use std::collections::HashMap;
use std::fmt::Write as _;

use heck::AsPascalCase;

use crate::spec::unknown_flags::UnknownFlags;
use crate::{Spec, SpecArg, SpecCommand, SpecDoubleDashChoices, SpecFlag};

/// How to emit.
#[derive(Debug, Clone, Default)]
pub struct GoOptions {
    /// The Go package clause. Defaults to the spec's `bin`, made into an
    /// identifier.
    ///
    /// Must satisfy [`is_valid_package`]. A caller taking this from a user should
    /// check it and say so; one that does not gets it sanitized, because emitting a
    /// file that cannot compile helps nobody.
    pub package: Option<String>,
}

/// Turn a spec into a Go source file declaring its parse tables.
pub fn generate(spec: &Spec, opts: &GoOptions) -> String {
    Emitter::new(spec, opts).run()
}

/// One entry's identifiers: the exported key constant, and for a command the
/// variable holding it.
struct Named {
    key: String,
    var: String,
    number: u64,
}

struct Emitter<'a> {
    spec: &'a Spec,
    package: String,
    /// Every identifier handed out, so a second entry wanting the same spelling
    /// gets a suffix instead of silently colliding.
    taken: HashMap<String, u32>,
    /// Assigned in emission order, so a key is stable as long as the spec is.
    next_key: u64,
    out: String,
}

impl<'a> Emitter<'a> {
    fn new(spec: &'a Spec, opts: &GoOptions) -> Self {
        // An explicit package that is not an identifier is sanitized rather than
        // emitted: a caller that wants to reject it should ask `is_valid_package`
        // first, which the CLI does.
        let package = match opts.package.as_deref() {
            Some(name) if is_valid_package(name) => name.to_string(),
            Some(name) => package_ident(name),
            None => package_ident(&spec.bin),
        };
        Emitter {
            spec,
            package,
            taken: HashMap::new(),
            next_key: 0,
            out: String::new(),
        }
    }

    /// Reserve an identifier, adding a numeric suffix if the spelling is taken.
    ///
    /// Collisions are ordinary rather than exotic: mise has both a `macos-defaults`
    /// command and a `macos defaults` path, and both want to be spelled
    /// `CmdMacosDefaults`.
    fn unique(&mut self, base: &str) -> String {
        let n = self.taken.entry(base.to_string()).or_insert(0);
        *n += 1;
        if *n == 1 {
            base.to_string()
        } else {
            format!("{base}{n}")
        }
    }

    fn name(&mut self, prefix: &str, path: &[&str], own: &str) -> Named {
        let mut base = String::from(prefix);
        for segment in path {
            let _ = write!(base, "{}", AsPascalCase(segment));
        }
        let _ = write!(base, "{}", AsPascalCase(own));
        let key = self.unique(&base);
        self.next_key += 1;
        Named {
            var: format!("cmd{}", &key[prefix.len()..]),
            key,
            number: self.next_key,
        }
    }

    fn run(mut self) -> String {
        // Collected first so the constants can be emitted in one block before any
        // table refers to them, which is also the order a reader wants: the names
        // they will switch on, then the data.
        let mut commands = Vec::new();
        self.collect(&self.spec.cmd.clone(), &[], true, &mut commands);

        self.header();
        self.constants(&commands);
        self.tables(&commands);

        // Each command is followed by a blank line, which leaves one at the end of
        // the file. gofmt strips it, and a generated file that is not gofmt-clean
        // is one every adopter has to run a formatter over before committing.
        let trimmed = self.out.trim_end().len();
        self.out.truncate(trimmed);
        self.out.push('\n');
        self.out
    }

    /// Walk the tree, naming everything, so that emission is a second pass with no
    /// lookaheads.
    fn collect(&mut self, cmd: &SpecCommand, path: &[&str], root: bool, out: &mut Vec<Emitted>) {
        let named = if root {
            self.next_key += 1;
            Named {
                // Claimed through the same counter as everything else, not just
                // spelled: a subcommand named `root` would otherwise be handed
                // `CmdRoot` too, and the file would declare the constant twice and
                // fail to compile.
                key: self.unique("CmdRoot"),
                var: "Root".to_string(),
                number: self.next_key,
            }
        } else {
            self.name("Cmd", &path[..path.len() - 1], path[path.len() - 1])
        };

        let flags = cmd
            .flags
            .iter()
            .map(|f| (f.clone(), self.name("Flag", path, &f.name)))
            .collect::<Vec<_>>();
        let args = cmd
            .args
            .iter()
            .map(|a| (a.clone(), self.name("Arg", path, &a.name)))
            .collect::<Vec<_>>();

        let index = out.len();
        out.push(Emitted {
            named,
            cmd: cmd.clone(),
            flags,
            args,
            subcommands: Vec::new(),
            root,
        });

        // Declaration order, not sorted: a recent change made the spec hold the
        // order a CLI declares its commands in, and a generated file that reordered
        // them would lose it for no gain — lookup is by name either way.
        let mut children = Vec::new();
        for (name, sub) in &cmd.subcommands {
            // An alias appears in `subcommands` under its own key as well as the
            // canonical name; emitting it twice would declare two commands where the
            // spec has one.
            if name != &sub.name {
                continue;
            }
            let mut child_path = path.to_vec();
            child_path.push(name);
            let at = out.len();
            self.collect(sub, &child_path, false, out);
            children.push(at);
        }
        out[index].subcommands = children;
    }

    fn header(&mut self) {
        let _ = writeln!(
            self.out,
            "// Code generated by `usage generate go`. DO NOT EDIT.\n\
             //\n\
             // Binding tables for `{}`, read by\n\
             // [github.com/jdx/usage/go/argv]. Regenerate rather than editing: the spec is\n\
             // the definition, and a hand-edit here is a difference no reviewer can see.\n\
             //\n\
             // These are package-level variables holding plain data, so the linker lays them\n\
             // out and nothing runs before main.\n\
             \n\
             package {}\n\
             \n\
             import \"github.com/jdx/usage/go/argv\"\n",
            self.spec.bin, self.package
        );

        if let Some(version) = &self.spec.version {
            let _ = writeln!(
                self.out,
                "// Version is what the spec declares, so a caller answering `--version` has it\n\
                 // without the parse tables carrying a string binding never reads.\n\
                 const Version = {}\n",
                go_string(version)
            );
        }
    }

    fn constants(&mut self, commands: &[Emitted]) {
        let _ = writeln!(
            self.out,
            "// Keys identify a table entry without a string comparison: switch on the Key an\n\
             // event carries rather than on its Name, which is there for diagnostics.\n\
             const ("
        );
        let mut entries: Vec<(&str, u64)> = Vec::new();
        for e in commands {
            entries.push((&e.named.key, e.named.number));
            entries.extend(e.flags.iter().map(|(_, n)| (n.key.as_str(), n.number)));
            entries.extend(e.args.iter().map(|(_, n)| (n.key.as_str(), n.number)));
        }
        // One run, so every name pads to the longest — which is what gofmt does to
        // a const block with no blank line in it.
        let width = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        for (key, number) in entries {
            let _ = writeln!(
                self.out,
                "\t{key}{:pad$} uint64 = {number}",
                "",
                pad = width - key.len()
            );
        }
        let _ = writeln!(self.out, ")\n");
    }

    fn tables(&mut self, commands: &[Emitted]) {
        // Resolved once, against the root's *direct* subcommands, because the spec
        // declares it once at the top and it names one of them. Searching the whole
        // tree instead is what wired mise's `default_subcommand run` to `oci run`,
        // which comes first in a depth-first walk — the parser would then have
        // descended into a command that is not the root's child at all. A name
        // nothing answers to is left unset rather than guessed at.
        let default_subcommand = self.spec.default_subcommand.as_ref().and_then(|name| {
            commands[0]
                .subcommands
                .iter()
                .map(|at| &commands[*at])
                .find(|e| {
                    &e.cmd.name == name
                        || e.cmd.aliases.contains(name)
                        || e.cmd.hidden_aliases.contains(name)
                })
                .map(|e| e.named.var.clone())
        });

        for (i, e) in commands.iter().enumerate() {
            let doc = if e.root {
                format!(
                    "// Root is the command tree for `{}`. Pass it to argv.New.",
                    self.spec.bin
                )
            } else {
                format!("// {}", e.cmd.full_cmd.join(" "))
            };
            let mut lines = vec![
                Line::Field("Name".into(), go_string(&e.cmd.name)),
                Line::Field("Key".into(), e.named.key.clone()),
            ];

            let aliases: Vec<&String> = e
                .cmd
                .aliases
                .iter()
                .chain(e.cmd.hidden_aliases.iter())
                .collect();
            if !aliases.is_empty() {
                // A hidden alias selects a command exactly as a visible one does:
                // hiding is about help output, which binding never reads.
                let list = aliases
                    .iter()
                    .map(|a| go_string(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(Line::Field("Aliases".into(), format!("[]string{{{list}}}")));
            }

            if !e.flags.is_empty() {
                let mut block = vec!["Flags: []*argv.Flag{".to_string()];
                for (flag, named) in &e.flags {
                    block.push(format!("\t{},", flag_literal(flag, named)));
                }
                block.push("},".to_string());
                lines.push(Line::Block(block));
            }

            if !e.args.is_empty() {
                let mut block = vec!["Args: []*argv.Arg{".to_string()];
                for (arg, named) in &e.args {
                    block.push(format!("\t{},", arg_literal(arg, named)));
                }
                block.push("},".to_string());
                lines.push(Line::Block(block));
            }

            if !e.subcommands.is_empty() {
                let list = e
                    .subcommands
                    .iter()
                    .map(|at| commands[*at].named.var.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(Line::Field(
                    "Subcommands".into(),
                    format!("[]*argv.Command{{{list}}}"),
                ));
            }

            // Already resolved: inheritance is the generator's job, so that the
            // parser reads one field rather than walking ancestors per token.
            if effective_unknown_flags(self.spec, commands, i) == UnknownFlags::Error {
                lines.push(Line::Field(
                    "UnknownFlags".into(),
                    "argv.UnknownFlagsError".into(),
                ));
            }

            if e.root {
                if let Some(var) = &default_subcommand {
                    lines.push(Line::Field("DefaultSubcommand".into(), var.clone()));
                }
                if self.spec.version.is_some() {
                    // Only where the CLI declares a version: a `--version` that answers
                    // with nothing is worse than one that is not there.
                    lines.push(Line::Field("Version".into(), "true".into()));
                }
            }

            let _ = writeln!(self.out, "{doc}");
            let _ = writeln!(self.out, "var {} = &argv.Command{{", e.named.var);
            render(&mut self.out, "\t", &lines);
            let _ = writeln!(self.out, "}}\n");
        }
    }
}

/// A line inside a `const` block or a composite literal.
///
/// The distinction exists only to reproduce gofmt's alignment, which pads within
/// *runs* of consecutive single-line entries and starts a new run after anything
/// that spans lines. Emitting gofmt-clean output rather than close-enough output
/// is what lets a generated file be committed as it comes out: the alternative is
/// every adopter needing a formatting step, and this repo's own CI failing
/// `gofmt -l` on the table it checks in.
enum Line {
    /// `Key: value,` — aligned against its neighbours.
    Field(String, String),
    /// Verbatim, and it breaks the run either side of it.
    Block(Vec<String>),
}

/// Render lines with gofmt's column alignment.
fn render(out: &mut String, indent: &str, lines: &[Line]) {
    let mut run: Vec<(&String, &String)> = Vec::new();

    fn flush(out: &mut String, indent: &str, run: &mut Vec<(&String, &String)>) {
        let width = run.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        for (key, value) in run.iter() {
            let _ = writeln!(
                out,
                "{indent}{key}:{:width$} {value},",
                "",
                width = width - key.len()
            );
        }
        run.clear();
    }

    for line in lines {
        match line {
            Line::Field(key, value) => run.push((key, value)),
            Line::Block(block) => {
                flush(out, indent, &mut run);
                for l in block {
                    let _ = writeln!(out, "{indent}{l}");
                }
            }
        }
    }
    flush(out, indent, &mut run);
}

/// One command, named and ready to emit.
struct Emitted {
    named: Named,
    cmd: SpecCommand,
    flags: Vec<(SpecFlag, Named)>,
    args: Vec<(SpecArg, Named)>,
    /// Indices into the flat list, in declaration order.
    subcommands: Vec<usize>,
    root: bool,
}

/// What an unrecognized flag-like token means at a command, with inheritance
/// applied.
///
/// The nearest enclosing command that states a preference wins, then the spec,
/// then `value`. Walked over `full_cmd` rather than threaded through the collect
/// pass, so that emission does not depend on the order commands happen to sit in.
fn effective_unknown_flags(spec: &Spec, commands: &[Emitted], at: usize) -> UnknownFlags {
    let path = &commands[at].cmd.full_cmd;
    for depth in (0..=path.len()).rev() {
        let ancestor = commands
            .iter()
            .find(|e| e.cmd.full_cmd.len() == depth && e.cmd.full_cmd[..] == path[..depth]);
        if let Some(mode) = ancestor.and_then(|e| e.cmd.unknown_flags) {
            return mode;
        }
    }
    spec.unknown_flags.unwrap_or_default()
}

fn flag_literal(flag: &SpecFlag, named: &Named) -> String {
    let mut fields = vec![
        format!("Key: {}", named.key),
        format!("Name: {}", go_string(&flag.name)),
    ];
    if !flag.long.is_empty() {
        let longs = flag
            .long
            .iter()
            .map(|l| go_string(l))
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(format!("Longs: []string{{{longs}}}"));
    }
    if !flag.short.is_empty() {
        let shorts = flag
            .short
            .iter()
            .map(|c| go_byte(*c))
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(format!("Shorts: []byte{{{shorts}}}"));
    }
    if let Some(negate) = &flag.negate {
        // The spec stores the negation with its dashes; the table wants the bare
        // name, since that is what the parser has after stripping the `--`.
        fields.push(format!(
            "Negate: {}",
            go_string(negate.trim_start_matches('-'))
        ));
    }
    if flag.arg.is_some() {
        fields.push("TakesValue: true".to_string());
    }
    // Only a variadic *argument* is greedy. The spec's flag-level `var` means the
    // flag may be repeated and takes one value each time, which needs nothing from
    // the parser: it reports every occurrence separately either way. Conflating the
    // two makes a merely repeatable flag greedy enough to eat a positional.
    if let Some(arg) = flag.arg.as_ref().filter(|a| a.var) {
        fields.push("Variadic: true".to_string());
        if let Some(max) = arg.var_max {
            fields.push(format!("VarMax: {}", clamp_var_max(max)));
        }
    }
    if flag.global {
        fields.push("Global: true".to_string());
    }
    format!("{{{}}}", fields.join(", "))
}

fn arg_literal(arg: &SpecArg, named: &Named) -> String {
    let mut fields = vec![
        format!("Key: {}", named.key),
        format!("Name: {}", go_string(&arg.name)),
    ];
    if arg.var {
        fields.push("Var: true".to_string());
        if let Some(max) = arg.var_max {
            fields.push(format!("VarMax: {}", clamp_var_max(max)));
        }
    }
    let double_dash = match arg.double_dash {
        SpecDoubleDashChoices::Required => Some("argv.DoubleDashRequired"),
        SpecDoubleDashChoices::Preserve => Some("argv.DoubleDashPreserve"),
        SpecDoubleDashChoices::Automatic => Some("argv.DoubleDashAutomatic"),
        _ => None,
    };
    if let Some(dd) = double_dash {
        fields.push(format!("DoubleDash: {dd}"));
    }
    format!("{{{}}}", fields.join(", "))
}

/// Zero means unbounded in the table, which is also what an absent `var_max`
/// lowers to, so the two agree. A bound past a `uint32` saturates rather than
/// wrapping: truncating four billion and one to one would read as "stop at once"
/// rather than "no real limit".
fn clamp_var_max(max: usize) -> u32 {
    u32::try_from(max).unwrap_or(u32::MAX)
}

/// Go's reserved words, which cannot be a package name.
///
/// Not hypothetical: `go`, `range`, `select`, `import` and `package` are all
/// plausible names for a CLI, and `package go` does not compile.
const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

/// Whether a string can be written after `package`.
///
/// Deliberately ASCII-only. Go itself allows a Unicode letter, but a package name
/// that needs one is a worse problem for an adopter than the restriction is.
pub fn is_valid_package(name: &str) -> bool {
    !name.is_empty()
        && !GO_KEYWORDS.contains(&name)
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A Go package identifier from a binary name: `my-cli` is not one, `mycli` is.
///
/// Only ever applied to a name derived from the spec, which the author did not
/// choose for this purpose and cannot be asked to fix. A `--package` given
/// explicitly is checked rather than mangled — see [`is_valid_package`] — because
/// silently emitting `mypkg` for someone who asked for `my-pkg` is a surprise
/// waiting in a build script.
fn package_ident(bin: &str) -> String {
    let lowered: String = bin
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase();
    if lowered.is_empty()
        || lowered.starts_with(|c: char| c.is_ascii_digit())
        || GO_KEYWORDS.contains(&lowered.as_str())
    {
        format!("cli{lowered}")
    } else {
        lowered
    }
}

/// A Go string literal.
///
/// Written out rather than borrowed from Rust's `{:?}`, which escapes to Rust's
/// rules: it spells a delete character `\u{7f}`, which Go does not accept.
fn go_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A Go byte literal for a short flag.
///
/// Non-ASCII shorts are emitted as their low byte, which can never match: a
/// cluster is walked one byte at a time. The spec is what should refuse them, and
/// silently dropping one here would be a flag that vanished.
fn go_byte(c: char) -> String {
    match c {
        '\'' => "'\\''".to_string(),
        '\\' => "'\\\\'".to_string(),
        c if c.is_ascii_graphic() => format!("'{c}'"),
        c => format!("0x{:02x}", (c as u32) & 0xff),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go(kdl: &str) -> String {
        let spec: Spec = kdl.parse().expect("the fixture spec should parse");
        generate(&spec, &GoOptions::default())
    }

    #[test]
    fn a_whole_cli() {
        let out = go(r#"
name "ex"
bin "ex"
version "1.2.3"
flag "-v --verbose" global=#true help="be loud"
flag "--color" negate="--no-color"
flag "-j --jobs <n>"
flag "--include <pattern>..." var_max=3
arg "<file>"
arg "[rest]..." var=#true
cmd "install" {
    alias "i"
    flag "-f --force"
    arg "<pkg>"
}
cmd "config" {
    cmd "ls" {
        flag "--no-header"
    }
}
"#);
        insta::assert_snapshot!(out);
    }

    /// Inheritance is resolved here so the parser reads one field per command.
    #[test]
    fn unknown_flags_are_inherited_and_overridable() {
        let out = go(r#"
name "ex"
bin "ex"
unknown_flags "error"
cmd "strict" {
    cmd "deep" {}
}
cmd "exec" unknown_flags="value" {
    cmd "nested" {}
}
"#);
        insta::assert_snapshot!(out);
    }

    /// mise declares both a `macos-defaults` command and a `macos defaults` path,
    /// and both want the same Go identifier.
    #[test]
    fn colliding_names_get_distinct_identifiers() {
        let out = go(r#"
name "ex"
bin "ex"
cmd "macos-defaults" {
    flag "--apply"
}
cmd "macos" {
    cmd "defaults" {
        flag "--apply"
    }
}
"#);
        insta::assert_snapshot!(out);
    }

    #[test]
    fn a_default_subcommand_points_into_the_tree() {
        let out = go(r#"
name "ex"
bin "ex"
default_subcommand "run"
arg "[task]"
cmd "run" {
    arg "[args]..." var=#true
}
"#);
        insta::assert_snapshot!(out);
    }

    #[test]
    fn a_bin_name_that_is_not_an_identifier_still_gives_a_package() {
        assert_eq!(package_ident("my-cli"), "mycli");
        assert_eq!(package_ident("7zip"), "cli7zip");
        assert_eq!(package_ident(""), "cli");
        // `package go` does not compile, and `go` is a plausible name for a CLI.
        assert_eq!(package_ident("go"), "cligo");
        assert_eq!(package_ident("type"), "clitype");
    }

    #[test]
    fn a_package_that_would_not_compile_is_refused_rather_than_emitted() {
        assert!(is_valid_package("mycli"));
        assert!(is_valid_package("mise_tables"));
        assert!(!is_valid_package("my-pkg"));
        assert!(!is_valid_package("7zip"));
        assert!(!is_valid_package(""));
        assert!(!is_valid_package("range"));

        // A library caller that skips the check still gets a file that compiles.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\n".parse().unwrap();
        let out = generate(
            &spec,
            &GoOptions {
                package: Some("my-pkg".into()),
            },
        );
        assert!(out.contains("package mypkg"), "{out}");
    }

    /// The bug the checked-in mise tables caught: `default_subcommand run` was
    /// wired to `oci run`, which a depth-first walk reaches first.
    ///
    /// It names a subcommand *of the root*, so nothing deeper is a candidate — and
    /// the parser would otherwise descend into a command that is not the root's
    /// child at all.
    #[test]
    fn a_default_subcommand_ignores_a_deeper_command_of_the_same_name() {
        let out = go(r#"
name "ex"
bin "ex"
default_subcommand "run"
cmd "oci" {
    cmd "run" {}
}
cmd "run" {
    arg "[args]..." var=#true
}
"#);
        assert!(
            out.contains("DefaultSubcommand: cmdRun,"),
            "should point at the root's own `run`, got:\n{out}"
        );
    }

    /// A subcommand actually named `root` wants the constant the root has.
    #[test]
    fn a_subcommand_named_root_does_not_collide_with_the_root() {
        let out = go(r#"
name "ex"
bin "ex"
cmd "root" {
    flag "--wat"
}
"#);
        // By first token, because the const block is column-aligned: matching
        // "CmdRoot uint64" would find nothing and pass for the wrong reason.
        let declared = |name: &str| {
            out.lines()
                .filter(|l| l.split_whitespace().next() == Some(name))
                .count()
        };
        assert_eq!(declared("CmdRoot"), 1, "CmdRoot declared twice:\n{out}");
        assert_eq!(declared("CmdRoot2"), 1, "no distinct key for it:\n{out}");
    }

    /// The two `var_max` are different questions, and the corpus pins them apart:
    /// on a flag's *argument* it bounds one occurrence's values and belongs in the
    /// binding table, while on the flag it counts occurrences and is checked after
    /// the parse.
    #[test]
    fn only_the_per_occurrence_bound_reaches_the_table() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--include <pattern>..." {
    arg "<pattern>..." var=#true var_max=2
}
flag "--tag <t>" var=#true var_max=1
"#);
        assert!(
            out.contains("Name: \"include\", Longs: []string{\"include\"}, TakesValue: true, Variadic: true, VarMax: 2"),
            "{out}"
        );
        let tag = out.lines().find(|l| l.contains("\"tag\"")).unwrap();
        assert!(!tag.contains("VarMax"), "occurrence bound leaked: {tag}");
    }

    #[test]
    fn strings_are_escaped_to_go_rules() {
        assert_eq!(go_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(go_string("tab\there"), r#""tab\there""#);
        // Rust would spell this `\u{7f}`, which Go rejects.
        assert_eq!(go_string("\u{7f}"), r#""\x7f""#);
    }
}
