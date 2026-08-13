//! Turning a spec into Rust source that declares the same CLI.
//!
//! The output is checked in rather than built by a `build.rs`, for two reasons: the
//! compile-time comparison wants a fixed input to measure, and a generated file in the
//! tree is a diff a reviewer can read when the derive's vocabulary changes.

use super::*;

/// What a spec property turned into, or why it did not.
///
/// Collected rather than warned about one at a time: a spec of mise's size drops
/// enough on the floor that a reader needs the totals, and silence would read as
/// "everything was expressible".
#[derive(Default)]
struct Skipped {
    counts: BTreeMap<&'static str, usize>,
}

impl Skipped {
    fn note(&mut self, what: &'static str) {
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

/// Which framework's vocabulary to write the CLI in.
///
/// Both dialects are generated from the same spec and the same traversal, so the two
/// shadows declare the same CLI and the comparison is between parsers rather than
/// between two people's transcriptions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Usage,
    Clap,
}

impl Dialect {
    fn as_str(self) -> &'static str {
        match self {
            Dialect::Usage => "usage",
            Dialect::Clap => "clap",
        }
    }

    /// How to spell a field's type.
    ///
    /// clap's derive reads the type as it is *written* — it looks for the token `Option`
    /// — so the clap shadow cannot use absolute paths, while the usage shadow can and
    /// does, since a generated file that leans on the prelude is one a shadowed `Option`
    /// could break.
    fn option(self) -> &'static str {
        match self {
            Dialect::Usage => "::std::option::Option<::std::string::String>",
            Dialect::Clap => "Option<String>",
        }
    }

    fn vec(self) -> &'static str {
        match self {
            Dialect::Usage => "::std::vec::Vec<::std::string::String>",
            Dialect::Clap => "Vec<String>",
        }
    }

    fn string(self) -> &'static str {
        match self {
            Dialect::Usage => "::std::string::String",
            Dialect::Clap => "String",
        }
    }

    /// What a field's attribute list is called.
    fn attr(self) -> &'static str {
        match self {
            Dialect::Usage => "usage",
            Dialect::Clap => "arg",
        }
    }
}

pub fn generate(spec_path: &Path, out_dir: &Path, dialect: Dialect) {
    let text = std::fs::read_to_string(spec_path)
        .unwrap_or_else(|e| fail(&format!("reading {}: {e}", spec_path.display())));
    let spec: Spec = text
        .parse()
        .unwrap_or_else(|e| fail(&format!("parsing {}: {e}", spec_path.display())));

    let (out, skipped) = render(&spec, spec_path, dialect);
    write_crate(out_dir, &spec.bin, &out, dialect);

    println!(
        "generated {} from {} ({} commands, {} flags, {} args)",
        out_dir.join("src/lib.rs").display(),
        spec_path.display(),
        count(&spec.cmd, |_| 1),
        count(&spec.cmd, |c| c.flags.len()),
        count(&spec.cmd, |c| c.args.len()),
    );
    skipped.report(dialect);
}

/// The crate's source, and what the spec said that it could not carry.
///
/// Separate from [`generate`] so it can be tested without writing anything: what a
/// dialect does with a given spec is the part worth asserting on.
fn render(spec: &Spec, spec_path: &Path, dialect: Dialect) -> (String, Skipped) {
    let mut skipped = Skipped::default();
    let mut names = Names::default();
    let mut out = String::new();

    // Properties of the CLI as a whole that the derive has no way to declare. The root's
    // grammar differs without them: mise sets `default_subcommand run`, so `mise build`
    // routes through `run` there and fills the root's own `[TASK]` here.

    header(&mut out, spec_path, dialect);
    let root = Type::root(&spec.cmd, &mut names);
    emit_command(
        &mut out,
        &spec.cmd,
        &root,
        true,
        &mut Run {
            bin: &spec.bin,
            default_subcommand: spec.default_subcommand.as_deref(),
            about: spec.about.as_deref(),
            about_long: spec.about_long.as_deref(),
            dialect,
            skipped: &mut skipped,
            names: &mut names,
        },
    );
    (out, skipped)
}

/// Walk the tree, adding up whatever a closure counts.
fn count(cmd: &SpecCommand, f: impl Fn(&SpecCommand) -> usize + Copy) -> usize {
    f(cmd) + cmd.subcommands.values().map(|c| count(c, f)).sum::<usize>()
}

fn header(out: &mut String, spec_path: &Path, dialect: Dialect) {
    let name = spec_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let imports = match dialect {
        Dialect::Usage => "use usage_derive::{Args, Cli, Subcommands};",
        Dialect::Clap => "use clap::{Args, Parser, Subcommand};",
    };
    let what = dialect.as_str();
    out.push_str(&format!(
        "//! A shadow of `{name}` in {what}'s vocabulary, generated by\n\
         //! `cargo run -p xtask -- gen-shadow`.\n\
         //!\n\
         //! Do not edit: regenerate it. It exists to be compiled and parsed against, so\n\
         //! that the parser can be measured at a real CLI's scale rather than a toy one.\n\
         #![allow(dead_code)]\n\n\
         {imports}\n\n"
    ));
}

/// A generated type's name, and the names already taken.
#[derive(Default)]
struct Names {
    taken: HashSet<String>,
}

impl Names {
    /// A unique CamelCase type name for a command path.
    ///
    /// The path rather than the last segment: mise has `mise settings set` and
    /// `mise config set`, and two `Set` structs would collide. A numeric suffix is the
    /// last resort, for names that collide only after sanitizing.
    fn claim(&mut self, path: &[String], suffix: &str) -> String {
        let base: String = path.iter().map(|p| camel(p)).collect::<String>();
        let mut name = format!("{base}{suffix}");
        if name.is_empty() {
            name = format!("Cmd{suffix}");
        }
        if self.taken.insert(name.clone()) {
            return name;
        }
        for n in 2.. {
            let candidate = format!("{name}{n}");
            if self.taken.insert(candidate.clone()) {
                return candidate;
            }
        }
        unreachable!("the loop above only ends by returning")
    }
}

/// The pair of type names a command needs: its arguments, and its subcommands.
struct Type {
    args: String,
    subcommands: String,
    path: Vec<String>,
}

