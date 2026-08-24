use crate::{Spec, SpecCommand};
use std::sync::LazyLock;
use tera::Tera;

mod style;
pub use style::Style;

pub fn render_help(spec: &Spec, cmd: &SpecCommand, long: bool) -> String {
    render_help_styled(spec, cmd, long, Style::PLAIN)
}

/// Render a terminal help page with an explicit colour policy.
pub fn render_help_styled(spec: &Spec, cmd: &SpecCommand, long: bool, style: Style) -> String {
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
                        help: None,
                        help_rendered: None,
                        items: supplied,
                    },
                ),
            }
        }
    }

    let width = crate::docs::layout::help_width(cmd.term_width, cmd.max_term_width);
    ctx.insert("terminal_width", &width);
    lay_out_group_help(&mut docs_cmd.subcommand_groups, width);
    lay_out_group_help(&mut docs_cmd.arg_groups, width);
    lay_out_group_help(&mut docs_cmd.flag_groups, width);
    let col = crate::docs::layout::usage_column_width(
        docs_cmd
            .flag_groups
            .iter()
            .flat_map(|g| g.items.iter())
            .chain(inherited.iter())
            .map(|f| f.display_usage.as_str()),
        width,
    );
    let flag_column = Column {
        width,
        col,
        long,
        next_line: cmd.next_line_help,
    };
    for group in &mut docs_cmd.flag_groups {
        lay_out(&mut group.items, flag_column);
    }
    lay_out(&mut inherited, flag_column);

    // The arguments get their own column, laid out here for the page being rendered rather than
    // taken from the model's — which is the long page's, and would put a long description in the
    // short page's column unwrapped.
    let arg_col = crate::docs::layout::usage_column_width(
        docs_cmd.args.iter().map(|a| a.usage.as_str()),
        width,
    );
    for group in &mut docs_cmd.arg_groups {
        lay_out_args(
            &mut group.items,
            Column {
                width,
                col: arg_col,
                long,
                next_line: cmd.next_line_help,
            },
        );
    }

    // Flattened descendants live on this page, so this page's width governs them. Their own
    // `term_width` applies when they get a page of their own, not when an ancestor expands them.
    lay_out_flattened(&mut docs_cmd.flattened_subcommands, width, long);

    // The command list, laid out the way the flag list is: one column for the whole page, the
    // name in it, and everything else — summary, aliases, deprecation — trailing as text that
    // wraps under itself. Both pages get the same rows, so `--help` no longer reprints every
    // child's long help on its parent's page.
    let help_row = {
        let show_help_row = !docs_cmd.subcommands.is_empty()
            && !docs_cmd.flatten_help
            && !cmd.disable_help_subcommand;
        let cmd_col = crate::docs::layout::usage_column_width(
            docs_cmd
                .subcommand_groups
                .iter()
                .flat_map(|g| g.items.iter())
                .map(|c| c.name.as_str())
                .chain(show_help_row.then_some(HELP_SUBCOMMAND)),
            width,
        );
        for group in &mut docs_cmd.subcommand_groups {
            lay_out_commands(&mut group.items, width, cmd_col, cmd.next_line_help);
        }
        // Rendered here rather than in the template because it is a row like any other: it
        // sits in the same column and wraps by the same rule, and neither is something a
        // template should be working out.
        show_help_row.then(|| {
            render_row(
                HELP_SUBCOMMAND,
                HELP_SUBCOMMAND_SUMMARY,
                width,
                cmd_col,
                cmd.next_line_help,
            )
        })
    };
    ctx.insert("help_row", &help_row);

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
    let styling = style::Styling::new(&docs_cmd, &inherited, sections.usage);
    let page = match spec
        .help_template
        .as_deref()
        .filter(|t| crate::help_template::is_set(t))
    {
        Some(template) => {
            crate::help_template::substitute_with_style(template, style.coloured, |name| {
                sections
                    .named(name)
                    .map(|section| styling.apply(&section, style))
            })
        }
        None => styling.apply(&sections.concatenated(), style),
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
fn lay_out(flags: &mut [crate::docs::models::SpecFlag], column: Column) {
    for flag in flags {
        flag.usage_col_width = column.col;
        flag.help_is_block =
            !column.can_inline(crate::docs::layout::visible_width(&flag.display_usage));
        let text = if column.long {
            flag.help_long
                .as_deref()
                .or(flag.help.as_deref())
                .map(str::to_string)
        } else if column.next_line {
            flag.help.clone()
        } else {
            with_annotations(flag.help.as_deref(), flag_annotations(flag))
        };
        wrap_into(
            text,
            column,
            crate::docs::layout::visible_width(&flag.display_usage),
            &mut flag.row,
            &mut flag.help_rendered,
            &mut flag.help_is_multiline,
            &mut flag.ann_indent,
        );
    }
}

/// The same pass over a command's arguments.
///
/// `SpecCommand::from` already made one, but it made the long page's — the short page prefers
/// the short description and carries the annotations in the text — so the page it is actually
/// rendering gets the last word.
fn lay_out_args(args: &mut [crate::docs::models::SpecArg], column: Column) {
    for arg in args {
        arg.usage_col_width = column.col;
        arg.help_is_block = !column.can_inline(crate::docs::layout::visible_width(&arg.usage));
        let text = if column.long {
            arg.help_long
                .as_deref()
                .or(arg.help.as_deref())
                .map(str::to_string)
        } else if column.next_line {
            arg.help.clone()
        } else {
            with_annotations(arg.help.as_deref(), arg_annotations(arg))
        };
        wrap_into(
            text,
            column,
            crate::docs::layout::visible_width(&arg.usage),
            &mut arg.row,
            &mut arg.help_rendered,
            &mut arg.help_is_multiline,
            &mut arg.ann_indent,
        );
    }
}

fn lay_out_flattened(commands: &mut [crate::docs::models::SpecCommand], width: usize, long: bool) {
    for command in commands {
        lay_out_group_help(&mut command.arg_groups, width);
        lay_out_group_help(&mut command.flag_groups, width);
        let arg_col = crate::docs::layout::usage_column_width(
            command
                .arg_groups
                .iter()
                .flat_map(|group| group.items.iter())
                .map(|arg| arg.usage.as_str()),
            width,
        );
        let arg_column = Column {
            width,
            col: arg_col,
            long,
            next_line: command.flattened_next_line_help,
        };
        for group in &mut command.arg_groups {
            lay_out_args(&mut group.items, arg_column);
        }

        let flag_col = crate::docs::layout::usage_column_width(
            command
                .flag_groups
                .iter()
                .flat_map(|group| group.items.iter())
                .map(|flag| flag.display_usage.as_str()),
            width,
        );
        let flag_column = Column {
            width,
            col: flag_col,
            long,
            next_line: command.flattened_next_line_help,
        };
        for group in &mut command.flag_groups {
            lay_out(&mut group.items, flag_column);
        }
    }
}

fn lay_out_group_help<T>(groups: &mut [crate::docs::models::Group<T>], width: usize) {
    for group in groups {
        group.help_rendered = group
            .help
            .as_deref()
            .map(|help| crate::docs::layout::render_indented_text(help, width, 2));
    }
}

/// Fit one entry's text to the column, and say which layout it wants.
///
/// `row` is the text as composed and `help_rendered` the same text wrapped; an empty wrapping
/// is how [`crate::docs::layout::render_help_text`] says "no room, put it underneath instead",
/// which is the case the template reads `row` for.
fn wrap_into(
    text: Option<String>,
    column: Column,
    usage_width: usize,
    row: &mut Option<String>,
    help_rendered: &mut Option<String>,
    help_is_multiline: &mut bool,
    ann_indent: &mut String,
) {
    *row = None;
    *help_rendered = None;
    *help_is_multiline = false;
    // An entry with nothing in the column still has annotations to place, and the column is
    // where they go: it is the entry's own row that is empty, not the table's.
    *ann_indent = column.annotation_indent(column.can_inline(usage_width));
    let Some(text) = text else { return };
    if !column.can_inline(usage_width) {
        let indent = column.block_indent();
        *row = Some(indent_text(
            &crate::docs::layout::render_indented_text(&text, column.width, indent),
            indent,
        ));
        return;
    }
    let first_indent = column.inline_help_start(usage_width);
    let continuation_indent = column.description_indent();
    let (rendered, is_multiline) = crate::docs::layout::render_help_text_at(
        &text,
        column.width,
        first_indent,
        continuation_indent,
    );
    // `render_help_text` wraps whatever it is given; whether the page *uses* that is the
    // template's decision, and on a next-line page it does not. Both have to agree, or the
    // annotations align to a column the description never entered.
    *ann_indent = column.annotation_indent(!rendered.is_empty());
    if !rendered.is_empty() {
        *help_rendered = Some(rendered);
        *help_is_multiline = is_multiline;
    }
    *row = Some(text);
}

fn indent_text(text: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A short entry's description with its annotations joined on.
///
/// The wide layout gives each annotation a line of its own; the narrow one has no room for
/// that, so they ride along with the description — and they have to be joined *before* it is
/// wrapped, or an entry with a long description keeps its `[env: …]` out past the column where
/// the wrapping was supposed to bring the text back.
fn with_annotations(help: Option<&str>, annotations: Vec<String>) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(help) = summarize(help) {
        parts.push(help.to_string());
    }
    parts.extend(annotations);
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// What a flag's short entry says about it beyond its description.
fn flag_annotations(flag: &crate::docs::models::SpecFlag) -> Vec<String> {
    let mut parts = value_annotations(
        flag.arg.as_ref().and_then(|arg| arg.choices.as_ref()),
        flag.hide_possible_values,
        flag.env.as_deref(),
        flag.hide_env,
        &flag.env_fallback,
        &flag.deprecated_env,
        &flag.default,
        flag.hide_default_value,
    );
    if let Some(label) = deprecation_label(
        flag.deprecated.as_deref(),
        flag.deprecated_warn_at.as_deref(),
        flag.deprecated_remove_at.as_deref(),
    ) {
        parts.push(label);
    }
    parts
}

/// The same for an argument, which carries no deprecation on the narrow page.
fn arg_annotations(arg: &crate::docs::models::SpecArg) -> Vec<String> {
    value_annotations(
        arg.choices.as_ref(),
        arg.hide_possible_values,
        arg.env.as_deref(),
        arg.hide_env,
        &arg.env_fallback,
        &arg.deprecated_env,
        &arg.default,
        arg.hide_default_value,
    )
}

/// What can be said about a value, in the order the narrow page says it.
#[allow(clippy::too_many_arguments)]
fn value_annotations(
    choices: Option<&crate::SpecChoices>,
    hide_possible_values: bool,
    env: Option<&str>,
    hide_env: bool,
    env_fallback: &[String],
    deprecated_env: &[String],
    default: &[String],
    hide_default_value: bool,
) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(choices) = choices.filter(|_| !hide_possible_values) {
        if !choices.choices.is_empty() {
            parts.push(format!("[{}]", choices.choices.join(", ")));
        }
        if let Some(env) = choices.env() {
            parts.push(format!("[choices env: {env}]"));
        }
    }
    if !hide_env {
        if let Some(env) = env {
            parts.push(format!("[env: {env}]"));
        }
        parts.extend(
            env_fallback
                .iter()
                .map(|env| format!("[env fallback: {env}]")),
        );
        parts.extend(
            deprecated_env
                .iter()
                .map(|env| format!("[deprecated env: {env}]")),
        );
    }
    if !hide_default_value && !default.is_empty() {
        parts.push(format!("(default: {})", default.join(", ")));
    }
    parts
}

/// What a page is fitting its entries to.
#[derive(Clone, Copy)]
struct Column {
    /// The width the page is laid out for.
    width: usize,
    /// How wide the usage column is.
    col: usize,
    /// The long page, which prefers the long description and gives each annotation a line.
    long: bool,
    /// A page that puts every description under its usage rather than beside it.
    next_line: bool,
}

/// The indent a page uses when it cannot align to its column.
const BLOCK_INDENT: usize = 4;

impl Column {
    fn usage_overflows(&self, usage_width: usize) -> bool {
        !self.next_line && usage_width > self.col
    }

    fn description_indent(&self) -> usize {
        2 + self.col + 2
    }

    fn inline_help_start(&self, usage_width: usize) -> usize {
        if self.usage_overflows(usage_width) {
            2 + usage_width + 2
        } else {
            self.description_indent()
        }
    }

    /// Keep an outlier inline only when its spelling leaves a useful amount of prose.
    fn can_inline(&self, usage_width: usize) -> bool {
        if self.next_line {
            return false;
        }
        let minimum = if self.usage_overflows(usage_width) {
            crate::docs::layout::MIN_INLINE_HELP_WIDTH
        } else {
            10
        };
        self.width
            .saturating_sub(self.inline_help_start(usage_width))
            >= minimum
    }

    /// Where an entry's annotations are indented to.
    ///
    /// The description column, when the description reached it — an annotation is a note about
    /// the same entry and belongs under the text it qualifies, not in the gutter beside a
    /// column it is ignoring. Where the description is already a block underneath, there is no
    /// column to align to and the annotations join it there.
    fn annotation_indent(&self, reached_column: bool) -> String {
        let indent = if reached_column {
            self.description_indent()
        } else {
            self.block_indent()
        };
        " ".repeat(indent)
    }

    /// Prefer the normal description column for a stacked entry whenever it still leaves a
    /// useful line. Exceptionally narrow pages retain the compact four-space fallback.
    fn block_indent(&self) -> usize {
        let description = self.description_indent();
        if !self.next_line && self.width.saturating_sub(description) >= 10 {
            description
        } else {
            BLOCK_INDENT
        }
    }
}

/// The entry every command list ends with, unless the CLI turned it off.
const HELP_SUBCOMMAND: &str = "help";
const HELP_SUBCOMMAND_SUMMARY: &str = "Print this message or the help of the given subcommand(s)";

/// Fit a list of subcommand summaries to the page's command column.
///
/// `lay_out`'s counterpart for the command list: the same column, the same wrapping, the same
/// "an empty rendering means use the block layout" signal to the template.
fn lay_out_commands(
    commands: &mut [crate::docs::models::HelpCommand],
    terminal_width: usize,
    col: usize,
    next_line: bool,
) {
    let column = Column {
        width: terminal_width,
        col,
        long: false,
        next_line,
    };
    for command in commands {
        command.usage_col_width = col;
        command.row = command_row(command);
        command.help_rendered = None;
        command.help_is_multiline = false;
        let usage_width = crate::docs::layout::visible_width(&command.name);
        if let Some(row) = command.row.as_deref() {
            if column.can_inline(usage_width) {
                let (rendered, is_multiline) = crate::docs::layout::render_help_text_at(
                    row,
                    terminal_width,
                    column.inline_help_start(usage_width),
                    column.description_indent(),
                );
                if !rendered.is_empty() {
                    command.help_rendered = Some(rendered);
                    command.help_is_multiline = is_multiline;
                }
            } else if let Some(row) = command.row.as_mut() {
                let indent = column.block_indent();
                *row = indent_text(
                    &crate::docs::layout::render_indented_text(row, terminal_width, indent),
                    indent,
                );
            }
        }
    }
}

/// Everything that follows a command's name in its parent's list, as one string.
///
/// The name alone occupies the column, so the summaries line up down the page and the syntax a
/// command takes belongs to that command's own page. What qualifies the command rather than
/// describing it — the names it also answers to, that it is going away — trails the summary,
/// where it wraps with the text instead of pushing it out of the column.
fn command_row(cmd: &crate::docs::models::HelpCommand) -> Option<String> {
    let mut parts = Vec::new();
    // A command that wrote only `help_long` still has a summary: its first line. Both pages
    // read the same one, so `-h` never says less about a command than `--help` does.
    let summary = summarize(cmd.help.as_deref()).or_else(|| {
        summarize(
            cmd.help_long
                .as_deref()
                .and_then(|help| help.lines().next()),
        )
    });
    if let Some(summary) = summary {
        parts.push(summary.to_string());
    }
    if !cmd.aliases.is_empty() {
        parts.push(format!("[aliases: {}]", cmd.aliases.join(", ")));
    }
    if let Some(label) = deprecation_label(
        cmd.deprecated.as_deref(),
        cmd.deprecated_warn_at.as_deref(),
        cmd.deprecated_remove_at.as_deref(),
    ) {
        parts.push(label);
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// A description reduced to what a list can show, or nothing if it says nothing.
fn summarize(text: Option<&str>) -> Option<&str> {
    text.map(str::trim_end).filter(|text| !text.is_empty())
}

/// How a page says something is going away, in the one place both lists read it from.
fn deprecation_label(
    message: Option<&str>,
    warn_at: Option<&str>,
    remove_at: Option<&str>,
) -> Option<String> {
    if message.is_none() && warn_at.is_none() && remove_at.is_none() {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(message) = message {
        parts.push(message.to_string());
    }
    if let Some(at) = warn_at {
        parts.push(format!("warns at {at}"));
    }
    if let Some(at) = remove_at {
        parts.push(format!("removed at {at}"));
    }
    Some(format!("[deprecated: {}]", parts.join("; ")))
}

/// One command row, ready to print — the form the synthetic `help` entry takes.
fn render_row(
    name: &str,
    row: &str,
    terminal_width: usize,
    col: usize,
    next_line_help: bool,
) -> String {
    if !next_line_help {
        if crate::docs::layout::visible_width(name) <= col {
            let (rendered, _) = crate::docs::layout::render_help_text(row, terminal_width, col);
            if !rendered.is_empty() {
                return format!("  {name:<col$}  {rendered}");
            }
        } else if !row.contains('\n') {
            return format!(
                "  {name}\n    {}",
                crate::docs::layout::render_block_text(row, terminal_width)
            );
        }
    }
    format!("  {name}\n    {row}")
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
    tera.register_filter(
        "terminal_wrap",
        |value: &tera::Value, args: tera::Kwargs, _: &tera::State| -> tera::TeraResult<String> {
            let value = value.as_str().unwrap_or("");
            let width = args.get::<u64>("width")?.unwrap_or(80) as usize;
            let indent = args.get::<u64>("indent")?.unwrap_or(0) as usize;
            Ok(crate::docs::layout::render_indented_text(
                value, width, indent,
            ))
        },
    );
    tera.register_filter(
        "terminal_label",
        |value: &tera::Value, args: tera::Kwargs, _: &tera::State| -> tera::TeraResult<String> {
            let value = value.as_str().unwrap_or("");
            let label = args.must_get::<String>("label")?;
            let width = args.get::<u64>("width")?.unwrap_or(80) as usize;
            let indent = args.get::<u64>("indent")?.unwrap_or(0) as usize;
            Ok(crate::docs::layout::render_labelled_text(
                &label, value, width, indent,
            ))
        },
    );
    tera.register_filter(
        "terminal_annotation",
        |value: &tera::Value, args: tera::Kwargs, _: &tera::State| -> tera::TeraResult<String> {
            let body = if let Some(values) = value.as_array() {
                values
                    .iter()
                    .filter_map(tera::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                value.as_str().unwrap_or("").to_string()
            };
            let label = args.get::<String>("label")?.unwrap_or_default();
            let suffix = args.get::<String>("suffix")?.unwrap_or_default();
            let indent = args
                .get::<String>("indent")?
                .unwrap_or_default()
                .chars()
                .count();
            let width = args.get::<u64>("width")?.unwrap_or(80) as usize;
            let rendered = crate::docs::layout::render_indented_text(
                &format!("{label}{body}{suffix}"),
                width,
                indent,
            );
            Ok(indent_text(&rendered, indent))
        },
    );
    tera.register_filter(
        "terminal_deprecation",
        |value: &tera::Value, args: tera::Kwargs, _: &tera::State| -> tera::TeraResult<String> {
            let field = |name| value.get_from_path(name).and_then(tera::Value::as_str);
            let Some(label) = deprecation_label(
                field("deprecated"),
                field("deprecated_warn_at"),
                field("deprecated_remove_at"),
            ) else {
                return Ok(String::new());
            };
            let indent = args
                .get::<String>("indent")?
                .unwrap_or_default()
                .chars()
                .count();
            let width = args.get::<u64>("width")?.unwrap_or(80) as usize;
            let rendered = crate::docs::layout::render_indented_text(&label, width, indent);
            Ok(indent_text(&rendered, indent))
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
          run   Run it
          help  Print this message or the help of the given subcommand(s)

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
          old-cmd  Do something [deprecated: use new-cmd instead; warns at 6.2; removed
                   at 7.0]
          help     Print this message or the help of the given subcommand(s)

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
            page.contains("old-cmd  Do something [deprecated: warns at 6.2]"),
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

    #[test]
    fn styled_help_colours_semantics_and_template_styles() {
        let spec = crate::spec! { r#"
bin "testcli"
help_template "{$bright-blue}My tool{/$}\n\n{{usage}}\n\n{{flags}}"
flag "--output <FILE>" help="Write **the file**"
        "# }
        .unwrap();

        let plain = render_help(&spec, &spec.cmd, true);
        assert!(plain.contains("My tool"), "{plain}");
        assert!(!plain.contains('\u{1b}'), "{plain:?}");

        let coloured = render_help_styled(&spec, &spec.cmd, true, Style::COLOURED);
        assert!(
            coloured.contains("\u{1b}[94mMy tool\u{1b}[0m"),
            "{coloured:?}"
        );
        assert!(
            coloured.contains("\u{1b}[1;33mUsage:\u{1b}[0m"),
            "{coloured:?}"
        );
        assert!(
            coloured.contains("\u{1b}[1;32m--output\u{1b}[0m"),
            "{coloured:?}"
        );
        assert!(
            coloured.contains("\u{1b}[1;35m<FILE>\u{1b}[0m"),
            "{coloured:?}"
        );
        assert!(
            coloured.contains("\u{1b}[1mthe file\u{1b}[22m"),
            "{coloured:?}"
        );
    }
}
