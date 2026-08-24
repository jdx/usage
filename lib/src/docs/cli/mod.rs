use crate::{Spec, SpecCommand};
use std::sync::LazyLock;
use tera::Tera;

pub fn render_help(spec: &Spec, cmd: &SpecCommand, long: bool) -> String {
    // Convert to docs models to get layout calculations
    let docs_spec = crate::docs::models::Spec::from(spec);
    let mut docs_cmd = crate::docs::models::SpecCommand::from(&without_hidden(cmd, long));

    let mut ctx = tera::Context::new();
    ctx.insert("spec", &docs_spec);
    ctx.insert("long", &long);
    // Which page this is. The banner and the program's own description belong to the
    // program's page; a subcommand's page describes the subcommand, which is the question
    // that was asked. `full_cmd` is the path a user would type, so the root's is empty.
    ctx.insert("root", &docs_cmd.full_cmd.is_empty());
    // Keep this out of the recursively serialized docs command. It controls this page only;
    // carrying it on every descendant makes rendering a whole command tree pay for the same
    // boolean at every level.
    ctx.insert("show_help_subcommand", &!cmd.disable_help_subcommand);
    // Everything this command inherits: from each ancestor, only what it declared `global` —
    // the rule the parser follows on the way down. `full_cmd` is the typed path, so walking it
    // from the root gives the exact ancestry with none of the ambiguity a search would have.
    //
    // Listed nowhere before this: `communique generate` accepts `--config` from its root and
    // its page mentioned none of it — a flag a user can type and cannot discover.
    let (mut inherited, ancestors_taken) = inherited_flags(spec, cmd, &docs_cmd.full_cmd, long);

    // One column over both lists, so the two sections read as one table with a rule through it
    // rather than two that happen to be adjacent. The width feeds the wrapping as well as the
    // padding — a continuation line is indented to sit under the description — so both lists
    // are laid out again once the width is known.
    // Last in the command's own section, which is where clap has them: they carry no
    // `help_heading`, so a CLI that groups its flags gets them at the end of the ungrouped
    // list rather than inside somebody's section.
    {
        let supplied = supplied_flags(spec, cmd, &ancestors_taken, docs_cmd.full_cmd.is_empty());
        if !supplied.is_empty() {
            match docs_cmd
                .flag_groups
                .iter_mut()
                .find(|g| g.heading.is_none())
            {
                Some(group) => group.items.extend(supplied),
                // Inserted first, not pushed: `group_by_heading` sorts the unheaded group to
                // the front and argv's `groups_section` emits it there, so a CLI that heads
                // every one of its flags would otherwise get `Flags:` *after* the headed
                // sections here and before them there.
                None => docs_cmd.flag_groups.insert(
                    0,
                    crate::docs::models::Group {
                        heading: None,
                        items: supplied,
                    },
                ),
            }
        }
    }

    let width = crate::docs::layout::help_width(cmd.term_width, cmd.max_term_width);
    let col = crate::docs::layout::max_usage_width(
        docs_cmd
            .flag_groups
            .iter()
            .flat_map(|g| g.items.iter())
            .chain(inherited.iter())
            .map(|f| f.display_usage.as_str()),
    );
    for group in &mut docs_cmd.flag_groups {
        lay_out(&mut group.items, width, col);
    }
    lay_out(&mut inherited, width, col);

    let arg_has_ungrouped = docs_cmd
        .arg_groups
        .iter()
        .any(|group| group.heading.is_none());
    let arg_has_grouped = docs_cmd
        .arg_groups
        .iter()
        .any(|group| group.heading.is_some());
    let flag_has_ungrouped = docs_cmd
        .flag_groups
        .iter()
        .any(|group| group.heading.is_none());
    let flag_has_grouped = docs_cmd
        .flag_groups
        .iter()
        .any(|group| group.heading.is_some());
    ctx.insert("arg_has_ungrouped", &arg_has_ungrouped);
    ctx.insert("arg_has_grouped", &arg_has_grouped);
    ctx.insert("flag_has_ungrouped", &flag_has_ungrouped);
    ctx.insert("flag_has_grouped", &flag_has_grouped);

    // Inserted after the layout, not before: the template reads the widths, and a `cmd` put
    // into the context first would carry the ones computed before the two lists were joined.
    ctx.insert("cmd", &docs_cmd);
    ctx.insert("global_flags", &inherited);
    for (name, mark) in MARKS {
        ctx.insert(name, &mark);
    }
    ctx.insert("mark_grouped_args", &MARK_GROUPED_ARGS);
    ctx.insert("mark_grouped_flags", &MARK_GROUPED_FLAGS);
    ctx.insert("mark_global_flags", &MARK_GLOBAL_FLAGS);
    let template = if long {
        "spec_template_long.tera"
    } else {
        "spec_template_short.tera"
    };
    let rendered = TERA.render(template, &ctx).unwrap();
    let sections = Sections::split(&rendered);
    let page = match spec
        .help_template
        .as_deref()
        .filter(|t| crate::help_template::is_set(t))
    {
        Some(template) => crate::help_template::substitute(template, |name| sections.named(name)),
        None => sections.concatenated(),
    };
    page.trim().to_string() + "\n"
}