impl Type {
    fn root(cmd: &SpecCommand, names: &mut Names) -> Self {
        let path = vec![];
        Self {
            args: names.claim(&path, "Cli"),
            subcommands: if cmd.subcommands.is_empty() {
                String::new()
            } else {
                names.claim(&path, "Commands")
            },
            path,
        }
    }

    fn child(parent: &Type, name: &str, cmd: &SpecCommand, names: &mut Names) -> Self {
        let mut path = parent.path.clone();
        path.push(name.to_string());
        Self {
            args: names.claim(&path, "Args"),
            subcommands: if cmd.subcommands.is_empty() {
                String::new()
            } else {
                names.claim(&path, "Commands")
            },
            path,
        }
    }
}

/// What every command in one run of the generator shares.
///
/// These five travelled as five parameters, which is how the signature got long enough to
/// need a clippy exclusion. They belong together: none of them varies per command.
struct Run<'a> {
    bin: &'a str,
    /// Only the root has one, and only it declares it.
    default_subcommand: Option<&'a str>,
    /// The spec's own description, which belongs to the root.
    about: Option<&'a str>,
    about_long: Option<&'a str>,
    dialect: Dialect,
    skipped: &'a mut Skipped,
    names: &'a mut Names,
}

fn emit_command(out: &mut String, cmd: &SpecCommand, ty: &Type, is_root: bool, run: &mut Run) {
    let (bin, dialect) = (run.bin, run.dialect);
    // Children first, so a reader meets a type before the struct that holds it. The
    // names are claimed here too, since the parent's `subcommand` field needs them.
    let children: Vec<(&String, &SpecCommand, Type)> = cmd
        .subcommands
        .iter()
        // Aliases appear in the map alongside the name they point at; generating both
        // would declare the same command twice. The alias itself is a loss — usage-argv
        // matches aliases, the derive has no way to declare one — so it is counted
        // below rather than passed over in silence.
        .filter(|(name, sub)| sub.name == **name)
        .map(|(name, sub)| (name, sub, Type::child(ty, name, sub, run.names)))
        .collect();
    for (_, sub, _) in &children {
        if sub.mounts.len() > 1 {
            run.skipped.note("a command's second and later mounts");
        }
    }
    for (_, sub, sub_ty) in &children {
        // `run` travels down unchanged; `default_subcommand` is read under an `is_root`
        // guard, so a child cannot pick up the root's.
        emit_command(out, sub, sub_ty, false, run);
    }

    // The root's own description is the *spec's* `about`, not the root command's help — a
    // spec puts it at the top level, and the root command usually has none. Taken from the
    // command first all the same, since a spec may say both.
    let (about, about_long) = if is_root {
        (
            cmd.help.as_deref().or(run.about),
            cmd.help_long.as_deref().or(run.about_long),
        )
    } else {
        (cmd.help.as_deref(), cmd.help_long.as_deref())
    };
    doc_comment(out, about, about_long, 0);
    // The command-level properties. The usage dialect declares them; clap has no way to say
    // any of them, so on that side they are counted as dropped — the shadow would otherwise
    // look more faithful than it is.
    let mut usage_opts: Vec<String> = Vec::new();
    if is_root {
        usage_opts.push(format!("bin = {bin:?}"));
    }
    for (present, declaration, what) in [
        (
            is_root && run.default_subcommand.is_some(),
            run.default_subcommand
                .map(|d| format!("default_subcommand = {d:?}")),
            "`default_subcommand` on a command",
        ),
        (
            cmd.restart_token.is_some(),
            cmd.restart_token
                .as_ref()
                .map(|t| format!("restart_token = {t:?}")),
            "a `restart_token` on a command",
        ),
        (
            !cmd.mounts.is_empty(),
            cmd.mounts.first().map(|m| format!("mount = {:?}", m.run)),
            "a `mount` on a command",
        ),
    ] {
        if !present {
            continue;
        }
        match dialect {
            Dialect::Usage => usage_opts.extend(declaration),
            Dialect::Clap => run.skipped.note(what),
        }
    }
    match (is_root, dialect) {
        (true, Dialect::Usage) => {
            out.push_str("#[derive(Cli)]\n");
            out.push_str(&format!("#[usage({})]\n", usage_opts.join(", ")));
        }
        (true, Dialect::Clap) => {
            out.push_str("#[derive(Parser)]\n");
            out.push_str(&format!("#[command(name = {bin:?})]\n"));
        }
        (false, Dialect::Usage) => {
            out.push_str("#[derive(Args)]\n");
            if !usage_opts.is_empty() {
                out.push_str(&format!("#[usage({})]\n", usage_opts.join(", ")));
            }
        }
        (false, Dialect::Clap) => out.push_str("#[derive(Args)]\n"),
    }
    out.push_str(&format!("pub struct {} {{\n", ty.args));

    // Field names are claimed before anything is written, because clap names a
    // relationship by the *field* it points at while the spec names it by the flag —
    // `conflicts_with = "dry_run"` against `conflicts="--dry-run"` — so resolving one
    // needs every name in the struct already decided.
    let mut fields = FieldNames::default();
    let flags: Vec<(&SpecFlag, Option<String>, String)> = cmd
        .flags
        .iter()
        .filter_map(|flag| {
            let long = flag.long.first().cloned();
            if long.is_none() && flag.short.is_empty() {
                run.skipped.note("a flag with no long or short form");
                return None;
            }
            let field = fields.claim(long.as_deref().unwrap_or(&flag.name));
            Some((flag, long, field))
        })
        .collect();
    let ids: BTreeMap<String, String> = flags
        .iter()
        .flat_map(|(flag, _, field)| {
            flag.long
                .iter()
                .map(|l| format!("--{l}"))
                .chain(flag.short.iter().map(|s| format!("-{s}")))
                .map(move |selector| (selector, field.clone()))
        })
        .collect();
    for (flag, long, field) in &flags {
        emit_flag(out, flag, long.clone(), field, dialect, &ids, run.skipped);
    }
    for arg in &cmd.args {
        let field = fields.claim(&arg.name);
        emit_arg(out, arg, &field, dialect, run.skipped);
    }
    if !ty.subcommands.is_empty() {
        // A bare `T` says a subcommand is required and an `Option<T>` says it may be
        // left out, which is the same distinction the spec draws with
        // `subcommand_required`. Reading it matters: a shadow that accepted
        // `mise bootstrap linux` with nothing after it would be answering a different
        // grammar from the one being measured.
        match dialect {
            Dialect::Usage => out.push_str("    #[usage(subcommand)]\n"),
            Dialect::Clap => out.push_str("    #[command(subcommand)]\n"),
        }
        let field = match (cmd.subcommand_required, dialect) {
            (true, _) => format!("    pub command: {},\n", ty.subcommands),
            (false, Dialect::Usage) => format!(
                "    pub command: ::std::option::Option<{}>,\n",
                ty.subcommands
            ),
            (false, Dialect::Clap) => {
                format!("    pub command: Option<{}>,\n", ty.subcommands)
            }
        };
        out.push_str(&field);
    }
    out.push_str("}\n\n");

    if !ty.subcommands.is_empty() {
        match dialect {
            Dialect::Usage => out.push_str("#[derive(Subcommands)]\n"),
            Dialect::Clap => out.push_str("#[derive(Subcommand)]\n"),
        }
        out.push_str(&format!("pub enum {} {{\n", ty.subcommands));
        for (name, sub, sub_ty) in &children {
            doc_comment(out, sub.help.as_deref(), None, 1);
            let variant = camel(name);
            // Written whenever the variant name is not the command name: both derives
            // would otherwise kebab-case the variant and mostly get it right, and
            // "mostly" is not a grammar.
            let mut opts: Vec<String> = Vec::new();
            if variant != **name {
                opts.push(format!("name = {name:?}"));
            }
            // Both frameworks can declare these, so both shadows carry them: 91 of mise's
            // commands answer to a second name, and a shadow that rejected `mise i` would
            // be measuring a smaller CLI than the one it claims to shadow.
            match dialect {
                Dialect::Usage => {
                    opts.extend(declared_help(sub.help.as_deref(), sub.help_long.as_deref()));
                    // `hide=#true` on a `cmd`: the command works and is not advertised. mise
                    // hides eight, `asdf` and `dotfiles` among them, and help listed every one
                    // of them before the variant could say so.
                    if sub.hide {
                        opts.push("hide".into());
                    }
                    if !sub.aliases.is_empty() {
                        opts.push(alias_list("alias", &sub.aliases));
                    }
                    if !sub.hidden_aliases.is_empty() {
                        opts.push(alias_list("alias_hidden", &sub.hidden_aliases));
                    }
                }
                // clap spells one and several differently, and repeating the singular
                // form is not the same as the plural one.
                Dialect::Clap => {
                    opts.extend(declared_help_clap(
                        sub.help.as_deref(),
                        sub.help_long.as_deref(),
                        true,
                    ));
                    match sub.aliases.as_slice() {
                        [] => {}
                        [one] => opts.push(format!("visible_alias = {one:?}")),
                        many => opts.push(format!("visible_aliases = [{}]", quoted_list(many))),
                    }
                    match sub.hidden_aliases.as_slice() {
                        [] => {}
                        [one] => opts.push(format!("alias = {one:?}")),
                        many => opts.push(format!("aliases = [{}]", quoted_list(many))),
                    }
                }
            }
            if !opts.is_empty() {
                let attr = match dialect {
                    Dialect::Usage => "usage",
                    Dialect::Clap => "command",
                };
                out.push_str(&format!("    #[{attr}({})]\n", opts.join(", ")));
            }
            // Boxed, as mise boxes its own largest commands: an enum is as large as its
            // biggest variant, so one thirty-flag subcommand makes every invocation move
            // that much stack. It is also what keeps `clippy::large_enum_variant` quiet,
            // which is what kept the shadow out of the workspace.
            out.push_str(&format!("    {variant}(Box<{}>),\n", sub_ty.args));
        }
        out.push_str("}\n\n");
    }
}

