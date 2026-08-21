//! Emitting Go parse tables from a spec.
//!
//! The Go side of usage has no derive macro to emit its tables, because Go has no
//! macros: what a Rust CLI gets from `#[derive(Cli)]` at compile time, a Go CLI
//! gets from this, at build time, through `go:generate`. The output is a plain Go
//! file an author checks in and a reviewer can read.
//!
//! # What it emits, and what it does not
//!
//! Two tables, kept apart on purpose. The hot one is what binding reads: which
//! token becomes which flag or argument, and nothing else. The cold one — `Meta` —
//! carries what the rules decided after the last token need: `required`,
//! `choices`, `default`, `env`, the var bounds, and the four that compare one
//! entry against another. A parse never touches the second.
//!
//! Help text is in neither. mise's runs to several hundred kilobytes, and a table
//! carrying it would put all of that in front of the parser; rendering help is its
//! own cold table and its own piece of work.
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

mod structs;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use heck::AsPascalCase;

use crate::spec::unknown_flags::UnknownFlags;
use crate::{Spec, SpecArg, SpecChoices, SpecCommand, SpecDoubleDashChoices, SpecFlag};

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
    ///
    /// The suffixed spelling is reserved too, and the loop is what makes that
    /// safe. Counting alone was not enough: `macos-defaults` and `macos defaults`
    /// produce `CmdMacosDefaults` and `CmdMacosDefaults2`, and a third command
    /// named `macos-defaults2` asks for `CmdMacosDefaults2` directly — which was
    /// unclaimed, so the file declared it twice and did not compile.
    fn unique(&mut self, base: &str) -> String {
        let mut n = self.taken.get(base).copied().unwrap_or(0);
        loop {
            n += 1;
            let candidate = if n == 1 {
                base.to_string()
            } else {
                format!("{base}{n}")
            };
            if !self.taken.contains_key(&candidate) {
                self.taken.insert(base.to_string(), n);
                // The spelling itself, so a later entry that asks for it by name is
                // suffixed rather than handed a duplicate.
                self.taken.entry(candidate.clone()).or_insert(0);
                return candidate;
            }
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
        self.metadata(&commands);
        self.help_table(&commands);
        structs::emit(&mut self.out, &commands);

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

        if let Some(version) = self
            .spec
            .version
            .as_ref()
            .or(self.spec.long_version.as_ref())
        {
            let _ = writeln!(
                self.out,
                "// Version is what the spec declares, so a caller answering `--version` has it\n\
                 // without the parse tables carrying a string binding never reads.\n\
                 const Version = {}\n",
                go_string(version)
            );
        }
        if let Some(version) = &self.spec.long_version {
            let _ = writeln!(
                self.out,
                "// LongVersion is the extended text printed for `--version`; `-V` uses Version.\n\
                 const LongVersion = {}\n",
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
            let direct = || commands[0].subcommands.iter().map(|at| &commands[*at]);
            // Names before aliases, as the grammar says: a command's own name outranks
            // another command's alias, so which one this resolves to does not depend on
            // the order the spec declares them in.
            direct()
                .find(|e| &e.cmd.name == name)
                .or_else(|| {
                    direct().find(|e| {
                        e.cmd.aliases.contains(name) || e.cmd.hidden_aliases.contains(name)
                    })
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

            if e.cmd.external_subcommand {
                lines.push(Line::Field("ExternalSubcommand".into(), "true".into()));
            }
            if e.cmd.arg_required_else_help {
                lines.push(Line::Field("ArgRequiredElseHelp".into(), "true".into()));
            }
            if e.cmd.subcommand_negates_reqs {
                lines.push(Line::Field("SubcommandNegatesReqs".into(), "true".into()));
            }
            if e.cmd.args_conflicts_with_subcommands {
                lines.push(Line::Field(
                    "ArgsConflictWithSubcommands".into(),
                    "true".into(),
                ));
            }
            if e.cmd.subcommand_precedence_over_arg {
                lines.push(Line::Field(
                    "SubcommandPrecedenceOverArg".into(),
                    "true".into(),
                ));
            }
            if e.cmd.allow_missing_positional {
                lines.push(Line::Field("AllowMissingPositional".into(), "true".into()));
            }
            if e.cmd.dont_delimit_trailing_values {
                lines.push(Line::Field(
                    "DontDelimitTrailingValues".into(),
                    "true".into(),
                ));
            }
            if e.root {
                if let Some(var) = &default_subcommand {
                    lines.push(Line::Field("DefaultSubcommand".into(), var.clone()));
                }
                if self.spec.version.is_some() || self.spec.long_version.is_some() {
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

impl Emitter<'_> {
    /// Emit the cold table: everything binding deliberately does not know.
    ///
    /// Indexed by key, which is what makes a lookup an index rather than a map —
    /// and a Go map would have to be built at init, which is the one thing these
    /// tables are for avoiding. Keys are handed out to commands as well as to
    /// flags and arguments, and a command has no cold half, so its slot is an
    /// empty entry rather than a gap: `Metadata.Lookup` checks the key it finds
    /// and reports nothing when it does not match, so an empty slot answers
    /// correctly and the index stays dense.
    fn metadata(&mut self, commands: &[Emitted]) {
        // By key, so the slice can be written in one pass in index order.
        let mut by_key: BTreeMap<u64, String> = BTreeMap::new();
        for e in commands {
            for (flag, named) in &e.flags {
                by_key.insert(named.number, self.flag_meta(flag, named, e, commands));
            }
            for (arg, named) in &e.args {
                by_key.insert(named.number, arg_meta(self.spec, arg, named, e, commands));
            }
        }

        let total = commands
            .iter()
            .map(|e| 1 + e.flags.len() + e.args.len())
            .sum::<usize>() as u64;

        let _ = writeln!(
            self.out,
            "// Meta is the cold table, read only by the rules that are decided once the\n\
             // last token has been read: required, choices, the env-then-default fallback,\n\
             // the var bounds, and the four that compare one entry against another. A parse\n\
             // never touches it.\n\
             //\n\
             // Indexed by key, so entry Key sits at Meta[Key-1]. A command's slot is empty:\n\
             // commands take keys too, and have no cold half.\n\
             var Meta = argv.Metadata{{"
        );
        for key in 1..=total {
            match by_key.get(&key) {
                Some(entry) => {
                    let _ = writeln!(self.out, "\t{entry},");
                }
                None => {
                    let _ = writeln!(self.out, "\t{{}},");
                }
            }
        }
        let _ = writeln!(self.out, "}}\n");
    }

    /// The cold half of a flag.
    fn flag_meta(
        &self,
        flag: &SpecFlag,
        named: &Named,
        owner: &Emitted,
        commands: &[Emitted],
    ) -> String {
        let mut fields = vec![
            format!("Key: {}", named.key),
            format!("Name: {}", go_string(&flag.name)),
            "Flag: true".to_string(),
        ];
        if !owner.cmd.args_override_self
            && !flag.var
            && !flag.count
            && !flag.arg.as_ref().is_some_and(|arg| arg.var)
        {
            fields.push("RejectDuplicate: true".to_string());
        }
        if flag.required {
            fields.push("Required: true".to_string());
        }
        if flag.arg.is_none() {
            fields.push("RequiresIfBoolean: true".to_string());
        }
        // How a user types it, worked out where the forms are visible: the rules
        // that judge an entry never see a flag, and guessing from the name gets a
        // one-letter long form and a short the wrong way round.
        if let Some(long) = flag.long.first() {
            fields.push(format!("Spelling: {}", go_string(&format!("--{long}"))));
        } else if let Some(short) = flag.short.first() {
            fields.push(format!("Spelling: {}", go_string(&format!("-{short}"))));
        }
        // What the value is called, which is what says whether a path belongs
        // there — `--into <DIR>` completes directories because of the name.
        if let Some(value) = flag.arg.as_ref() {
            fields.push(format!("ValueName: {}", go_string(&value.name)));
        }
        let named_value = flag
            .arg
            .as_ref()
            .map(|a| a.name.as_str())
            .unwrap_or(flag.name.as_str());
        if let Some(kind) = complete_type(self.spec, named_value) {
            fields.push(format!("CompleteType: {}", go_string(kind)));
        }
        // Written on the value a flag takes, never on the flag.
        if let Some(choices) = flag.arg.as_ref().and_then(|a| a.choices.as_ref()) {
            fields.push(format!(
                "Choices: {}",
                string_slice(&visible_choices(choices))
            ));
            fields.push(format!(
                "AcceptedChoices: {}",
                string_slice(&accepted_choices(choices))
            ));
            if choices.ignore_case {
                fields.push("IgnoreCase: true".to_string());
            }
        }
        // A default can be written in either place, and usage-lib falls back to
        // the one on the value. `env` deliberately does not follow the same
        // nesting, because usage-lib does not read it there either.
        let default = if !flag.default.is_empty() {
            &flag.default
        } else {
            flag.arg
                .as_ref()
                .map(|a| &a.default)
                .unwrap_or(&flag.default)
        };
        if !default.is_empty() {
            fields.push(format!("Default: {}", string_slice(default)));
        }
        if let Some(env) = &flag.env {
            fields.push(format!("Env: {}", go_string(env)));
        }
        let minimum = flag
            .arg
            .as_ref()
            .filter(|arg| arg.var)
            .and_then(|arg| arg.var_min)
            .or(flag.var_min);
        if let Some(min) = minimum {
            fields.push(format!("VarMin: {}", clamp_var_max(min)));
        }
        // Occurrences. The per-occurrence value bound is a limit binding applies
        // and lives on the parse table.
        if let Some(max) = flag.var_max {
            fields.push(format!("VarMax: {}", clamp_var_max(max)));
        }

        for (label, names) in [
            ("Conflicts", &flag.conflicts),
            ("Overrides", &flag.overrides),
            ("RequiredUnless", &flag.required_unless),
            ("RequiredUnlessAll", &flag.required_unless_all),
            ("RequiredIf", &flag.required_if),
            ("Requires", &flag.requires),
        ] {
            let keys = resolve_relationship(names, owner, commands);
            if !keys.is_empty() {
                fields.push(format!("{label}: {}", key_slice(&keys)));
            }
        }
        for (label, conditions) in [
            ("RequiredIfEq", &flag.required_if_eq),
            ("RequiredIfEqAll", &flag.required_if_eq_all),
        ] {
            let values = conditions
                .iter()
                .filter_map(|condition| {
                    resolve_relationship(std::slice::from_ref(&condition.selector), owner, commands)
                        .into_iter()
                        .next()
                        .map(|key| {
                            format!("{{Key: {key}, Value: {}}}", go_string(&condition.value))
                        })
                })
                .collect::<Vec<_>>();
            if !values.is_empty() {
                fields.push(format!(
                    "{label}: []argv.ValueCondition{{{}}}",
                    values.join(", ")
                ));
            }
        }
        let requires_if = flag
            .requires_if
            .iter()
            .filter_map(|condition| {
                resolve_relationship(std::slice::from_ref(&condition.requires), owner, commands)
                    .into_iter()
                    .next()
                    .map(|key| format!("{{Value: {}, Key: {key}}}", go_string(&condition.value)))
            })
            .collect::<Vec<_>>();
        if !requires_if.is_empty() {
            fields.push(format!(
                "RequiresIf: []argv.ValueRequirement{{{}}}",
                requires_if.join(", ")
            ));
        }
        let default_if = flag
            .default_if
            .iter()
            .filter_map(|condition| {
                resolve_relationship(std::slice::from_ref(&condition.selector), owner, commands)
                    .into_iter()
                    .next()
                    .map(|key| match &condition.when {
                        None => format!("{{Key: {key}, Value: {}}}", go_string(&condition.value)),
                        Some(when) => format!(
                            "{{Key: {key}, When: {}, Value: {}}}",
                            go_string(when),
                            go_string(&condition.value)
                        ),
                    })
            })
            .collect::<Vec<_>>();
        if !default_if.is_empty() {
            fields.push(format!(
                "DefaultIf: []argv.DefaultIf{{{}}}",
                default_if.join(", ")
            ));
        }

        format!("{{{}}}", fields.join(", "))
    }
}

/// The cold half of a positional argument.
/// The type a spec's `complete` block names for an entry, if it names one.
///
/// By lowercased name, which is how usage-lib files them: `complete "FILE"` and
/// an argument written `<file>` are the same position as far as the reference is
/// concerned.
fn complete_type<'a>(spec: &'a Spec, name: &str) -> Option<&'a str> {
    spec.complete
        .get(&name.to_lowercase())
        .and_then(|c| c.type_.as_deref())
}

fn arg_meta(
    spec: &Spec,
    arg: &SpecArg,
    named: &Named,
    owner: &Emitted,
    commands: &[Emitted],
) -> String {
    let mut fields = vec![
        format!("Key: {}", named.key),
        format!("Name: {}", go_string(&arg.name)),
    ];
    if arg.required {
        fields.push("Required: true".to_string());
    }
    // What the position takes, where the spec said so. Read by completion rather
    // than by any post-binding rule: an author who wrote `complete "input"
    // type="file"` named what belongs there, and the alternative is inferring it
    // from a name they did not choose.
    if let Some(kind) = complete_type(spec, &arg.name) {
        fields.push(format!("CompleteType: {}", go_string(kind)));
    }
    if let Some(choices) = &arg.choices {
        fields.push(format!(
            "Choices: {}",
            string_slice(&visible_choices(choices))
        ));
        fields.push(format!(
            "AcceptedChoices: {}",
            string_slice(&accepted_choices(choices))
        ));
        if choices.ignore_case {
            fields.push("IgnoreCase: true".to_string());
        }
    }
    if !arg.default.is_empty() {
        fields.push(format!("Default: {}", string_slice(&arg.default)));
    }
    if let Some(env) = &arg.env {
        fields.push(format!("Env: {}", go_string(env)));
    }
    if let Some(min) = arg.var_min {
        fields.push(format!("VarMin: {}", clamp_var_max(min)));
    }
    let conflicts = resolve_relationship(&arg.conflicts, owner, commands);
    if !conflicts.is_empty() {
        fields.push(format!("Conflicts: {}", key_slice(&conflicts)));
    }
    for (label, names) in [
        ("Requires", &arg.requires),
        ("RequiredIf", &arg.required_if),
        ("RequiredUnless", &arg.required_unless),
        ("RequiredUnlessAll", &arg.required_unless_all),
    ] {
        let keys = resolve_relationship(names, owner, commands);
        if !keys.is_empty() {
            fields.push(format!("{label}: {}", key_slice(&keys)));
        }
    }
    for (label, conditions) in [
        ("RequiredIfEq", &arg.required_if_eq),
        ("RequiredIfEqAll", &arg.required_if_eq_all),
    ] {
        let values = conditions
            .iter()
            .filter_map(|condition| {
                resolve_relationship(std::slice::from_ref(&condition.selector), owner, commands)
                    .into_iter()
                    .next()
                    .map(|key| format!("{{Key: {key}, Value: {}}}", go_string(&condition.value)))
            })
            .collect::<Vec<_>>();
        if !values.is_empty() {
            fields.push(format!(
                "{label}: []argv.ValueCondition{{{}}}",
                values.join(", ")
            ));
        }
    }
    // No VarMax: for an argument the bound is a limit binding applies, which is
    // what makes `[a]… [b]` fillable at all, so judging it again would fail an
    // invocation that never broke it.
    format!("{{{}}}", fields.join(", "))
}

/// Turn the names in a relationship into the keys they refer to.
///
/// Resolved here, where the whole command is visible, so that nothing downstream
/// searches by name on a path it would repeat per parse. The names arrive as
/// written — `--stdin`, dashes and all — so they are matched against a flag's long
/// forms, its shorts, and the name the spec gives it.
///
/// A name nothing answers to is dropped. That is a spec bug worth reporting, but
/// this function has no way to; the check belongs beside the duplicate-form and
/// duplicate-key checks that already run where the whole tree is visible.
fn resolve_relationship(names: &[String], owner: &Emitted, commands: &[Emitted]) -> Vec<String> {
    let mut out = Vec::new();
    for name in names {
        // The declaring command's own flags first, then any ancestor's globals —
        // the scope a token has, in the order a token gets it, so a subcommand
        // redeclaring an inherited name shadows it here as it does at parse time.
        let mut found = match_flag(owner, name, false);
        if found.is_none() && !name.starts_with('-') {
            found = owner
                .args
                .iter()
                .find(|(arg, _)| arg.name == *name)
                .map(|(_, named)| named.key.clone());
        }
        if found.is_none() {
            let path = &owner.cmd.full_cmd;
            for depth in (0..path.len()).rev() {
                let ancestor = commands
                    .iter()
                    .find(|e| e.cmd.full_cmd.len() == depth && e.cmd.full_cmd[..] == path[..depth]);
                if let Some(key) = ancestor.and_then(|a| match_flag(a, name, true)) {
                    found = Some(key);
                    break;
                }
            }
        }
        if let Some(key) = found {
            out.push(key);
        }
    }
    out
}

/// Find a flag by any spelling a declaration may use for it.
///
/// The negation counts, and resolves to the same entry: usage-lib treats
/// `conflicts = "--no-color"` as naming the `color` flag and reports the conflict
/// whichever of the two spellings was typed. The relationship is between entries
/// rather than between tokens, which is what the key model already assumes.
fn match_flag(cmd: &Emitted, name: &str, globals_only: bool) -> Option<String> {
    // Two passes, in the order the parser itself looks: every ordinary form
    // first, then negations.
    //
    // That order is not a nicety. The parser tries every long form before it
    // tries any negation, so with `--a` declaring `negate = "--zap"` and a
    // separate `--zap`, typing `--zap` binds *zap*. A per-candidate search hands
    // the relationship to `a`, and the table then enforces a rule against a flag
    // the command line never binds. The table has to agree with the binder it
    // feeds.
    let eligible = |flag: &SpecFlag| !globals_only || flag.global;

    // The form is part of the name: `--q` does not reach the short `-q`, and
    // `-color` does not reach the long `--color`. usage-lib resolves neither.
    let (long, short, bare) = if let Some(rest) = name.strip_prefix("--") {
        (Some(rest), None, None)
    } else if let Some(rest) = name.strip_prefix('-') {
        let mut chars = rest.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => (None, Some(c), None),
            _ => (None, None, None),
        }
    } else {
        (None, None, Some(name))
    };

    let ordinary = cmd.flags.iter().find(|(flag, _)| {
        if !eligible(flag) {
            return false;
        }
        if let Some(bare) = bare {
            return flag.name == bare;
        }
        if let Some(long) = long {
            return flag.long.iter().any(|l| l == long);
        }
        short.is_some_and(|c| flag.short.contains(&c))
    });
    if let Some((_, named)) = ordinary {
        return Some(named.key.clone());
    }

    // Negations, compared exactly as both sides were written — dashes included.
    // `negate = "-no-tint"` is named by `-no-tint` and not by `--no-tint`, and
    // usage-lib resolves it that way round too.
    cmd.flags
        .iter()
        .find(|(flag, _)| eligible(flag) && flag.negate.as_deref() == Some(name))
        .map(|(_, named)| named.key.clone())
}
fn string_slice(values: &[String]) -> String {
    let list = values
        .iter()
        .map(|v| go_string(v))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[]string{{{list}}}")
}

fn accepted_choices(choices: &SpecChoices) -> Vec<String> {
    choices
        .choices
        .iter()
        .chain(
            choices
                .details
                .iter()
                .flat_map(|choice| choice.aliases.iter().map(|alias| &alias.value)),
        )
        .cloned()
        .collect()
}

fn visible_choices(choices: &SpecChoices) -> Vec<String> {
    choices
        .choices
        .iter()
        .filter(|value| {
            !choices
                .details
                .iter()
                .any(|choice| choice.value == value.as_str() && choice.hide)
        })
        .chain(choices.details.iter().flat_map(|choice| {
            choice
                .aliases
                .iter()
                .filter(|alias| !alias.hide)
                .map(|alias| &alias.value)
        }))
        .cloned()
        .collect()
}

fn key_slice(keys: &[String]) -> String {
    format!("[]uint64{{{}}}", keys.join(", "))
}

impl Emitter<'_> {
    /// Emit the help table: what a page prints.
    ///
    /// A third table rather than more fields on `Meta`, because Go's linker drops
    /// an unreferenced package-level symbol whole — folding help text into the
    /// post-binding table would make every CLI that applies a rule carry mise's
    /// several hundred kilobytes of help strings too.
    fn help_table(&mut self, commands: &[Emitted]) {
        let mut by_key: BTreeMap<u64, String> = BTreeMap::new();
        for e in commands {
            by_key.insert(e.named.number, command_help(e));
            for (flag, named) in &e.flags {
                by_key.insert(named.number, flag_help(flag, named));
            }
            for (arg, named) in &e.args {
                by_key.insert(named.number, arg_help(arg, named));
            }
        }

        let total = commands
            .iter()
            .map(|e| 1 + e.flags.len() + e.args.len())
            .sum::<usize>() as u64;

        let _ = writeln!(
            self.out,
            "// HelpText is the third table, read only when a page is rendered. Neither the\n\
             // parser nor the post-binding rules touch it, and a CLI that never prints help\n\
             // does not carry it: Go's linker drops an unreferenced table whole.\n\
             //\n\
             // Indexed by key, like the others.\n\
             var HelpText = argv.HelpTable{{"
        );
        for key in 1..=total {
            match by_key.get(&key) {
                Some(entry) => {
                    let _ = writeln!(self.out, "\t{entry},");
                }
                None => {
                    let _ = writeln!(self.out, "\t{{}},");
                }
            }
        }
        let _ = writeln!(self.out, "}}\n");

        let mut fields = vec![
            format!("Name: {}", go_string(&self.spec.name)),
            format!("Bin: {}", go_string(&self.spec.bin)),
        ];
        if let Some(version) = self
            .spec
            .version
            .as_ref()
            .or(self.spec.long_version.as_ref())
        {
            fields.push(format!("Version: {}", go_string(version)));
        }
        if let Some(version) = &self.spec.long_version {
            fields.push(format!("LongVersion: {}", go_string(version)));
        }
        // `about` alone, with no fall back to the long one: usage-lib's short page
        // prints nothing where a spec wrote only `about_long`, and the long page
        // is what reads LongAbout.
        if let Some(about) = &self.spec.about {
            fields.push(format!("About: {}", go_string(about)));
        }
        if let Some(long) = &self.spec.about_long {
            fields.push(format!("LongAbout: {}", go_string(long)));
        }
        if let Some(before) = &self.spec.before_help {
            fields.push(format!("BeforeHelp: {}", go_string(before)));
        }
        if let Some(after) = &self.spec.after_help {
            fields.push(format!("AfterHelp: {}", go_string(after)));
        }
        if let Some(before) = &self.spec.before_help_long {
            fields.push(format!("BeforeLongHelp: {}", go_string(before)));
        }
        if let Some(after) = &self.spec.after_help_long {
            fields.push(format!("AfterLongHelp: {}", go_string(after)));
        }
        let _ = writeln!(
            self.out,
            "// HelpMeta is what a page needs from the spec's root rather than from any one\n\
             // command: the header, and the text that brackets every page.\n\
             var HelpMeta = argv.HelpSpec{{{}}}\n",
            fields.join(", ")
        );
    }
}

/// The help entry for a command: its about text.
fn command_help(e: &Emitted) -> String {
    let mut fields = vec![format!("Key: {}", e.named.key)];
    if e.cmd.hide {
        fields.push("Hide: true".to_string());
    }
    if let Some(heading) = &e.cmd.help_heading {
        fields.push(format!("Heading: {}", go_string(heading)));
    }
    if let Some(order) = e.cmd.display_order {
        fields.push(format!("DisplayOrder: {order}"));
        fields.push("DisplayOrderSet: true".to_string());
    }
    if let Some(help) = e.cmd.help.as_deref().or(e.cmd.help_long.as_deref()) {
        fields.push(format!("Short: {}", go_string(help)));
    }
    if let Some(long) = &e.cmd.help_long {
        fields.push(format!("Long: {}", go_string(long)));
    }
    if let Some(heading) = &e.cmd.subcommand_help_heading {
        fields.push(format!("SubcommandHelpHeading: {}", go_string(heading)));
    }
    if let Some(name) = &e.cmd.subcommand_value_name {
        fields.push(format!("SubcommandValueName: {}", go_string(name)));
    }
    if e.cmd.next_line_help {
        fields.push("NextLineHelp: true".to_string());
    }
    if e.cmd.flatten_help {
        fields.push("FlattenHelp: true".to_string());
    }
    if e.cmd.subcommand_required {
        fields.push("SubcommandRequired: true".to_string());
    }
    // Visible only: the parse table merges hidden aliases in beside these,
    // because binding does not care which is which. A page does.
    let visible: Vec<String> = e
        .cmd
        .aliases
        .iter()
        .filter(|a| !e.cmd.hidden_aliases.contains(a))
        .cloned()
        .collect();
    if !visible.is_empty() {
        fields.push(format!("VisibleAliases: {}", string_slice(&visible)));
    }
    if let Some(before) = &e.cmd.before_help {
        fields.push(format!("BeforeHelp: {}", go_string(before)));
    }
    if let Some(after) = &e.cmd.after_help {
        fields.push(format!("AfterHelp: {}", go_string(after)));
    }
    // The long page's own brackets, which most of mise's commands use: their
    // examples are written as `after_long_help`, and a generated CLI that dropped
    // them printed a page with the examples missing.
    if let Some(before) = &e.cmd.before_help_long {
        fields.push(format!("BeforeLongHelp: {}", go_string(before)));
    }
    if let Some(after) = &e.cmd.after_help_long {
        fields.push(format!("AfterLongHelp: {}", go_string(after)));
    }
    if !e.cmd.examples.is_empty() {
        let items = e
            .cmd
            .examples
            .iter()
            .map(|x| {
                let mut parts = Vec::new();
                if let Some(header) = &x.header {
                    parts.push(format!("Header: {}", go_string(header)));
                }
                parts.push(format!("Code: {}", go_string(&x.code)));
                // The line the long page prints above the command. It introduces
                // the invocation rather than commenting on it, and a generated CLI
                // that dropped it printed the command with nothing to say why.
                if let Some(help) = &x.help {
                    parts.push(format!("Help: {}", go_string(help)));
                }
                format!("{{{}}}", parts.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(format!("Examples: []argv.Example{{{items}}}"));
    }
    format!("{{{}}}", fields.join(", "))
}

fn flag_help(flag: &SpecFlag, named: &Named) -> String {
    let mut fields = vec![format!("Key: {}", named.key)];
    if flag.hide {
        fields.push("Hide: true".to_string());
    }
    if let Some(order) = flag.display_order {
        fields.push(format!("DisplayOrder: {order}"));
        fields.push("DisplayOrderSet: true".to_string());
    }
    for (name, hidden) in [
        ("HideDefaultValue", flag.hide_default_value),
        ("HideEnv", flag.hide_env),
        ("HideEnvValues", flag.hide_env_values),
        ("HidePossibleValues", flag.hide_possible_values),
        ("HideShortHelp", flag.hide_short_help),
        ("HideLongHelp", flag.hide_long_help),
    ] {
        if hidden {
            fields.push(format!("{name}: true"));
        }
    }
    // Required *and* undefaulted, which is what decides the brackets: a required
    // flag with a default is one the user never has to type.
    if flag.required && flag.default.is_empty() {
        fields.push("Demanded: true".to_string());
    }
    if flag.var {
        fields.push("Repeatable: true".to_string());
    }
    if let Some(arg) = &flag.arg {
        if arg.name != flag.name {
            fields.push(format!("ValueName: {}", go_string(&arg.name)));
        }
        // The value's own requiredness, which is independent of the flag's:
        // `<--v <n>>` is a required flag whose value must be given, and
        // `<--jobs [n]>` a required flag whose value has a default.
        if arg.required && arg.default.is_empty() {
            fields.push("ValueDemanded: true".to_string());
        }
        if !arg.value_names.is_empty() {
            fields.push(format!("ValueNames: {}", string_slice(&arg.value_names)));
        }
        if arg.var && arg.var_min == arg.var_max && arg.var_min.is_some_and(|n| n > 1) {
            fields.push(format!("ValueArity: {}", arg.var_min.unwrap()));
        }
    }
    // The whole `help`, not its first line: usage-lib's short page prints the
    // text as declared, and mise has flags whose help is two lines.
    if let Some(help) = flag.help.as_deref().or(flag.help_first_line.as_deref()) {
        fields.push(format!("Short: {}", go_string(help)));
    }
    if let Some(long) = flag.help_long.as_deref().or(flag.help.as_deref()) {
        fields.push(format!("Long: {}", go_string(long)));
    }
    if let Some(heading) = &flag.help_heading {
        fields.push(format!("Heading: {}", go_string(heading)));
    }
    // Annotations. A flag's choices are declared on the value it takes.
    if let Some(choices) = flag.arg.as_ref().and_then(|a| a.choices.as_ref()) {
        fields.push(format!(
            "Choices: {}",
            string_slice(&visible_choices(choices))
        ));
    }
    if let Some(env) = &flag.env {
        fields.push(format!("Env: {}", go_string(env)));
    }
    let default = if !flag.default.is_empty() {
        &flag.default
    } else {
        flag.arg
            .as_ref()
            .map(|a| &a.default)
            .unwrap_or(&flag.default)
    };
    if !default.is_empty() {
        fields.push(format!("Default: {}", string_slice(default)));
    }
    format!("{{{}}}", fields.join(", "))
}

fn arg_help(arg: &SpecArg, named: &Named) -> String {
    let mut fields = vec![format!("Key: {}", named.key)];
    if let Some(order) = arg.display_order {
        fields.push(format!("DisplayOrder: {order}"));
        fields.push("DisplayOrderSet: true".to_string());
    }
    if arg.hide {
        fields.push("Hide: true".to_string());
    }
    for (name, hidden) in [
        ("HideDefaultValue", arg.hide_default_value),
        ("HideEnv", arg.hide_env),
        ("HideEnvValues", arg.hide_env_values),
        ("HidePossibleValues", arg.hide_possible_values),
        ("HideShortHelp", arg.hide_short_help),
        ("HideLongHelp", arg.hide_long_help),
    ] {
        if hidden {
            fields.push(format!("{name}: true"));
        }
    }
    if arg.required && arg.default.is_empty() {
        fields.push("Demanded: true".to_string());
    }
    if !arg.value_names.is_empty() {
        fields.push(format!("ValueNames: {}", string_slice(&arg.value_names)));
    }
    if arg.var && arg.var_min == arg.var_max && arg.var_min.is_some_and(|n| n > 1) {
        fields.push(format!("ValueArity: {}", arg.var_min.unwrap()));
    }
    if let Some(help) = arg.help.as_deref().or(arg.help_first_line.as_deref()) {
        fields.push(format!("Short: {}", go_string(help)));
    }
    if let Some(long) = arg.help_long.as_deref().or(arg.help.as_deref()) {
        fields.push(format!("Long: {}", go_string(long)));
    }
    if let Some(heading) = &arg.help_heading {
        fields.push(format!("Heading: {}", go_string(heading)));
    }
    if let Some(choices) = &arg.choices {
        fields.push(format!(
            "Choices: {}",
            string_slice(&visible_choices(choices))
        ));
    }
    if let Some(env) = &arg.env {
        fields.push(format!("Env: {}", go_string(env)));
    }
    if !arg.default.is_empty() {
        fields.push(format!("Default: {}", string_slice(&arg.default)));
    }
    format!("{{{}}}", fields.join(", "))
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
    if !flag.hidden_aliases.is_empty() {
        fields.push(format!(
            "HiddenLongs: {}",
            string_slice(&flag.hidden_aliases)
        ));
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
    if !flag.hidden_short_aliases.is_empty() {
        let shorts = flag
            .hidden_short_aliases
            .iter()
            .map(|c| go_byte(*c))
            .collect::<Vec<_>>()
            .join(", ");
        fields.push(format!("HiddenShorts: []byte{{{shorts}}}"));
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
    if flag.value_optional {
        fields.push("ValueOptional: true".to_string());
    }
    if flag.bool_value {
        fields.push("BoolValue: true".to_string());
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
    if flag.allow_hyphen_values() {
        fields.push("AllowHyphenValues: true".to_string());
    }
    if let Some(arg) = &flag.arg {
        if arg.allow_negative_numbers {
            fields.push("AllowNegativeNumbers: true".to_string());
        }
        if let Some(terminator) = &arg.value_terminator {
            fields.push(format!("ValueTerminator: {}", go_string(terminator)));
        }
        if let Some(delimiter) = arg.delimiter {
            fields.push(format!("Delimiter: {}", go_byte(delimiter)));
        }
    }
    if flag.require_equals {
        fields.push("RequireEquals: true".to_string());
    }
    if let Some(missing) = &flag.default_missing {
        fields.push(format!("DefaultMissing: {}", go_string(missing)));
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
    if arg.required {
        fields.push("Required: true".to_string());
    }
    if arg.var {
        fields.push("Var: true".to_string());
        if let Some(max) = arg.var_max {
            fields.push(format!("VarMax: {}", clamp_var_max(max)));
        }
    }
    if arg.allow_negative_numbers {
        fields.push("AllowNegativeNumbers: true".to_string());
    }
    if let Some(terminator) = &arg.value_terminator {
        fields.push(format!("ValueTerminator: {}", go_string(terminator)));
    }
    if let Some(delimiter) = arg.delimiter {
        fields.push(format!("Delimiter: {}", go_byte(delimiter)));
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

/// Two more names a table package cannot have, for two different reasons.
///
/// `_` is refused where it is written: `invalid package name _`. `init` declares
/// perfectly well and cannot be *imported* — an import binds the package name as
/// an identifier in file scope, and `init` may only be a func, so an importer gets
/// `cannot import package as init - init must be a func`. A table package exists
/// to be imported, so it is out either way.
///
/// Both checked against the compiler rather than taken from a citation. The issue
/// usually cited for `init` is about the import, and `package init` on its own
/// does build — so a validator written from the citation would have rejected it
/// for a reason that is not true.
const UNUSABLE_PACKAGE_NAMES: &[&str] = &["_", "init"];

/// Whether a string can be written after `package` and then imported.
///
/// Deliberately ASCII-only. Go itself allows a Unicode letter, but a package name
/// that needs one is a worse problem for an adopter than the restriction is.
pub fn is_valid_package(name: &str) -> bool {
    !name.is_empty()
        && !GO_KEYWORDS.contains(&name)
        && !UNUSABLE_PACKAGE_NAMES.contains(&name)
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A Go field name from a spec name: exported, and an identifier.
fn field_name(name: &str) -> String {
    let ident = format!("{}", AsPascalCase(name));
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        format!("X{ident}")
    } else {
        ident
    }
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
    if is_valid_package(&lowered) {
        lowered
    } else {
        // One rule rather than a second copy of the conditions, so the sanitizer
        // cannot come to disagree with the validator about what is acceptable.
        // `cli` in front keeps it recognizable: `cligo`, `cli7zip`, `cliinit`.
        format!("cli{lowered}")
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

    /// The emitted `Meta` line for an entry, so a test can assert about the part
    /// it cares about rather than the whole rendered row — which grows a field
    /// every time the cold table learns something.
    ///
    /// Used for what a row *does* say as well as for what it does not. Two
    /// substring checks over the whole file — one for the name, one for the
    /// relationship — pass when the relationship is attached to a different flag
    /// entirely, which is the regression these tests exist to catch.
    fn entry_of(out: &str, name: &str) -> String {
        out.lines()
            .find(|l| l.contains(&format!("Name: \"{name}\", Flag: true")))
            .unwrap_or_default()
            .to_string()
    }

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
long_version "1.2.3\ncommit abc123"
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

    #[test]
    fn rich_choices_keep_acceptance_and_visibility_separate() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--color <when>" {
    choices ignore_case=#true {
        choice "always" {
            alias "yes"
            alias "on" hide=#true
        }
        choice "never" hide=#true
    }
}
"#);
        let entry = entry_of(&out, "color");
        assert!(
            entry.contains(r#"Choices: []string{"always", "yes"}"#),
            "{entry}"
        );
        assert!(
            entry.contains(r#"AcceptedChoices: []string{"always", "never", "yes", "on"}"#),
            "{entry}"
        );
        assert!(entry.contains("IgnoreCase: true"), "{entry}");
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
        // `package _` is refused outright; `package init` declares fine and cannot
        // be imported, which for a table package is the same thing.
        assert_eq!(package_ident("_"), "cli_");
        assert_eq!(package_ident("init"), "cliinit");
        // Two underscores is fine, and only the exact name is reserved.
        assert_eq!(package_ident("__"), "__");
        assert_eq!(package_ident("initialize"), "initialize");

        // Whatever it produces must be something the validator accepts, for every
        // one of these — the sanitizer disagreeing with the check is how a file
        // that does not compile gets emitted.
        for bin in [
            "my-cli", "7zip", "", "go", "type", "_", "init", "__", "MiSe",
        ] {
            let out = package_ident(bin);
            assert!(is_valid_package(&out), "{bin:?} sanitized to {out:?}");
        }
    }

    /// Counting alone let a third command collide with a generated suffix.
    #[test]
    fn a_name_matching_a_generated_suffix_still_gets_its_own() {
        let out = go(r#"
name "ex"
bin "ex"
cmd "macos-defaults" {}
cmd "macos" {
    cmd "defaults" {}
}
cmd "macos-defaults2" {}
"#);
        // The invariant, not a guess at the spelling. The third command lands on
        // `CmdMacosDefaults22` rather than `...3`, which is unlovely and correct;
        // asserting the exact name would pin the suffix scheme instead of the
        // property that matters, which is that nothing is declared twice.
        assert_declares_each_constant_once(&out);
    }

    /// Every constant in the emitted `const` block, in declaration order.
    fn constant_names(out: &str) -> Vec<&str> {
        out.lines()
            .skip_while(|l| !l.starts_with("const ("))
            .skip(1)
            .take_while(|l| !l.starts_with(')'))
            .filter_map(|l| l.split_whitespace().next())
            .collect()
    }

    /// Two entries sharing a constant is a file that does not compile.
    fn assert_declares_each_constant_once(out: &str) {
        let names = constant_names(out);
        assert!(!names.is_empty(), "no constants at all:\n{out}");
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            assert!(seen.insert(*name), "{name} is declared twice:\n{out}");
        }
    }

    #[test]
    fn a_package_that_would_not_compile_is_refused_rather_than_emitted() {
        assert!(is_valid_package("mycli"));
        assert!(is_valid_package("mise_tables"));
        assert!(!is_valid_package("my-pkg"));
        assert!(!is_valid_package("7zip"));
        assert!(!is_valid_package(""));
        assert!(!is_valid_package("range"));
        assert!(!is_valid_package("_"));
        assert!(!is_valid_package("init"));
        assert!(is_valid_package("__"));

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
    /// The long page's text is a table entry too.
    ///
    /// mise writes its examples as `after_long_help`, on 115 of its commands, so a
    /// generator that dropped them emitted a `--help` with every example missing —
    /// while the page tests, which build their tables by lowering rather than by
    /// generating, saw nothing wrong. The two producers are compared against each
    /// other now; this is the same rule from the emitter's side.
    #[test]
    fn the_long_pages_text_reaches_the_tables() {
        let out = go(r#"
name "ex"
bin "ex"
about "Short."
about_long "Long."
before_long_help "ROOT-BEFORE"
after_long_help "ROOT-AFTER"
cmd "run" help="Run it" {
    before_long_help "RUN-BEFORE"
    after_long_help "RUN-AFTER"
}
"#);
        let meta = out
            .lines()
            .find(|l| l.contains("var HelpMeta"))
            .expect("a root header is emitted");
        assert!(
            meta.contains(r#"About: "Short.""#) && meta.contains(r#"LongAbout: "Long.""#),
            "the two abouts are separate fields: {meta}"
        );
        assert!(
            meta.contains(r#"BeforeLongHelp: "ROOT-BEFORE""#)
                && meta.contains(r#"AfterLongHelp: "ROOT-AFTER""#),
            "the root's long brackets are emitted: {meta}"
        );

        let run = out
            .lines()
            .find(|l| l.contains("Short: \"Run it\""))
            .expect("the command has a help entry");
        assert!(
            run.contains(r#"BeforeLongHelp: "RUN-BEFORE""#)
                && run.contains(r#"AfterLongHelp: "RUN-AFTER""#),
            "a command's long brackets are emitted: {run}"
        );
    }

    /// An example's help line reaches the tables.
    ///
    /// The long page prints it above the command, where it introduces the
    /// invocation; a generated CLI that dropped it printed the command with
    /// nothing to say why. mise cannot show this — it writes its examples as
    /// `after_long_help` text rather than as `example` nodes — so the producer
    /// comparison over mise's spec cannot see it either.
    #[test]
    fn an_examples_help_line_reaches_the_tables() {
        let out = go(r#"
name "ex"
bin "ex"
cmd "run" help="Run it" {
    example "ex run --fast" header="Speed" help="When you are in a hurry"
    example "ex run"
}
"#);
        let run = out
            .lines()
            .find(|l| l.contains("Examples: []argv.Example"))
            .expect("the command's examples are emitted");
        assert!(
            run.contains(
                r#"{Header: "Speed", Code: "ex run --fast", Help: "When you are in a hurry"}"#
            ),
            "all three fields are emitted: {run}"
        );
        // And a bare example says only what it has, rather than an empty header.
        assert!(
            run.contains(r#"{Code: "ex run"}"#),
            "an example with no header emits no header: {run}"
        );
    }

    /// `about_long` alone leaves the short page's About unset, because usage-lib
    /// prints nothing there: the long text belongs to the long page.
    #[test]
    fn a_long_about_alone_does_not_become_the_short_one() {
        let out = go(r#"
name "ex"
bin "ex"
about_long "Long only."
"#);
        let meta = out
            .lines()
            .find(|l| l.contains("var HelpMeta"))
            .expect("a root header is emitted");
        assert!(
            !meta.contains(", About: ") && meta.contains(r#"LongAbout: "Long only.""#),
            "only the long one is set: {meta}"
        );
    }

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

    /// A command's own name outranks another command's alias, so which command the
    /// emitted `DefaultSubcommand` points at does not depend on declaration order.
    #[test]
    fn a_default_subcommand_prefers_a_name_to_another_commands_alias() {
        let ordered = |first: &str, second: &str| {
            go(&format!(
                r#"
name "ex"
bin "ex"
default_subcommand "run"
{first}
{second}
"#
            ))
        };
        let alpha = "cmd \"alpha\" {\n    alias \"run\"\n}";
        let run = "cmd \"run\" {\n    arg \"[args]...\" var=#true\n}";
        for out in [ordered(alpha, run), ordered(run, alpha)] {
            assert!(
                out.contains("DefaultSubcommand: cmdRun,"),
                "should point at the command named `run`, got:\n{out}"
            );
        }
    }

    #[test]
    fn an_external_subcommand_is_emitted_on_the_command_that_declares_it() {
        let out = go(r#"
name "ex"
bin "ex"
external_subcommand #true
cmd "install"
cmd "exec" external_subcommand=#true
"#);
        let block = |var: &str| {
            let start = out
                .find(&format!("var {var} ="))
                .unwrap_or_else(|| panic!("{var} should be emitted, got:\n{out}"));
            let rest = &out[start..];
            let end = rest[1..]
                .find("\nvar ")
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            &rest[..end]
        };
        assert!(
            block("Root").contains("ExternalSubcommand: true"),
            "the root should forward unmatched words:\n{}",
            block("Root")
        );
        assert!(
            block("cmdExec").contains("ExternalSubcommand: true"),
            "a nested command can forward too:\n{}",
            block("cmdExec")
        );
        assert!(
            !block("cmdInstall").contains("ExternalSubcommand"),
            "a command that does not declare it should not carry it:\n{}",
            block("cmdInstall")
        );
    }

    #[test]
    fn arg_required_else_help_reaches_the_table_and_typed_front_door() {
        let out = go(r#"
name "ex"
bin "ex"
cmd "run" arg_required_else_help=#true {
    flag "--all"
}
"#);
        assert!(
            out.contains("ArgRequiredElseHelp: true"),
            "the command table should carry the policy:\n{out}"
        );
        assert!(
            out.contains("p.Command().ArgRequiredElseHelp && p.CommandStart() == len(args)"),
            "the typed parser should enforce it before fallbacks:\n{out}"
        );
    }

    #[test]
    fn subcommand_negates_requirements_reaches_generated_go() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nsubcommand_negates_reqs #true\nflag \"--config\" required=#true\ncmd \"run\"\n",
        );
        assert!(out.contains("SubcommandNegatesReqs: true"), "{out}");
        assert!(
            out.contains("checkRequirements := i == len(chain)-1 || !cmd.SubcommandNegatesReqs"),
            "{out}"
        );
        assert!(
            out.contains("CheckRelationshipsWithValuesAndRequirements"),
            "{out}"
        );
    }

    #[test]
    fn argument_subcommand_conflicts_reach_generated_go() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nargs_conflicts_with_subcommands #true\nflag \"--verbose\"\ncmd \"run\"\n",
        );
        assert!(out.contains("ArgsConflictWithSubcommands: true"), "{out}");
    }

    #[test]
    fn allow_missing_positional_reaches_generated_go() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nallow_missing_positional #true\narg \"[optional]\"\narg \"<required>\"\n",
        );
        assert!(out.contains("AllowMissingPositional: true"), "{out}");
        assert!(out.contains("Name: \"optional\""), "{out}");
        assert!(out.contains("Name: \"required\", Required: true"), "{out}");
    }

    #[test]
    fn optional_flag_values_reach_generated_go() {
        let out = go("name \"ex\"\nbin \"ex\"\nflag \"--color [WHEN]\" value_optional=#true\n");
        assert!(
            out.contains("TakesValue: true, ValueOptional: true"),
            "{out}"
        );
    }

    #[test]
    fn explicit_boolean_values_reach_generated_go() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nflag \"--color\" negate=\"--no-color\" bool_value=#true\n",
        );
        assert!(out.contains("BoolValue: true"), "{out}");
        assert!(out.contains("if ev.Flag.BoolValue"), "{out}");
        assert!(
            out.contains("given[ev.Flag.Key] = []string{ev.Value}"),
            "{out}"
        );
        assert!(
            out.contains("(ev.Value == \"true\") != ev.Negated"),
            "{out}"
        );
    }

    #[test]
    fn granular_help_hides_reach_generated_go() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nflag \"--mode <mode>\" hide_default_value=#true hide_env=#true hide_env_values=#true hide_possible_values=#true hide_short_help=#true hide_long_help=#true\n",
        );
        for field in [
            "HideDefaultValue: true",
            "HideEnv: true",
            "HideEnvValues: true",
            "HidePossibleValues: true",
            "HideShortHelp: true",
            "HideLongHelp: true",
        ] {
            assert!(out.contains(field), "missing {field}:\n{out}");
        }
    }

    #[test]
    fn strict_duplicate_policy_reaches_metadata() {
        let permissive = go("name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\"\n");
        assert!(!permissive.contains("RejectDuplicate"), "{permissive}");

        let strict =
            go("name \"ex\"\nbin \"ex\"\nargs_override_self #false\nflag \"--jobs <n>\"\n");
        assert!(strict.contains("RejectDuplicate: true"), "{strict}");
    }

    #[test]
    fn strict_negated_flags_track_each_spelling_separately() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nargs_override_self #false\nflag \"--color\" negate=\"--no-color\"\n",
        );
        assert!(out.contains("polaritySeen := map[uint64]uint8{}"), "{out}");
        assert!(
            out.contains("polaritySeen[ev.Flag.Key]&polarity != 0"),
            "{out}"
        );
        assert!(out.contains("if duplicateSeen[key]"), "{out}");
    }

    #[test]
    fn strict_global_duplicate_tracking_resets_at_subcommands() {
        let out = go(
            "name \"ex\"\nbin \"ex\"\nargs_override_self #false\nflag \"--jobs <n>\" global=#true\ncmd \"run\" {\n  args_override_self #false\n}\n",
        );
        assert!(
            out.contains("levelSeen = map[uint64]int{}"),
            "a subcommand should start a new duplicate scope:\n{out}"
        );
        assert!(out.contains("strictSeen[ev.Flag.Key] = true"), "{out}");
        assert!(out.contains("if strictSeen[key]"), "{out}");
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
        assert_declares_each_constant_once(&out);
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
    arg "<pattern>..." var=#true var_min=2 var_max=2
}
flag "--tag <t>" var=#true var_max=1
"#);
        assert!(
            out.contains("Name: \"include\", Longs: []string{\"include\"}, TakesValue: true, Variadic: true, VarMax: 2"),
            "{out}"
        );
        assert!(
            out.contains("Name: \"include\", Flag: true") && out.contains("VarMin: 2"),
            "the nested value minimum must reach post-binding metadata:\n{out}"
        );
        let tag = out.lines().find(|l| l.contains("\"tag\"")).unwrap();
        assert!(!tag.contains("VarMax"), "occurrence bound leaked: {tag}");
    }

    #[test]
    fn exact_arity_with_one_label_reaches_go_help() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--pair <ITEM>..." {
    arg "<ITEM>..." var=#true var_min=2 var_max=2 {
        value_names "ITEM"
    }
}
arg "<ITEM>..." var=#true var_min=2 var_max=2 {
    value_names "ITEM"
}
"#);
        assert_eq!(out.matches("ValueArity: 2").count(), 2, "{out}");
        assert_eq!(
            out.matches("ValueNames: []string{\"ITEM\"}").count(),
            2,
            "{out}"
        );
    }

    #[test]
    fn allow_hyphen_values_reaches_the_table() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--args <ARGS>" allow_hyphen_values=#true
"#);
        assert!(
            out.contains("Name: \"args\", Longs: []string{\"args\"}, TakesValue: true, AllowHyphenValues: true"),
            "{out}"
        );
    }

    #[test]
    fn require_equals_reaches_the_table() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--inspect <PORT>" require_equals=#true
"#);
        assert!(
            out.contains("Name: \"inspect\", Longs: []string{\"inspect\"}, TakesValue: true, RequireEquals: true"),
            "{out}"
        );
    }

    #[test]
    fn default_missing_reaches_the_table() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--color <WHEN>" default_missing="always"
"#);
        assert!(
            out.contains(
                "Name: \"color\", Longs: []string{\"color\"}, TakesValue: true, DefaultMissing: \"always\""
            ),
            "{out}"
        );
    }

    /// A relationship names a flag by any spelling that reaches it, and from
    /// anywhere the flag is in scope.
    ///
    /// Both halves were silently resolving to nothing, which is worse than an
    /// error: the rule simply never fired, while usage-lib enforced it.
    #[test]
    fn a_relationship_resolves_through_scope_and_negation() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--quiet" global=#true
flag "--color" negate="--no-color"
flag "--plain" conflicts="--no-color"
cmd "run" {
    flag "--loud" conflicts="--quiet"
    flag "--solo" conflicts="--plain"
}
"#);
        // A negation names the flag it belongs to.
        assert!(out.contains("Conflicts: []uint64{FlagColor}"), "{out}");
        // An inherited global is in scope from below.
        assert!(out.contains("Conflicts: []uint64{FlagQuiet}"), "{out}");
        // `--plain` is not global, so from a subcommand it names nothing — the
        // other half, and the one a looser search would get wrong.
        assert!(
            !entry_of(&out, "solo").contains("Conflicts"),
            "a non-global should not resolve from below:\n{out}"
        );
    }

    #[test]
    fn positional_conflicts_reach_go_metadata_in_both_directions() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--from-file <file>" conflicts="value"
arg "[value]" conflicts="--from-file"
"#);

        assert!(
            entry_of(&out, "from-file").contains("Conflicts: []uint64{ArgValue}"),
            "{out}"
        );
        assert!(
            out.lines().any(|line| {
                line.contains("{Key: ArgValue, Name: \"value\"")
                    && line.contains("Conflicts: []uint64{FlagFromFile}")
            }),
            "{out}"
        );
    }

    #[test]
    fn a_value_conditional_requirement_reaches_go_metadata() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--format <format>" {
    requires_if "json" "--schema"
}
flag "--schema <file>"
"#);
        assert!(
            entry_of(&out, "format").contains(
                "RequiresIf: []argv.ValueRequirement{{Value: \"json\", Key: FlagSchema}}"
            ),
            "{out}"
        );
        assert!(
            out.contains("argv.CheckRelationshipsWithValues"),
            "the emitted parser must enforce the metadata:\n{out}"
        );
    }

    #[test]
    fn required_if_eq_makes_generated_go_supply_values() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--token <token>" {
    required_if_eq "--mode" "remote"
}
flag "--mode <mode>"
"#);
        assert!(
            entry_of(&out, "token").contains(
                "RequiredIfEq: []argv.ValueCondition{{Key: FlagMode, Value: \"remote\"}}"
            ),
            "{out}"
        );
        assert!(out.contains("resolved := map[uint64][]string{}"), "{out}");
        assert!(out.contains("argv.CheckRelationshipsWithValues"), "{out}");
    }

    #[test]
    fn boolean_sources_are_normalized_for_value_relationships() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--token <token>" {
    required_if_eq "--mode" "true"
}
flag "--mode" negate="--no-mode" bool_value=#true
"#);
        assert!(
            entry_of(&out, "mode").contains("RequiresIfBoolean: true"),
            "{out}"
        );
    }

    #[test]
    fn a_conditional_default_reaches_go_metadata() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--bin-names" {
    default_if "--json" "true"
    default_if "--output" "json" "pretty"
}
flag "--json"
flag "--output <fmt>"
"#);
        assert!(
            entry_of(&out, "bin-names")
                .contains("DefaultIf: []argv.DefaultIf{{Key: FlagJson, Value: \"true\"}"),
            "{out}"
        );
        assert!(
            entry_of(&out, "bin-names").contains("When: \"json\""),
            "{out}"
        );
        assert!(
            out.contains("argv.ApplyDefaultIf"),
            "the emitted parser must apply the metadata:\n{out}"
        );
        assert!(
            out.contains("negated[ev.Flag.Key] = ev.Negated"),
            "Equals default_if needs the negate form:\n{out}"
        );
    }

    /// The form is part of the name, and usage-lib resolves neither of the
    /// mismatched ones — so resolving them would have a generated CLI enforcing a
    /// rule the reference does not.
    #[test]
    fn a_relationship_needs_the_right_form() {
        let out = go(r#"
name "ex"
bin "ex"
flag "-q --quiet"
flag "--color"
flag "--a" conflicts="--q"
flag "--b" conflicts="-color"
flag "--c" conflicts="-q"
flag "--d" conflicts="--color"
"#);
        // `--q` is not a long form of anything, and `-color` is not a short.
        assert!(!entry_of(&out, "a").contains("Conflicts"), "{out}");
        assert!(!entry_of(&out, "b").contains("Conflicts"), "{out}");
        // The forms the flags actually have.
        assert!(
            entry_of(&out, "c").contains("Conflicts: []uint64{FlagQuiet}"),
            "{out}"
        );
        assert!(
            entry_of(&out, "d").contains("Conflicts: []uint64{FlagColor}"),
            "{out}"
        );
    }

    /// The table has to agree with the binder it feeds.
    ///
    /// The parser tries every long form before any negation, so with `--a`
    /// declaring `negate="--zap"` and a separate `--zap`, typing `--zap` binds
    /// *zap*. A per-candidate search handed the relationship to `a`, which would
    /// have enforced the rule against a flag the command line never binds.
    #[test]
    fn an_ordinary_form_beats_another_flags_negation() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--a" negate="--zap"
flag "--zap"
flag "--p" conflicts="--zap"
"#);
        assert!(
            entry_of(&out, "p").contains("Conflicts: []uint64{FlagZap}"),
            "should name the flag `--zap` binds, not the one negating to it:\n{out}"
        );
    }

    /// A negation is named by the form it was written as, whatever the dashes.
    #[test]
    fn a_single_dash_negation_is_named_by_its_own_form() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--tint" negate="-no-tint"