/// Where each section of a rendered page starts, as the templates write it.
///
/// The layout lives in the templates, and this is how it stays there: each one emits a marker
/// at every section boundary, so the boundaries are declared beside the sections rather than
/// worked out again here. A page with no `help_template` is the marks taken back out, which is
/// the same string the templates produced before any of this existed — and what the fleet gate
/// compares byte for byte.
///
/// Control characters, because a marker has to be something no help text contains and no
/// terminal shows if one ever escapes.
const MARKS: [(&str, &str); 6] = [
    ("mark_usage", "\u{1}usage\u{1}"),
    ("mark_commands", "\u{1}commands\u{1}"),
    ("mark_args", "\u{1}args\u{1}"),
    ("mark_flags", "\u{1}flags\u{1}"),
    ("mark_flattened", "\u{1}flattened\u{1}"),
    ("mark_after_help", "\u{1}after_help\u{1}"),
];
const MARK_GROUPED_ARGS: &str = "\u{1}grouped_args\u{1}";
const MARK_GROUPED_FLAGS: &str = "\u{1}grouped_flags\u{1}";
const MARK_GLOBAL_FLAGS: &str = "\u{1}global_flags\u{1}";

/// A rendered page cut into the sections a `help_template` may reorder.
///
/// The twin of `usage_argv::help`'s `Sections`, down to `flattened` not being a section an
/// author can name: it is the other half of `commands`, since `flatten_help` replaces a
/// command list with the subcommands' own bodies, and only one of the two is ever there.
struct Sections<'a> {
    about: &'a str,
    usage: &'a str,
    commands: &'a str,
    args: String,
    flags: String,
    grouped_args: &'a str,
    ungrouped_args: &'a str,
    grouped_flags: &'a str,
    ungrouped_flags: String,
    flattened: &'a str,
    after_help: &'a str,
}

impl<'a> Sections<'a> {
    fn split(rendered: &'a str) -> Self {
        let mut rest = rendered;
        let mut parts: Vec<&str> = Vec::with_capacity(MARKS.len() + 1);
        for (_, mark) in MARKS {
            // A missing marker leaves that section empty rather than swallowing the ones after
            // it: every one is written at the top level of both templates, so this cannot
            // happen, and it is not worth a panic in a help renderer if it ever does.
            match rest.split_once(mark) {
                Some((before, after)) => {
                    parts.push(before);
                    rest = after;
                }
                None => parts.push(""),
            }
        }
        parts.push(rest);
        let (ungrouped_args, grouped_args) = parts[3]
            .split_once(MARK_GROUPED_ARGS)
            .unwrap_or((parts[3], ""));
        let (own_flags, global_flags) = parts[4]
            .split_once(MARK_GLOBAL_FLAGS)
            .unwrap_or((parts[4], ""));
        let (own_ungrouped_flags, grouped_flags) = own_flags
            .split_once(MARK_GROUPED_FLAGS)
            .unwrap_or((own_flags, ""));
        Self {
            about: parts[0],
            usage: parts[1],
            commands: parts[2],
            args: format!("{ungrouped_args}{grouped_args}"),
            flags: format!("{own_ungrouped_flags}{grouped_flags}{global_flags}"),
            grouped_args,
            ungrouped_args,
            grouped_flags,
            ungrouped_flags: format!("{own_ungrouped_flags}{global_flags}"),
            flattened: parts[5],
            after_help: parts[6],
        }
    }

    /// The default page: every section in the order the templates wrote them.
    fn concatenated(&self) -> String {
        [
            self.about,
            self.usage,
            self.commands,
            self.args.as_str(),
            self.flags.as_str(),
            self.flattened,
            self.after_help,
        ]
        .concat()
    }

    /// One section by name, trimmed, so that a template owns the whitespace between them.
    fn named(&self, name: &str) -> Option<String> {
        Some(match name {
            "about" => self.about.trim().to_string(),
            "usage" => self.usage.trim().to_string(),
            "commands" => {
                let mut out = self.commands.trim().to_string();
                let flattened = self.flattened.trim();
                if !flattened.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(flattened);
                }
                out
            }
            "args" => self.args.trim().to_string(),
            "flags" => self.flags.trim().to_string(),
            "grouped_args" => self.grouped_args.trim().to_string(),
            "ungrouped_args" => self.ungrouped_args.trim().to_string(),
            "grouped_flags" => self.grouped_flags.trim().to_string(),
            "ungrouped_flags" => self.ungrouped_flags.trim().to_string(),
            "after_help" => self.after_help.trim().to_string(),
            _ => return None,
        })
    }
}