/// Field names already used in the struct being written.
#[derive(Default)]
struct FieldNames {
    taken: HashSet<String>,
}

impl FieldNames {
    fn claim(&mut self, from: &str) -> String {
        let mut name = snake(from);
        if name.is_empty() || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            name = format!("v{name}");
        }
        if is_keyword(&name) {
            name = format!("{name}_");
        }
        if self.taken.insert(name.clone()) {
            return name;
        }
        for n in 2.. {
            let candidate = format!("{name}_{n}");
            if self.taken.insert(candidate.clone()) {
                return candidate;
            }
        }
        unreachable!("the loop above only ends by returning")
    }
}

/// The Rust type a flag's values land in, which is the same in either dialect.
///
/// Both derives read the type as part of the declaration — a `bool` is a switch, a
/// `Vec` collects, an `Option` may be absent — so this is where "what kind of flag is
/// this" is decided, once.
fn flag_type(flag: &SpecFlag, dialect: Dialect) -> &'static str {
    let Some(arg) = flag.arg.as_ref() else {
        return if flag.count { "u8" } else { "bool" };
    };
    if flag.count {
        "u8"
    } else if flag.var || arg.var {
        dialect.vec()
    } else if flag.required {
        dialect.string()
    } else {
        dialect.option()
    }
}

fn arg_type(arg: &SpecArg, dialect: Dialect) -> &'static str {
    if arg.var {
        dialect.vec()
    } else if arg.required {
        dialect.string()
    } else {
        dialect.option()
    }
}

/// The defaults a flag declares, which may sit on the flag or on its argument.
fn flag_defaults(flag: &SpecFlag) -> &[String] {
    match flag.arg.as_ref() {
        Some(arg) if !arg.default.is_empty() => &arg.default,
        _ => &flag.default,
    }
}

fn emit_flag(
    out: &mut String,
    flag: &SpecFlag,
    long: Option<String>,
    field: &str,
    dialect: Dialect,
    ids: &BTreeMap<String, String>,
    skipped: &mut Skipped,
) {
    doc_comment(out, flag.help.as_deref(), flag.help_long.as_deref(), 1);
    let ty = flag_type(flag, dialect);
    let opts = match dialect {
        Dialect::Usage => usage_flag_opts(flag, long.as_deref(), ty, skipped),
        Dialect::Clap => clap_flag_opts(flag, long.as_deref(), ids, skipped),
    };
    writeln!(out, "    #[{}({})]", dialect.attr(), opts.join(", ")).expect("writing to a String");
    writeln!(out, "    pub {field}: {ty},").expect("writing to a String");
}