flag "--plain" conflicts="-no-tint"
flag "--other" conflicts="--no-tint"
"#);
        assert!(
            entry_of(&out, "plain").contains("Conflicts: []uint64{FlagTint}"),
            "the exact form should resolve:\n{out}"
        );
        // And the form it was not written as does not.
        assert!(
            !entry_of(&out, "other").contains("Conflicts"),
            "`--no-tint` is not how it was declared:\n{out}"
        );
    }

    /// A negation is matched as the spec wrote it, dashes and all.
    #[test]
    fn a_negation_is_matched_as_written() {
        let out = go(r#"
name "ex"
bin "ex"
flag "--color" negate="--no-color"
flag "--tint" negate="-no-tint"
flag "--a" conflicts="--no-color"
flag "--b" conflicts="--no-tint"
"#);
        assert!(
            entry_of(&out, "a").contains("Conflicts: []uint64{FlagColor}"),
            "{out}"
        );
        // `--no-tint` is not the form `-no-tint`, so it names nothing — as in
        // usage-lib, which does not resolve it either.
        assert!(!entry_of(&out, "b").contains("Conflicts"), "{out}");
    }

    #[test]
    fn strings_are_escaped_to_go_rules() {
        assert_eq!(go_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(go_string("tab\there"), r#""tab\there""#);
        // Rust would spell this `\u{7f}`, which Go rejects.
        assert_eq!(go_string("\u{7f}"), r#""\x7f""#);
    }
}