/// The entries for `--help` and `--version`, which the parser supplies and no spec declares.
///
/// Listed because help is written for people: a reader looking for how to ask for help should
/// find it on the page. This reverses the rule these two used to follow — that a page lists
/// exactly what its spec declares — and the reason is that the spec has its own readers, and
/// they are not the ones reading this.
///
/// `--version` only on the program's own page and only where a version is declared, which is
/// where a parser accepts one. Each spelling is dropped where the CLI claimed it, since a page
/// must not describe a flag that something else binds.
///
/// The twin of `supplied_entries` in `usage-argv`'s `help` module; the gate over mise's spec is
/// what says the two agree.
fn supplied_flags(
    spec: &Spec,
    cmd: &SpecCommand,
    ancestors_taken: &[String],
    is_root: bool,
) -> Vec<crate::docs::models::SpecFlag> {
    // The command's own spellings plus everything in scope above it — the set the inherited
    // walk built, which counts hidden globals and negations. Rebuilding it from the *visible*
    // inherited list lost both: a hidden ancestor that binds `--help` would have had the page
    // offer it anyway.
    let mut taken: Vec<String> = ancestors_taken.to_vec();
    for f in &cmd.flags {
        taken.extend(f.long.iter().map(|l| format!("--{l}")));
        taken.extend(f.short.iter().map(|s| format!("-{s}")));
        // Stored with its dashes here, unlike in usage-argv.
        taken.extend(f.negate.clone());
    }

    let build = |name: &str, long: &str, short: char, help: &str| {
        let long_free = !taken.contains(&format!("--{long}"));
        let short_free = !taken.contains(&format!("-{short}"));
        if !long_free && !short_free {
            return None;
        }
        // Named after the form it shows: a short-only entry called `help` reads as a renamed
        // flag and printed `help: -h`.
        let name = if long_free { name } else { &short.to_string() };
        let mut flag = crate::SpecFlag {
            name: name.to_string(),
            long: if long_free {
                vec![long.to_string()]
            } else {
                vec![]
            },
            short: if short_free { vec![short] } else { vec![] },
            help: Some(help.to_string()),
            ..Default::default()
        };
        flag.usage = flag.usage();
        Some(crate::docs::models::SpecFlag::from(&flag))
    };

    let mut out = Vec::new();
    // `disable_help` turns the parser's answer off — `is_help_arg` refuses the spelling
    // outright — so a page that still listed it would describe an action nothing performs.
    // The same rule as a claimed or hidden spelling, with the claim made by the spec itself.
    //
    // usage-argv has no equivalent: `disable_help` is a KDL-only word, so no spec that crate
    // can hold ever carries one, and the two renderers cannot disagree about it.
    if spec.disable_help != Some(true) && !cmd.disable_help_flag {
        out.extend(build("help", "help", 'h', "Print help"));
    }
    if is_root
        && (spec.version.is_some() || spec.long_version.is_some())
        && !cmd.disable_version_flag
    {
        out.extend(build("version", "version", 'V', "Print version"));
    }
    out
}

/// Fit a list of flags to a column: how wide their names are, and where their help wraps.
///
/// The same pass `SpecCommand::from` makes, run again once the width is known over *both* the
/// command's own flags and the ones it inherits. The width is not only padding — a wrapped
/// description is indented to sit under itself — so it cannot be decided per section and then
/// shared.
fn lay_out(flags: &mut [crate::docs::models::SpecFlag], terminal_width: usize, col: usize) {
    for flag in flags {
        flag.usage_col_width = col;
        flag.help_rendered = None;
        flag.help_is_multiline = false;
        let help = flag.help_long.as_deref().or(flag.help.as_deref());
        if let Some(help) = help {
            let (rendered, is_multiline) =
                crate::docs::layout::render_help_text(help, terminal_width, col);
            // An empty rendering is how this says "use the block layout instead".
            if !rendered.is_empty() {
                flag.help_rendered = Some(rendered);
                flag.help_is_multiline = is_multiline;
            }
        }
    }
}

/// The flags a command inherits, as its page should list them.
///
/// Walked down `full_cmd` from the root, which is the path a user would type — so the chain is
/// exact. Each ancestor contributes only what it declared `global`, and hidden ones are left
/// out here as they are everywhere else.
///
/// The twin of `own_and_global` in `usage-argv`'s `help` module; the two must agree, and the
/// gate over mise's spec is what says they do.
fn inherited_flags(
    spec: &Spec,
    cmd: &SpecCommand,
    full_cmd: &[String],
    long_help: bool,
) -> (Vec<crate::docs::models::SpecFlag>, Vec<String>) {
    // Every ancestor, root first, which is the order a reader meets them walking down.
    let mut ancestors: Vec<&SpecCommand> = Vec::new();
    let mut at = &spec.cmd;
    for name in full_cmd.iter().take(full_cmd.len().saturating_sub(1)) {
        ancestors.push(at);
        let Some(next) = at.subcommands.get(name) else {
            return (Vec::new(), Vec::new());
        };
        at = next;
    }
    if !full_cmd.is_empty() {
        ancestors.push(at);
    }

    // Shadowing, which the parser does and the page has to agree with: a command's own flags
    // are looked up before its ancestors', so `mise use --raw` is *use's* and never the root's.
    // Listing both would print two descriptions for one spelling, one of which can never apply.
    // Nearest ancestor first for the decision, then emitted root-first.
    // Two sets, because the parser has two passes: it resolves a word against every long and
    // short in scope before it looks at a negation at all, so *any* long beats *any* negation
    // however far away it is. Reading them as one said a nearer negation had taken a spelling
    // that a farther long actually wins.
    //
    // usage-lib stores a negation *with* its dashes — `negate="--no-colour"` reaches the model
    // as `--no-colour` — where usage-argv stores it without. Prefixing here produced
    // `----no-colour`, which matched nothing, so negations were counted in name only.
    let forms = |f: &crate::SpecFlag| -> Vec<String> {
        f.long
            .iter()
            .map(|l| format!("--{l}"))
            .chain(f.short.iter().map(|s| format!("-{s}")))
            .collect()
    };
    let every_form: Vec<String> = cmd
        .flags
        .iter()
        .chain(
            ancestors
                .iter()
                .flat_map(|a| a.flags.iter())
                .filter(|f| f.global),
        )
        .flat_map(&forms)
        .collect();

    let mut taken: Vec<String> = cmd.flags.iter().flat_map(&forms).collect();
    let mut taken_negations: Vec<String> =
        cmd.flags.iter().filter_map(|f| f.negate.clone()).collect();
    let mut keep: Vec<(&crate::SpecFlag, Option<String>, Option<char>, bool)> = Vec::new();
    for ancestor in ancestors.iter().rev() {
        for f in ancestor.flags.iter().filter(|f| f.global) {
            let long = f
                .long
                .iter()
                .find(|l| !f.hidden_aliases.contains(l) && !taken.contains(&format!("--{l}")))
                .cloned();
            let short = f
                .short
                .iter()
                .find(|s| !f.hidden_short_aliases.contains(s) && !taken.contains(&format!("-{s}")))
                .copied();
            let mine = forms(f);
            let negate = f.negate.as_ref().is_some_and(|n| {
                !taken_negations.contains(n) && (!every_form.contains(n) || mine.contains(n))
            });
            // Reserved whether or not it is shown: a hidden one still binds, and so does one
            // whose every spelling something nearer already took.
            taken.extend(forms(f));
            taken_negations.extend(f.negate.clone());
            if f.hide
                || if long_help {
                    f.hide_long_help
                } else {
                    f.hide_short_help
                }
                || (long.is_none() && short.is_none() && !negate)
            {
                continue;
            }
            keep.push((f, long, short, negate));
        }
    }
    let shown: Vec<crate::docs::models::SpecFlag> = ancestors
        .iter()
        .flat_map(|a| a.flags.iter())
        .filter_map(|f| {
            keep.iter()
                .find(|(k, _, _, _)| std::ptr::eq(*k, f))
                .map(|(_, l, s, n)| (f, l.clone(), *s, *n))
        })
        .map(|(f, long, short, negate)| {
            // Only the spellings that survived, so the entry offers what the parser would
            // actually accept here.
            let mut shown = f.clone();
            shown.long = long.into_iter().collect();
            shown.short = short.into_iter().collect();
            if !negate {
                shown.negate = None;
            }
            shown.usage = shown.usage();
            crate::docs::models::SpecFlag::from(&shown)
        })
        .collect();
    // The claim set travels with the result, forms and negations together: the supplied
    // `--help` and `--version` entries lose to both, since `find_negation` runs before either
    // is offered — even though a negation loses to a long.
    taken.extend(taken_negations);
    (shown, taken)
}