fn usage_flag_opts(
    flag: &SpecFlag,
    long: Option<&str>,
    ty: &str,
    skipped: &mut Skipped,
) -> Vec<String> {
    // `flag.help_long`, not `long` — that is the flag's long *name*, which every other caller of
    // this function gets right. Passing it here emitted `long_help = "shims"` for a flag called
    // `--shims`, so the shadow dropped the real long help and its regenerated spec said the
    // flag's own name where the source said a paragraph.
    let mut opts: Vec<String> = declared_help(flag.help.as_deref(), flag.help_long.as_deref());
    // Written out rather than bare, because the field name may have been sanitized —
    // `--type` becomes `type_`, and a bare `long` would rename the flag.
    if let Some(long) = long {
        opts.push(format!("long = {long:?}"));
    }
    if let Some(short) = flag.short.first() {
        opts.push(format!("short = {short:?}"));
    }
    if flag.long.len() > 1 || flag.short.len() > 1 {
        skipped.note("a flag's second long or short form");
    }
    if let Some(negate) = &flag.negate {
        opts.push(format!("negate = {negate:?}"));
    }
    if flag.global {
        opts.push("global".into());
    }
    if flag.hide {
        opts.push("hide".into());
    }
    if flag.count {
        opts.push("count".into());
    }
    if let Some(env) = &flag.env {
        opts.push(format!("env = {env:?}"));
    }
    if let Some(heading) = &flag.help_heading {
        opts.push(format!("help_heading = {heading:?}"));
    }
    if !flag.overrides.is_empty() {
        opts.push(selector_list("overrides", &flag.overrides));
    }
    if !flag.conflicts.is_empty() {
        opts.push(selector_list("conflicts", &flag.conflicts));
    }
    if !flag.required_if.is_empty() {
        opts.push(selector_list("required_if", &flag.required_if));
    }
    // A repeatable flag that takes no value: `-v --verbose var=#true count=#true`. The
    // `var` was reaching the shadow only through the branch below, which a valueless flag
    // never enters — so a spec that said the flag could be given again came back saying it
    // could not. `count` implies it now, but a plain repeatable switch still has to say so.
    if flag.arg.is_none() && flag.var && !flag.count {
        opts.push("var".into());
    }
    if let Some(arg) = flag.arg.as_ref() {
        // The placeholder for the value: `TOOL` in `--tool <TOOL>`. Without it the flag's own
        // name stands in, so `--tool <TOOL>` came back as `--tool <tool>` — 14 of mise's
        // flags read differently in help for no reason but this.
        if arg.name != flag.name {
            opts.push(format!("value_name = {:?}", arg.name));
        }
        if let Some(choices) = &arg.choices {
            opts.push(format!("choices({})", quoted_list(&choices.choices)));
        }
        // Not on a collecting field: the derive refuses a default it would write into the
        // spec and then never apply.
        let defaults = flag_defaults(flag);
        if flag.var || arg.var {
            if !defaults.is_empty() {
                skipped.note("a default on a flag that collects values");
            }
            if flag.var {
                opts.push("var".into());
            }
            // Required, said the only way a collecting flag can say it. A scalar flag says it
            // by *type* — `String` rather than `Option<String>` — and a `Vec` has no such
            // spelling, so without this a spec demanding one-or-more values came back as a
            // shadow that accepted none, describing `[VALUE]…` where the source said `…`.
            if flag.required {
                opts.push("required".into());
            }
            if arg.var {
                // A variadic argument on a flag: one occurrence taking several values,
                // which is a different claim from a repeatable flag.
                opts.push("variadic".into());
            }
            // The spec can bound a repeatable flag on the flag itself as well as on its
            // argument, and usage-lib enforces both against the values collected. The
            // derive has one pair of bounds, so the flag's own win and a second,
            // differing pair is a loss worth counting.
            if let Some(min) = flag.var_min.or(arg.var_min) {
                opts.push(format!("var_min = {min}"));
            }
            if let Some(max) = flag.var_max.or(arg.var_max) {
                opts.push(format!("var_max = {max}"));
            }
            if (flag.var_min.is_some() && arg.var_min.is_some())
                || (flag.var_max.is_some() && arg.var_max.is_some())
            {
                skipped.note("a bound on both a flag and its argument");
            }
        } else {
            if let Some(default) = defaults.first() {
                opts.push(format!("default = {default:?}"));
            }
            if defaults.len() > 1 {
                skipped.note("a flag's second and later defaults");
            }
        }
    }
    // `required_unless` needs somewhere to put "absent", so it only goes on an `Option`.
    // A required flag that also carries one is a contradiction the spec allows and the
    // derive does not.
    if !flag.required_unless.is_empty() {
        if ty.contains("Option<") {
            opts.push(selector_list("required_unless", &flag.required_unless));
        } else {
            skipped.note("`required_unless` on a flag whose type is always filled");
        }
    }
    if flag.required && flag.arg.is_none() {
        skipped.note("`required` on a flag that takes no value");
    }

    opts
}

fn clap_flag_opts(
    flag: &SpecFlag,
    long: Option<&str>,
    ids: &BTreeMap<String, String>,
    skipped: &mut Skipped,
) -> Vec<String> {
    let mut opts: Vec<String> =
        declared_help_clap(flag.help.as_deref(), flag.help_long.as_deref(), false);
    if let Some(long) = long {
        opts.push(format!("long = {long:?}"));
    }
    if let Some(short) = flag.short.first() {
        opts.push(format!("short = {short:?}"));
    }
    // Deliberately dropped on both sides: the usage derive cannot declare a second long
    // form, and giving clap one would make it answer a larger grammar than the shadow it
    // is being compared against.
    if flag.long.len() > 1 || flag.short.len() > 1 {
        skipped.note("a flag's second long or short form");
    }
    // clap spells a negation as a second argument with `ArgAction::SetFalse`, not as a
    // property of this one, so it is out of reach of a field-by-field translation.
    if flag.negate.is_some() {
        skipped.note("a negated flag");
    }
    if flag.global {
        opts.push("global = true".into());
    }
    if flag.hide {
        opts.push("hide = true".into());
    }
    if flag.count {
        opts.push("action = ::clap::ArgAction::Count".into());
    }
    if let Some(env) = &flag.env {
        opts.push(format!("env = {env:?}"));
    }
    if let Some(heading) = &flag.help_heading {
        opts.push(format!("help_heading = {heading:?}"));
    }
    // A relationship names the field it points at, so a selector the struct does not
    // declare is dropped rather than guessed at.
    for (option, selectors) in [
        ("overrides_with", &flag.overrides),
        ("conflicts_with", &flag.conflicts),
        ("required_unless_present", &flag.required_unless),
    ] {
        for selector in selectors {
            match ids.get(selector) {
                Some(id) => opts.push(format!("{option} = {id:?}")),
                None => skipped.note("a relationship naming a flag of another command"),
            }
        }
    }
    // clap's nearest equivalent is `required_if_eq`, which asks what the other flag's
    // *value* is rather than whether it was given at all.
    if !flag.required_if.is_empty() {
        skipped.note("`required_if` on a flag");
    }
    if let Some(arg) = flag.arg.as_ref() {
        opts.push(format!("value_name = {:?}", arg.name));
        if let Some(choices) = &arg.choices {
            opts.push(format!(
                "value_parser = ::clap::builder::PossibleValuesParser::new([{}])",
                quoted_list(&choices.choices)
            ));
        }
        let defaults = flag_defaults(flag);
        if let Some(default) = defaults.first() {
            opts.push(format!("default_value = {default:?}"));
        }
        if defaults.len() > 1 {
            skipped.note("a flag's second and later defaults");
        }
        // A `Vec` already appends in clap; what needs saying is the other kind of
        // collecting, where one occurrence keeps taking values.
        if arg.var {
            let least = arg.var_min.unwrap_or(1);
            match arg.var_max {
                Some(max) => opts.push(format!("num_args = {least}..={max}")),
                None => opts.push(format!("num_args = {least}..")),
            }
        }
        if flag.required {
            opts.push("required = true".into());
        }
    } else if flag.required {
        skipped.note("`required` on a flag that takes no value");
    }
    opts
}

fn emit_arg(out: &mut String, arg: &SpecArg, field: &str, dialect: Dialect, skipped: &mut Skipped) {
    doc_comment(out, arg.help.as_deref(), arg.help_long.as_deref(), 1);
    let ty = arg_type(arg, dialect);
    let opts = match dialect {
        Dialect::Usage => usage_arg_opts(arg, skipped),
        Dialect::Clap => clap_arg_opts(arg, skipped),
    };
    writeln!(out, "    #[{}({})]", dialect.attr(), opts.join(", ")).expect("writing to a String");
    writeln!(out, "    pub {field}: {ty},").expect("writing to a String");
}

fn usage_arg_opts(arg: &SpecArg, skipped: &mut Skipped) -> Vec<String> {
    let mut opts: Vec<String> = vec!["arg".into(), format!("name = {:?}", arg.name)];
    opts.extend(declared_help(arg.help.as_deref(), arg.help_long.as_deref()));
    if arg.hide {
        opts.push("hide".into());
    }
    // An argument can be backed by an environment variable and grouped under a heading
    // just as a flag can, and both reach `ArgMeta` — leaving them out made the shadow's
    // grammar quietly differ from the spec's for any spec that used them.
    if let Some(env) = &arg.env {
        opts.push(format!("env = {env:?}"));
    }
    if let Some(heading) = &arg.help_heading {
        opts.push(format!("help_heading = {heading:?}"));
    }
    if let Some(choices) = &arg.choices {
        opts.push(format!("choices({})", quoted_list(&choices.choices)));
    }
    // A default on a collecting field is refused by the derive: it would be written into
    // the spec and then never applied, which is worse than not saying it.
    if arg.var {
        if !arg.default.is_empty() {
            skipped.note("a default on a variadic argument");
        }
    } else {
        if let Some(default) = arg.default.first() {
            opts.push(format!("default = {default:?}"));
        }
        if arg.default.len() > 1 {
            skipped.note("an argument's second and later defaults");
        }
    }
    if arg.var {
        // `<TARGET>…` means one or more. A `Vec` field cannot say that — it has no bare form
        // to contrast with an `Option` — so it takes an explicit `required`, without which the
        // shadow described `[TARGET]…` for the eight of mise's arguments that demand a value.
        if arg.required {
            opts.push("required".into());
        }
        if let Some(min) = arg.var_min {
            opts.push(format!("var_min = {min}"));
        }
        if let Some(max) = arg.var_max {
            opts.push(format!("var_max = {max}"));
        }
    }
    double_dash(arg, skipped, |mode| {
        opts.push(format!("double_dash = {mode:?}"))
    });
    opts
}

fn clap_arg_opts(arg: &SpecArg, skipped: &mut Skipped) -> Vec<String> {
    let mut opts: Vec<String> = vec![format!("value_name = {:?}", arg.name)];
    opts.extend(declared_help_clap(
        arg.help.as_deref(),
        arg.help_long.as_deref(),
        false,
    ));
    if arg.hide {
        opts.push("hide = true".into());
    }
    if let Some(choices) = &arg.choices {
        opts.push(format!(
            "value_parser = ::clap::builder::PossibleValuesParser::new([{}])",
            quoted_list(&choices.choices)
        ));
    }
    if let Some(default) = arg.default.first() {
        opts.push(format!("default_value = {default:?}"));
    }
    if arg.default.len() > 1 {
        skipped.note("an argument's second and later defaults");
    }
    if arg.var {
        let least = arg.var_min.unwrap_or(0);
        match arg.var_max {
            Some(max) => opts.push(format!("num_args = {least}..={max}")),
            None => opts.push(format!("num_args = {least}..")),
        }
    }
    // clap calls it `last`: the argument after the `--`.
    double_dash(arg, skipped, |_| opts.push("last = true".into()));
    opts
}