/// The command without anything marked `hide`.
///
/// Help showed hidden flags, hidden arguments and hidden subcommands — everything `hide`
/// exists to keep out of it. The usage *line* filtered them already, through
/// `SpecCommand::usage`, so `ex --help` listed a `--secret` that the line above it did not
/// mention. Markdown and manpage rendering filter too; the help templates were the one place
/// that did not.
///
/// Filtered here rather than in the templates, and before the docs model builds its groups, so
/// that a heading whose every entry is hidden produces no section — the same rule markdown
/// already follows.
fn without_hidden(cmd: &SpecCommand, long: bool) -> SpecCommand {
    let mut visible = cmd.clone();
    visible.flags.retain(|flag| {
        !flag.hide
            && if long {
                !flag.hide_long_help
            } else {
                !flag.hide_short_help
            }
    });
    visible.args.retain(|arg| {
        !arg.hide
            && if long {
                !arg.hide_long_help
            } else {
                !arg.hide_short_help
            }
    });
    visible.subcommands.retain(|_, sub| !sub.hide);
    // Ordinary help only lists immediate subcommands, so their fields are never
    // rendered on this page. Walking and cloning the whole remaining tree here
    // makes rendering every page quadratic on a fleet-sized CLI. Flattened help
    // is the one mode that renders descendant fields and therefore needs the
    // recursive filtering.
    if visible.flatten_help {
        for sub in visible.subcommands.values_mut() {
            *sub = without_hidden(sub, long);
        }
    }
    visible
}

static TERA: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = Tera::default();

    // Register ljust filter for left-justifying text with padding
    tera.register_filter(
        "ljust",
        |value: &tera::Value, args: tera::Kwargs, _: &tera::State| -> tera::TeraResult<String> {
            let value = value.as_str().unwrap_or("");
            let width = args.get::<u64>("width")?.unwrap_or(0) as usize;
            Ok(format!("{:<width$}", value, width = width))
        },
    );
    tera.register_filter(
        "default",
        |value: &tera::Value,
         kwargs: tera::Kwargs,
         _: &tera::State|
         -> tera::TeraResult<tera::Value> {
            let default_val = kwargs.must_get::<tera::Value>("value")?;
            let boolean = kwargs.get::<bool>("boolean")?.unwrap_or_default();
            if value.is_undefined() || value.is_none() || (boolean && !value.is_truthy()) {
                Ok(default_val)
            } else {
                Ok(value.clone())
            }
        },
    );

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("spec_template_short.tera", include_str!("templates/spec_template_short.tera")),
        ("spec_template_long.tera", include_str!("templates/spec_template_long.tera")),
    ]).unwrap();

    tera
});

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn flag_aliases_do_not_leak_into_interactive_help() {
        let spec = crate::spec! { r#"
bin "ex"
flag "-t -f --tail --follow" help="Follow output"
        "# }
        .unwrap();

        for long in [false, true] {
            let page = super::render_help(&spec, &spec.cmd, long);
            assert!(page.contains("-t, --tail"), "long={long}:\n{page}");
            assert!(!page.contains("aliases:"), "long={long}:\n{page}");
        }
    }

    #[test]
    fn a_hidden_ancestor_claim_keeps_help_off_the_page() {
        // `--help` is supplied by the parser, and a hidden global that declares it still binds
        // first — `hide` keeps a flag off the page, not out of the parse. Deciding the supplied
        // entries from the *visible* inherited list lost exactly that, and the page offered a
        // `--help` that does something else.
        let spec = crate::spec! { r#"
bin "ex"
flag "--help" global=#true hide=#true help="the CLI's own, and invisible"
cmd inner help="a command" {
    flag "--plain" help="its own"
}
        "# }
        .unwrap();

        let inner = spec.cmd.subcommands.get("inner").expect("inner");
        for long in [false, true] {
            let page = super::render_help(&spec, inner, long);
            assert!(
                !page.contains("--help"),
                "long={long}: a hidden ancestor binds this:\n{page}"
            );
            // The short form is untouched, since nothing claimed it.
            assert!(page.contains("-h"), "long={long}:\n{page}");
        }
    }

    #[test]
    fn a_long_beats_a_negation_however_far_away_it_is() {
        // A negation is stored *with* its dashes here and without them in usage-argv, so the
        // spelling was being looked up as `----no-cache` and matched nothing — negations were
        // counted in name only. And which one binds is not about distance: a word is resolved
        // against every long in scope before any negation is considered, so the root's plain
        // `--no-cache` wins over the subcommand's negation and belongs on its page.
        let spec = crate::spec! { r#"
bin "ex"
flag "--no-cache" global=#true help="the root's plain long"
flag "--colour" negate="--no-colour" global=#true help="the root's, with a negation"
cmd narrow help="a command" {
    flag "--cache" negate="--no-cache" help="its own, with a negation"
    flag "--tint" negate="--no-colour" help="claims the root's negation"
}
        "# }
        .unwrap();

        let narrow = spec.cmd.subcommands.get("narrow").expect("narrow");
        for long in [false, true] {
            let page = super::render_help(&spec, narrow, long);
            assert!(
                page.contains("--no-cache"),
                "long={long}: a long beats a negation, so this still binds here:\n{page}"
            );
            // And a negation *is* claimed by a nearer negation — which is what the dashes
            // matter for. `--colour` stays; the negation it used to carry does not.
            assert!(page.contains("--colour"), "long={long}:\n{page}");
            let global = page
                .split_once("Global flags:")
                .expect("a global section")
                .1;
            assert!(
                !global.contains("--colour / --no-colour"),
                "long={long}: the nearer negation owns that spelling:\n{page}"
            );
        }
    }

    #[test]
    fn a_description_of_only_spaces_is_no_description() {
        // `usage-argv` filters a blank description wherever it reads one, and this template
        // asked only whether the string was there — so `help="   "` bought a column of padding
        // and a line of trailing spaces here and nothing there. Two renderings of one spec.
        //
        // Asserted on the trailing whitespace rather than by comparing the two renderers, so
        // the test says what is wrong with the line rather than only that they disagree.
        let spec = crate::spec! { r#"
bin "ex"
flag "--blank" help="   "
flag "--plain" help="plain"
        "# }
        .unwrap();

        for long in [false, true] {
            let page = super::render_help(&spec, &spec.cmd, long);
            // In the flags section, not the usage line — `Usage: ex [--blank] [--plain]`
            // also contains the name and has no padding to get wrong.
            let listing = page.split_once("\nFlags:").expect("a flags section").1;
            let line = listing
                .lines()
                .find(|l| l.contains("--blank"))
                .unwrap_or_else(|| panic!("long={long}: {page}"));
            assert_eq!(
                line,
                line.trim_end(),
                "long={long}: trailing space on {line:?}"
            );
        }
    }

    #[test]
    fn test_render_help_omits_hidden_entries() {
        let spec = crate::spec! { r#"
bin "ex"
flag "--visible" help="shown"
flag "--secret" hide=#true help="hidden"
flag "--filtered" hide=#true help="hidden" help_heading="Filtering"
arg "[SHOWN]" help="an arg"
arg "[HIDDEN]" hide=#true help="a hidden arg"
cmd open help="a command"
cmd sneaky hide=#true help="a hidden command"
        "# }
        .unwrap();

        // `hide` keeps something out of help. The usage line filtered already — through
        // `SpecCommand::usage` — so before this, `ex --help` listed a `--secret` the line
        // above it did not mention. A heading whose every entry is hidden produces no
        // section, which is the rule markdown rendering already followed.
        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: ex [--visible] [SHOWN] <SUBCOMMAND>

        Commands:
          open  a command
          help  Print this message or the help of the given subcommand(s)

        Arguments:
          [SHOWN]  an arg

        Flags:
              --visible  shown
          -h, --help     Print help
        ");
    }

    #[test]
    fn test_render_help_groups_by_heading() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--verbose" help="Verbose output"
flag "--filter <pattern>" help="Only matching" help_heading="Filtering"
flag "--exclude <pattern>" help="Skip matching" help_heading="Filtering"
flag "--jobs <n>" help="How many at once" help_heading="Performance"
arg "<file>" help="The file"
arg "<mode>" help="How to run" help_heading="Behaviour"
        "# }
        .unwrap();

        // Unheaded entries keep the default title and come first; each heading
        // then gets its own section, in the order the headings first appear.
        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [FLAGS] <file> <mode>

        Arguments:
          <file>  The file

        Behaviour:
          <mode>  How to run

        Flags:
              --verbose            Verbose output
          -h, --help               Print help

        Filtering:
              --filter <pattern>   Only matching
              --exclude <pattern>  Skip matching

        Performance:
              --jobs <n>           How many at once
        ");
    }

    #[test]
    fn test_render_help_with_only_headed_flags() {
        // No default section when nothing lands in it: a CLI that gives every
        // flag a heading should not get an empty "Flags:".
        let spec = crate::spec! { r#"
bin "testcli"
flag "--filter <pattern>" help="Only matching" help_heading="Filtering"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [--filter <pattern>]

        Flags:
          -h, --help              Print help

        Filtering:
              --filter <pattern>  Only matching
        ");
    }

    #[test]
    fn test_render_help_with_env() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--color" env="MYCLI_COLOR" help="Enable color output"
flag "--verbose" env="MYCLI_VERBOSE" help="Verbose output"
flag "--debug" help="Debug mode"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [FLAGS]

        Flags:
              --color    Enable color output [env: MYCLI_COLOR]
              --verbose  Verbose output [env: MYCLI_VERBOSE]
              --debug    Debug mode
          -h, --help     Print help
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [FLAGS]

        Flags:
              --color    Enable color output
            [env: MYCLI_COLOR]
              --verbose  Verbose output
            [env: MYCLI_VERBOSE]
              --debug    Debug mode
          -h, --help     Print help
        ");
    }

    #[test]
    fn test_render_help_with_arg_env() {
        let spec = crate::spec! { r#"
bin "testcli"
arg "<input>" env="MY_INPUT" help="Input file"
arg "<output>" env="MY_OUTPUT" help="Output file"
arg "<extra>" help="Extra arg without env"
arg "[default]" help="Arg with default value" default="default value"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli <ARGS>…

        Arguments:
          <input>    Input file [env: MY_INPUT]
          <output>   Output file [env: MY_OUTPUT]
          <extra>    Extra arg without env
          [default]  Arg with default value (default: default value)

        Flags:
          -h, --help  Print help
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli <ARGS>…

        Arguments:
          <input>    Input file
            [env: MY_INPUT]
          <output>   Output file
            [env: MY_OUTPUT]
          <extra>    Extra arg without env
          [default]  Arg with default value
            (default: default value)

        Flags:
          -h, --help  Print help
        ");
    }

    #[test]
    fn test_render_help_with_negated_flag() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--compress" negate="--no-compress" default=#true help="Compress output"
flag "--verbose" help="Verbose output"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [--compress] [--verbose]

        Flags:
              --compress / --no-compress  Compress output (default: true)
              --verbose                   Verbose output
          -h, --help                      Print help
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [--compress] [--verbose]

        Flags:
              --compress / --no-compress  Compress output
            (default: true)
              --verbose                   Verbose output
          -h, --help                      Print help
        ");
    }

    #[test]
    fn granular_help_hides_preserve_behavior_but_remove_presentation() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--mode <mode>" help="Select mode" env="MODE" default="fast" hide_default_value=#true hide_env=#true hide_possible_values=#true {
  choices {
    choice "fast"
    choice "slow"
  }
}
flag "--short-only" help="short" hide_long_help=#true
flag "--long-only" help="long" hide_short_help=#true
arg "[input]" help="Input" env="INPUT" default="file" hide_default_value=#true hide_env=#true
        "# }
        .unwrap();

        let short = render_help(&spec, &spec.cmd, false);
        assert!(short.contains("--mode <mode>"), "{short}");
        assert!(short.contains("--short-only"), "{short}");
        assert!(!short.contains("--long-only"), "{short}");
        assert!(
            !short.contains("MODE") && !short.contains("fast, slow"),
            "{short}"
        );
        assert!(
            !short.contains("default: fast") && !short.contains("default: file"),
            "{short}"
        );

        let long = render_help(&spec, &spec.cmd, true);
        assert!(long.contains("--long-only"), "{long}");
        assert!(!long.contains("--short-only"), "{long}");
        assert!(
            !long.contains("MODE") && !long.contains("possible values"),
            "{long}"
        );

        let rendered = spec.to_string();
        let reparsed: crate::Spec = rendered.parse().unwrap();
        assert!(reparsed.cmd.flags[0].hide_default_value);
        assert!(reparsed.cmd.flags[0].hide_env);
        assert!(reparsed.cmd.flags[0].hide_possible_values);
    }

    #[test]
    fn a_help_template_reorders_omits_and_wraps_the_sections() {
        // The whole of what a template can do: `{{flags}}` before `{{args}}`, no
        // `{{commands}}` at all, and text of the author's own around them. Nothing else is
        // substituted, so the layout is the spec's and the sections' contents are not.
        let spec = crate::spec! { r#"
bin "ex"
about "An example"
help_template "{{about}}\n\n{{usage}}\n\n{{flags}}\n\n{{args}}\n\n-- ask a person --"
flag "--force" help="Do it anyway"
arg "<file>" help="Which file"
cmd "run" help="Run it"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        An example

        Usage: ex [--force] <file> <SUBCOMMAND>

        Flags:
              --force  Do it anyway
          -h, --help   Print help

        Arguments:
          <file>  Which file

        -- ask a person --
        ");
    }

    #[test]
    fn a_template_places_the_sections_a_page_actually_has() {
        // A template names every section, and this command has no arguments — the gap
        // `{{args}}` would leave closes up rather than pushing the commands down the page.
        // What lets one template serve a whole CLI, since most commands are missing most
        // sections. Here the version banner and description are last, and `after_help`
        // carries them nothing.
        let spec = crate::spec! { r#"
bin "ex"
version "1.2.3"
about "An example"
after_help "Read the docs."
help_template "{{usage}}\n\n{{flags}}\n\n{{args}}\n\n{{commands}}\n\n{{after_help}}\n\n{{about}}"
flag "--force" help="Do it anyway"
cmd "run" help="Run it"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: ex [--force] <SUBCOMMAND>

        Flags:
              --force    Do it anyway
          -h, --help     Print help
          -V, --version  Print version

        Commands:
          run
            Run it

          help
            Print this message or the help of the given subcommand(s)

        Read the docs.

        ex 1.2.3
        An example
        ");
    }

    #[test]
    fn a_flattened_page_puts_its_bodies_where_the_commands_would_go() {
        // `flatten_help` replaces a command list with the subcommands' own bodies, so a
        // template that places `{{commands}}` places whichever of the two this command has.
        let spec = crate::spec! { r#"
bin "ex"
flatten_help #true
help_template "{{usage}}\n\n{{commands}}\n\n{{flags}}"
cmd "run" help="Run it" {
    flag "--dry-run" help="Only show changes"
}
        "# }
        .unwrap();

        let page = render_help(&spec, &spec.cmd, false);
        assert!(
            page.find("run:").unwrap() < page.find("Flags:").unwrap(),
            "{page}"
        );
        assert!(page.contains("--dry-run"), "{page}");
    }

    #[test]
    fn test_render_help_with_before_after_help() {
        let spec = crate::spec! { r#"
bin "testcli"
before_help "This text appears before the help"
after_help "This text appears after the help"
flag "--verbose" help="Enable verbose output"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        This text appears before the help

        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help

        This text appears after the help
        ");
    }

    #[test]
    fn test_render_help_with_before_after_help_long() {
        let spec = crate::spec! { r#"
bin "testcli"
before_help "short before"
before_help_long "This is the long version of before help"
after_help "short after"
after_help_long "This is the long version of after help"
flag "--verbose" help="Enable verbose output"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        short before

        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help

        short after
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        This is the long version of before help

        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help

        This is the long version of after help
        ");
    }

    #[test]
    fn test_render_help_with_examples() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--verbose" help="Enable verbose output"
example "testcli --verbose" header="Run with verbose output"
example "testcli" header="Run normally" help="Just runs the tool"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help

        Examples:
          Run with verbose output:
            $ testcli --verbose
          Run normally:
            $ testcli
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help

        Examples:
          Run with verbose output:
            $ testcli --verbose
          Run normally:
            Just runs the tool
            $ testcli
        ");
    }

    #[test]
    fn test_render_help_with_version() {
        let spec = crate::spec! { r#"
bin "testcli"
name "TestCLI"
version "1.2.3"
flag "--verbose" help="Enable verbose output"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        TestCLI 1.2.3
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help
          -V, --version  Print version
        ");
    }

    #[test]
    fn test_render_help_with_only_long_version() {
        let spec = crate::spec! { r#"
bin "testcli"
long_version "1.2.3\ncommit abc123"
flag "--verbose" help="Enable verbose output"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help
          -V, --version  Print version
        ");
    }

    #[test]
    fn test_render_help_omits_help_when_disabled() {
        // `disable_help` turns the parser's answer off, so the page must not offer it: the same
        // rule as a spelling the CLI claimed, with the spec doing the claiming. `--version`
        // stays, because nothing disabled that.
        let spec = crate::spec! { r#"
bin "testcli"
version "1.2.3"
disable_help #true
flag "--verbose" help="Enable verbose output"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        testcli 1.2.3
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -V, --version  Print version
        ");
    }

    #[test]
    fn test_render_help_with_author_license() {
        let spec = crate::spec! { r#"
bin "testcli"
author "Test Author"
license "MIT"
flag "--verbose" help="Enable verbose output"
        "# }
        .unwrap();

        // Short help should not show author/license
        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help
        ");

        // Long help should show author/license at the bottom
        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output
          -h, --help     Print help

        Author: Test Author
        License: MIT
        ");
    }

    #[test]
    fn test_render_help_with_deprecated_command() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--old" help="Old switch" deprecated="use --new" deprecated_warn_at="6.1" deprecated_remove_at="7.0"
cmd "old-cmd" help="Do something" deprecated="use new-cmd instead" deprecated_warn_at="6.2" deprecated_remove_at="7.0"
cmd "new-cmd" help="Do something better"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @"
        Usage: testcli [--old] <SUBCOMMAND>

        Commands:
          new-cmd  Do something better
          old-cmd [deprecated: use new-cmd instead; warns at 6.2; removed at 7.0]  Do something
          help  Print this message or the help of the given subcommand(s)

        Flags:
              --old   Old switch [deprecated: use --new; warns at 6.1; removed at 7.0]
          -h, --help  Print help
        ");
    }

    #[test]
    fn deprecation_milestones_do_not_need_a_message() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--old" help="Old switch" deprecated_remove_at="7.0"
cmd "old-cmd" help="Do something" deprecated_warn_at="6.2"
        "# }
        .unwrap();

        let page = render_help(&spec, &spec.cmd, false);
        assert!(
            page.contains("old-cmd [deprecated: warns at 6.2]"),
            "{page}"
        );
        assert!(page.contains("[deprecated: removed at 7.0]"), "{page}");
        assert!(!page.contains("[deprecated:;"), "{page}");
    }

    #[test]
    fn test_render_help_with_subcommand_presentation() {
        let spec = crate::spec! { r#"
bin "testcli"
subcommand_help_heading "Actions"
subcommand_value_name "ACTION"
cmd "run" help="Run it\n"
        "# }
        .unwrap();

        let page = render_help(&spec, &spec.cmd, false);
        assert!(page.contains("Usage: testcli <ACTION>"), "{page}");
        assert!(page.contains("\nActions:\n"), "{page}");
    }

    #[test]
    fn test_render_help_honors_explicit_display_order() {
        let spec = crate::spec! { r#"
bin "testcli"
flag "--unset" help="Unordered"
flag "--later" help="Later" display_order=20
flag "--first" help="First" display_order=10
cmd "zulu" help="Unordered"
cmd "later" help="Later" display_order=20
cmd "first" help="First" display_order=10
cmd "alpha" help="Unordered"
        "# }
        .unwrap();

        let page = render_help(&spec, &spec.cmd, false);
        let commands = page.split_once("\nCommands:\n").unwrap().1;
        assert!(
            commands.find("first").unwrap() < commands.find("later").unwrap()
                && commands.find("later").unwrap() < commands.find("alpha").unwrap()
                && commands.find("alpha").unwrap() < commands.find("zulu").unwrap(),
            "{page}"
        );
        let flags = page.split_once("\nFlags:\n").unwrap().1;
        assert!(
            flags.find("--first").unwrap() < flags.find("--later").unwrap()
                && flags.find("--later").unwrap() < flags.find("--unset").unwrap(),
            "{page}"
        );
    }

    #[test]
    fn test_render_help_groups_subcommands_by_heading() {
        let spec = crate::spec! { r#"
bin "testcli"
cmd "run" help="Run it" help_heading="Core commands"
cmd "clean" help="Remove old state" help_heading="Maintenance"
cmd "status" help="Show status" help_heading="Commands"
        "# }
        .unwrap();

        for page in [
            render_help(&spec, &spec.cmd, false),
            render_help(&spec, &spec.cmd, true),
        ] {
            let commands = page.find("\nCommands:\n").expect("default command section");
            assert_eq!(page.matches("\nCommands:\n").count(), 1, "{page}");
            let core = page.find("\nCore commands:\n").expect("core section");
            let maintenance = page.find("\nMaintenance:\n").expect("maintenance section");
            assert!(commands < core && commands < maintenance, "{page}");
            let default_end = core.min(maintenance);
            assert!(page[commands..default_end].contains("status"), "{page}");
            assert!(page[commands..default_end].contains("help"), "{page}");
            assert!(page[core..].contains("run"), "{page}");
            assert!(page[maintenance..].contains("clean"), "{page}");
        }
    }

    #[test]
    fn test_render_help_with_next_line_layout() {
        let spec = crate::spec! { r#"
bin "testcli"
next_line_help #true
arg "<input>" help="Input file" env="INPUT" default="fast" {
    choices {
        choice "fast"
        choice "slow"
    }
}
flag "--verbose" help="Enable verbose output"
cmd "run" help="Run it"
        "# }
        .unwrap();

        let short = render_help(&spec, &spec.cmd, false);
        assert!(!short.contains("    Run it\n\n  help"), "{short}");
        for page in [short, render_help(&spec, &spec.cmd, true)] {
            assert!(page.contains("  [input]\n    Input file"), "{page}");
            assert!(
                page.contains("--verbose\n    Enable verbose output"),
                "{page}"
            );
            assert!(
                page.contains(
                    "    [possible values: fast, slow]\n    [env: INPUT]\n    (default: fast)"
                ),
                "{page}"
            );
            assert!(page.contains("  run\n    Run it"), "{page}");
        }
    }

    #[test]
    fn flatten_help_expands_subcommands_instead_of_listing_them() {
        let spec = crate::spec! { r#"
bin "testcli"
flatten_help #true
next_line_help #true
cmd "run" help="Run it" {
    arg "<task>" help="Task name" env="TASK" default="build" {
        choices {
            choice "build"
            choice "test"
        }
    }
    flag "--dry-run" help="Only show changes"
    flatten_help #true
    cmd "nested" help="Nested operation" {
        flag "--deep" help="Deep option"
    }
}
        "# }
        .unwrap();

        for page in [
            render_help(&spec, &spec.cmd, false),
            render_help(&spec, &spec.cmd, true),
        ] {
            assert!(
                page.contains("Usage: testcli\n       testcli run"),
                "{page}"
            );
            assert!(!page.contains("\nCommands:\n"), "{page}");
            assert!(page.contains("\nrun:\nRun it"), "{page}");
            assert!(page.contains("[task]"), "{page}");
            assert!(page.contains("--dry-run"), "{page}");
            assert!(page.contains("\nrun nested:\nNested operation"), "{page}");
            assert!(page.contains("--deep"), "{page}");
            assert!(
                page.contains(
                    "    [possible values: build, test]\n    [env: TASK]\n    (default: build)"
                ),
                "{page}"
            );
        }
    }
}