/// The one `double_dash` mode both dialects can express, and notes for the rest.
fn double_dash(arg: &SpecArg, skipped: &mut Skipped, mut required: impl FnMut(&str)) {
    match arg.double_dash {
        // The default: a `--` is allowed and changes nothing about where words land.
        usage::SpecDoubleDashChoices::Optional => {}
        usage::SpecDoubleDashChoices::Required => required("required"),
        usage::SpecDoubleDashChoices::Automatic => {
            skipped.note("`double_dash = \"automatic\"` on an argument")
        }
        usage::SpecDoubleDashChoices::Preserve => {
            skipped.note("`double_dash = \"preserve\"` on an argument")
        }
    }
}

/// `"a", "b"` — a list of string literals, as either dialect writes one.
fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("{v:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `alias("a", "b")`, or `alias = "a"` for one.
fn alias_list(option: &str, aliases: &[String]) -> String {
    if let [one] = aliases {
        return format!("{option} = {one:?}");
    }
    format!("{option}({})", quoted_list(aliases))
}

/// `conflicts("--a", "--b")`, or `conflicts = "--a"` for one.
fn selector_list(option: &str, selectors: &[String]) -> String {
    if let [one] = selectors {
        return format!("{option} = {one:?}");
    }
    let list = selectors
        .iter()
        .map(|s| format!("{s:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{option}({list})")
}

/// Help text as a doc comment, which is how the derive reads it.
/// Whether help text has to be declared rather than written as a comment.
///
/// A doc comment's first paragraph is read the way Rust reads one, so a line break inside it
/// becomes a space. mise's specs use multi-line `help` on 37 commands and flags, and every one
/// of them came back with its lines run together — so where the break is part of the text, the
/// generated code declares it instead.
fn needs_declaring(help: Option<&str>) -> bool {
    help.map(|h| h.trim().contains('\n')).unwrap_or(false)
}

/// The same declarations in clap's vocabulary.
///
/// clap takes `help`/`long_help` on an argument and `about`/`long_about` on a command, so there
/// is nothing it cannot say — but it does not read the *usage* attribute list. Skipping the
/// comment without writing clap's own left the clap shadow with no text at all for every command
/// and flag whose help a comment cannot carry, and *uncounted*, which is the silent-drop class
/// this generator exists to prevent.
fn declared_help_clap(help: Option<&str>, long: Option<&str>, command: bool) -> Vec<String> {
    let mut opts = Vec::new();
    if !needs_declaring(help) {
        return opts;
    }
    let (short_key, long_key) = if command {
        ("about", "long_about")
    } else {
        ("help", "long_help")
    };
    if let Some(help) = help.filter(|h| !h.trim().is_empty()) {
        opts.push(format!("{short_key} = {:?}", help.trim()));
    }
    if let Some(long) = long.filter(|l| !l.trim().is_empty()) {
        opts.push(format!("{long_key} = {:?}", long.trim_end()));
    }
    opts
}

/// The `help = "..."` option for text a comment would reflow.
fn declared_help(help: Option<&str>, long: Option<&str>) -> Vec<String> {
    let mut opts = Vec::new();
    if let Some(help) = help.filter(|h| needs_declaring(Some(h))) {
        opts.push(format!("help = {:?}", help.trim()));
        // The long form goes with it: read from the comment, it would be measured against a
        // short form that no longer matches, and written in full twice over.
        if let Some(long) = long.filter(|l| !l.trim().is_empty()) {
            opts.push(format!("long_help = {:?}", long.trim_end()));
        }
    }
    opts
}

fn doc_comment(out: &mut String, help: Option<&str>, long: Option<&str>, depth: usize) {
    let indent = "    ".repeat(depth);
    // Declared instead, by `declared_help`.
    if needs_declaring(help) {
        return;
    }
    let Some(help) = help.filter(|h| !h.trim().is_empty()) else {
        // Nothing at all where the spec says nothing. A placeholder was standing in — the
        // derive does not require help text — and it became *real* help: the shadow told a
        // reader that `--output` was "Undocumented" where mise's spec leaves it bare, which a
        // fixture for comparing help output cannot do.
        return;
    };
    for line in help.lines() {
        writeln!(out, "{indent}/// {}", escape_doc(line)).expect("writing to a String");
    }
    // The long form is the whole comment, so it follows a blank line — and only what it
    // says beyond the short form.
    //
    // A spec's long help usually *opens* with its short help, which is the convention the
    // derive reads back: first paragraph short, whole comment long. Writing both in full
    // repeated that opening paragraph on nearly every command mise has, so only the
    // remainder goes here.
    let Some(long) = long.map(str::trim_end).filter(|l| !l.trim().is_empty()) else {
        return;
    };
    // Nothing but punctuation between them — mise's specs often end the long form's
    // first sentence with a period the short form leaves off.
    if long.trim().trim_end_matches('.') == help.trim().trim_end_matches('.') {
        return;
    }
    let rest = match long.trim_start().strip_prefix(help.trim()) {
        // What is left after the short form: the orphaned period and the blank line that
        // separated them go, and nothing else — trimming the whole remainder would take
        // the indentation off an example, and mise's help is full of them.
        Some(rest) => rest.trim_start_matches(['.', '\n', '\r']).trim_end(),
        // The long form does not open with the short one, so it is written in full and
        // the repetition, if any, is the spec's.
        None => long.trim_start(),
    };
    if rest.is_empty() {
        return;
    }
    writeln!(out, "{indent}///").expect("writing to a String");
    for line in rest.lines() {
        writeln!(out, "{indent}/// {}", escape_doc(line)).expect("writing to a String");
    }
}

/// Help text, as a doc comment can carry it.
///
/// Verbatim but for carriage returns. The crate turns doctests off, so a code fence or
/// an indented example in help text is text rather than something to compile — and a
/// shadow is more useful the closer its help is to the real thing.
fn escape_doc(line: &str) -> String {
    line.replace('\r', "")
}

fn write_crate(dir: &Path, bin: &str, lib: &str, dialect: Dialect) {
    let src = dir.join("src");
    std::fs::create_dir_all(&src)
        .unwrap_or_else(|e| fail(&format!("creating {}: {e}", src.display())));
    let name = match dialect {
        Dialect::Usage => format!("shadow-{}", snake(bin).replace('_', "-")),
        Dialect::Clap => format!("shadow-{}-clap", snake(bin).replace('_', "-")),
    };
    let deps = match dialect {
        Dialect::Usage => {
            "usage-argv = { path = \"../../../argv\", features = [\"spec\"] }\n\
                           usage-derive = { path = \"../../../derive\" }\n"
        }
        // The features clap's derive needs and nothing more, since anything else would be
        // weight the comparison did not ask for.
        Dialect::Clap => "clap = { version = \"4\", features = [\"derive\", \"env\"] }\n",
    };
    let manifest = format!(
        "# Generated by `cargo run -p xtask -- gen-shadow`. Do not edit.\n\
         [package]\n\
         name = {name:?}\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\n\
         [lib]\n\
         # The doc comments are copied help text, and mise's help is full of indented\n\
         # examples — which Rust would otherwise compile as doctests.\n\
         doctest = false\n\n\
         [dependencies]\n\
         {deps}\n\
         [lints.rust]\n\
         # The shadow is generated, so a warning here is a message to the generator\n\
         # rather than to a reader of this crate.\n\
         missing_docs = \"allow\"\n"
    );
    write(&dir.join("Cargo.toml"), &manifest);
    let lib_path = src.join("lib.rs");
    write(&lib_path, lib);
    // Formatted here rather than left to whoever notices: the file is checked in, so
    // `cargo fmt --check` sees it, and help text copied out of a spec carries trailing
    // whitespace on its blank lines.
    rustfmt(&lib_path);
}

fn rustfmt(path: &Path) {
    match std::process::Command::new("rustfmt")
        .args(["--edition", "2021"])
        .arg(path)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => fail(&format!("rustfmt exited with {status}")),
        Err(e) => fail(&format!("running rustfmt: {e}")),
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents)
        .unwrap_or_else(|e| fail(&format!("writing {}: {e}", path.display())));
}

fn camel(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

fn snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// Rust keywords, which a spec is free to use as a flag name — mise has `--type`.
fn is_keyword(name: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "try",
        "type", "union", "unsafe", "use", "where", "while", "yield",
    ];
    KEYWORDS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(spec: &str) -> (String, Skipped) {
        rendered_as(spec, Dialect::Usage)
    }

    fn rendered_as(spec: &str, dialect: Dialect) -> (String, Skipped) {
        let spec: Spec = spec.parse().expect("valid spec");
        render(&spec, Path::new("probe.usage.kdl"), dialect)
    }

    #[test]
    fn an_argument_carries_its_env_and_heading() {
        // Not exercised by mise's spec, which puts `env` only on flags — so it is
        // asserted here rather than assumed from a generated file that happens not to
        // contain one.
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\narg \"[file]\" env=\"EX_FILE\" help_heading=\"Input\"\n",
        );
        assert!(out.contains(r#"env = "EX_FILE""#), "{out}");
        assert!(out.contains(r#"help_heading = "Input""#), "{out}");
    }

    #[test]
    fn both_dialects_carry_help_a_comment_cannot() {
        // Multi-line help is skipped as a comment for *both* dialects, and only one of them was
        // writing it as an attribute — so the clap shadow had no text for those flags at all,
        // uncounted. clap can say it: `help` and `long_help` on an argument.
        let spec = "name \"ex\"\nbin \"ex\"\nflag \"--shims\" help=#\"\"\"\nUse shims\nlike so:\n\"\"\"#\n";

        let (usage, _) = rendered_as(spec, Dialect::Usage);
        assert!(usage.contains(r#"help = "Use shims\nlike so:""#), "{usage}");

        let (clap, _) = rendered_as(spec, Dialect::Clap);
        assert!(clap.contains(r#"help = "Use shims\nlike so:""#), "{clap}");
    }

    #[test]
    fn a_flags_long_help_is_its_long_help_and_not_its_name() {
        // The long *name* was passed where the long *help* belongs, so a flag called `--shims`
        // with a paragraph of extended help emitted `long_help = "shims"` — the shadow dropped
        // the real text, and its regenerated spec said the flag's own name instead.
        // The short help has to be *multi-line* to reach this path at all: a one-line help is
        // written as a doc comment, and `declared_help` — where the bug lived — is only called
        // for help a comment cannot carry. Without that, the mutation survived.
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--shims\" help=\"Use shims\\nacross tools\" \\
                long_help=\"Use shims\\nacross tools\\n\\nAnd here is why.\"\n",
        );
        assert!(
            !out.contains(r#"long_help = "shims""#),
            "the flag's name became its long help: {out}"
        );
        assert!(out.contains("And here is why."), "{out}");
    }

    #[test]
    fn a_required_collecting_flag_stays_required() {
        // A scalar flag says "required" by type — `String` rather than `Option<String>`. A
        // collecting one is a `Vec` either way, so it says it with `required`; without that the
        // shadow accepted none of a flag the source spec demanded at least one of, and its
        // regenerated spec described `[VALUE]…` where the original said `…`.
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--tag <TAG>\" required=#true var=#true help=\"Tags\"\n",
        );
        assert!(out.contains("required"), "{out}");
        assert!(out.contains("::std::vec::Vec<"), "it collects: {out}");

        // A collecting flag that is *not* required must not gain it.
        let (out, _) =
            rendered("name \"ex\"\nbin \"ex\"\nflag \"--tag <TAG>\" var=#true help=\"Tags\"\n");
        assert!(!out.contains("required"), "{out}");
    }

    #[test]
    fn a_required_subcommand_is_not_an_option() {
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\ncmd \"outer\" subcommand_required=#true {\n  cmd \"inner\"\n}\n",
        );
        assert!(
            out.contains("pub command: OuterCommands,"),
            "a required subcommand should not be an Option: {out}"
        );

        let (out, _) = rendered("name \"ex\"\nbin \"ex\"\ncmd \"outer\" {\n  cmd \"inner\"\n}\n");
        assert!(
            out.contains("pub command: ::std::option::Option<OuterCommands>,"),
            "an optional subcommand should be: {out}"
        );
    }

    #[test]
    fn long_help_does_not_repeat_the_short_form() {
        // A spec's long help opens with its short help, and the derive reads a doc
        // comment the same way — first paragraph short, whole comment long. Writing both
        // in full said the first paragraph twice, on nearly every command mise has.
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" help=\"How many\" \\
                long_help=\"How many\\n\\nAnd why it matters.\"\n",
        );
        assert_eq!(
            out.matches("/// How many").count(),
            1,
            "the short form should appear once: {out}"
        );
        assert!(out.contains("/// And why it matters."), "{out}");
    }

    #[test]
    fn stripping_the_short_form_leaves_the_rest_intact() {
        // Taking the remainder with a blunt `trim` corrupted two shapes that mise's help
        // really has: a long form whose first sentence ends in a period the short form
        // leaves off, which left the period stranded on a line of its own, and an
        // indented example, which lost its indentation.
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--security\" help=\"Include security info\" \\
                long_help=\"Include security info.\\n\\nRequires --json.\"\n",
        );
        assert!(!out.contains("/// ."), "a stranded period: {out}");
        assert!(out.contains("/// Requires --json."), "{out}");

        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--shims\" help=\"Use shims\" \\
                long_help=\"Use shims\\n\\n    PATH=/x\\n\\nAnd so on.\"\n",
        );
        assert!(
            out.contains("///     PATH=/x"),
            "an example should keep its indentation: {out}"
        );
    }

    #[test]
    fn a_long_form_that_only_adds_a_period_says_nothing() {
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--force\" help=\"Do it\" long_help=\"Do it.\"\n",
        );
        assert_eq!(out.matches("/// Do it").count(), 1, "{out}");
    }

    #[test]
    fn a_flag_carries_its_own_bounds() {
        // The spec can bound a repeatable flag on the flag or on its argument, and
        // usage-lib enforces both. Reading only the argument's dropped occurrence limits
        // silently — mise declares none, so nothing generated would have shown it.
        let (out, _) = rendered(
            "name \"ex\"\nbin \"ex\"\nflag \"--include <pattern>\" var=#true var_min=1 var_max=5\n",
        );
        assert!(out.contains("var_min = 1"), "{out}");
        assert!(out.contains("var_max = 5"), "{out}");
    }

    #[test]
    fn both_dialects_say_the_same_things_about_a_flag() {
        // Not the same words — the point of two dialects — but the same claims, since a
        // comparison between the two shadows is only fair if they describe one CLI.
        let spec = "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\" env=\"EX_JOBS\" global=#true {\n  choices \"1\" \"2\"\n}\n";
        let (usage, _) = rendered_as(spec, Dialect::Usage);
        let (clap, _) = rendered_as(spec, Dialect::Clap);

        assert!(usage.contains(r#"long = "jobs""#), "{usage}");
        assert!(usage.contains("global"), "{usage}");
        assert!(usage.contains(r#"env = "EX_JOBS""#), "{usage}");
        assert!(usage.contains(r#"choices("1", "2")"#), "{usage}");

        assert!(clap.contains(r#"long = "jobs""#), "{clap}");
        assert!(clap.contains("global = true"), "{clap}");
        assert!(clap.contains(r#"env = "EX_JOBS""#), "{clap}");
        assert!(
            clap.contains(r#"PossibleValuesParser::new(["1", "2"])"#),
            "{clap}"
        );
        // clap's derive reads the type as written, so the clap shadow cannot use
        // absolute paths where the usage shadow does.
        assert!(clap.contains("pub jobs: Option<String>"), "{clap}");
        assert!(
            usage.contains("pub jobs: ::std::option::Option<::std::string::String>"),
            "{usage}"
        );
    }

    /// The three command-level properties, which only one of the two dialects can say.
    const COMMAND_PROPERTIES: &str = "name \"ex\"\nbin \"ex\"\ndefault_subcommand \"go\"\n\
         cmd \"go\" restart_token=\":::\" {\n  mount run=\"ex tasks --usage\"\n}\n";

    #[test]
    fn the_usage_dialect_carries_the_command_properties() {
        let (out, skipped) = rendered_as(COMMAND_PROPERTIES, Dialect::Usage);
        assert!(out.contains(r#"default_subcommand = "go""#), "{out}");
        assert!(out.contains(r#"restart_token = ":::""#), "{out}");
        assert!(out.contains(r#"mount = "ex tasks --usage""#), "{out}");

        // Carried means not lost: none of the three may appear in the report.
        for what in [
            "`default_subcommand` on a command",
            "a `restart_token` on a command",
            "a `mount` on a command",
        ] {
            assert!(
                !skipped.counts.contains_key(what),
                "{what} is expressible now, so it should not be counted as dropped"
            );
        }
    }

    #[test]
    fn the_clap_dialect_counts_them_as_dropped() {
        // clap has no way to say any of the three. The shadow is a fairness fixture, so
        // what it cannot carry has to be named — a silent drop would make the clap side
        // look like a faithful translation of a spec it cannot represent.
        let (out, skipped) = rendered_as(COMMAND_PROPERTIES, Dialect::Clap);
        for what in [
            "`default_subcommand` on a command",
            "a `restart_token` on a command",
            "a `mount` on a command",
        ] {
            assert_eq!(skipped.counts.get(what), Some(&1), "{what}");
        }
        assert!(!out.contains("restart_token"), "{out}");
        assert!(!out.contains("default_subcommand"), "{out}");
    }

    #[test]
    fn only_the_root_declares_a_default_subcommand() {
        // `default_subcommand` is a property of the spec, not of every command in it. The
        // recursion passes one context down, so a child reading the root's value would be
        // an easy mistake to make and an invisible one to have made.
        let (out, _) = rendered_as(COMMAND_PROPERTIES, Dialect::Usage);
        assert_eq!(
            out.matches("default_subcommand").count(),
            1,
            "only the root should declare it: {out}"
        );
    }

    #[test]
    fn an_expressible_property_is_not_counted() {
        let (_, skipped) = rendered("name \"ex\"\nbin \"ex\"\ncmd \"go\" {\n  alias \"g\"\n}\n");
        // The alias is expressible, so it is carried rather than counted.
        assert!(!skipped.counts.contains_key("a command alias"));
    }
}
