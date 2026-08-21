use crate::docs::markdown::MarkdownRenderer;
use crate::spec::effect::SpecCommandEffect;
use crate::SpecChoices;
use indexmap::IndexMap;
use serde::Serialize;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Spec {
    pub name: String,
    pub bin: String,
    pub cmd: SpecCommand,
    pub config: SpecConfig,
    pub version: Option<String>,
    pub usage: String,
    // pub complete: IndexMap<String, SpecComplete>,
    pub source_code_link_template: Option<String>,
    pub repository: Option<String>,
    pub author: Option<String>,
    pub about: Option<String>,
    pub about_long: Option<String>,
    pub about_md: Option<String>,
    pub license: Option<String>,
    pub before_help: Option<String>,
    pub after_help: Option<String>,
    pub before_help_long: Option<String>,
    pub after_help_long: Option<String>,
    pub disable_help: Option<bool>,
    pub min_usage_version: Option<String>,
    pub examples: Vec<SpecExample>,
    pub rendered: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct SpecCommand {
    pub full_cmd: Vec<String>,
    pub usage: String,
    pub subcommands: IndexMap<String, SpecCommand>,
    /// Immediate children in help presentation order, without duplicating their trees.
    pub help_subcommands: Vec<HelpCommand>,
    /// Visible subcommand summaries partitioned by their own `help_heading`.
    pub subcommand_groups: Vec<Group<HelpCommand>>,
    pub args: Vec<SpecArg>,
    pub flags: Vec<SpecFlag>,
    /// `flags`, partitioned by `help_heading`. Same flags, same order.
    pub flag_groups: Vec<Group<SpecFlag>>,
    /// `args`, partitioned by `help_heading`.
    pub arg_groups: Vec<Group<SpecArg>>,
    // pub mounts: Vec<SpecMount>,
    pub deprecated: Option<String>,
    pub deprecated_warn_at: Option<String>,
    pub deprecated_remove_at: Option<String>,
    pub effect: Option<SpecCommandEffect>,
    pub hide: bool,
    pub help_heading: Option<String>,
    pub display_order: Option<usize>,
    pub subcommand_required: bool,
    pub subcommand_help_heading: Option<String>,
    pub next_line_help: bool,
    pub flatten_help: bool,
    pub args_conflicts_with_subcommands: bool,
    pub flattened_usage: Vec<String>,
    /// Visible descendants rendered as sections when an ancestor flattens help.
    pub flattened_subcommands: Vec<SpecCommand>,
    /// The flattening parent's layout policy for this section.
    pub flattened_next_line_help: bool,
    pub restart_token: Option<String>,
    pub help: Option<String>,
    pub help_long: Option<String>,
    pub help_md: Option<String>,
    pub name: String,
    pub aliases: Vec<String>,
    pub hidden_aliases: Vec<String>,
    pub before_help: Option<String>,
    pub before_help_long: Option<String>,
    pub before_help_md: Option<String>,
    pub after_help: Option<String>,
    pub after_help_long: Option<String>,
    pub after_help_md: Option<String>,
    pub examples: Vec<SpecExample>,
    // pub complete: IndexMap<String, SpecComplete>,
    pub rendered: bool,
}

/// The fields an immediate child contributes to its parent's command list.
///
/// Keeping this separate from [`SpecCommand`] avoids cloning every descendant tree for every
/// ancestor merely to put one level of command summaries in presentation order.
#[derive(Debug, Serialize, Clone)]
pub struct HelpCommand {
    pub usage: String,
    pub deprecated: Option<String>,
    pub deprecated_warn_at: Option<String>,
    pub deprecated_remove_at: Option<String>,
    pub aliases: Vec<String>,
    pub help: Option<String>,
    pub help_long: Option<String>,
    pub help_heading: Option<String>,
}

impl From<&SpecCommand> for HelpCommand {
    fn from(cmd: &SpecCommand) -> Self {
        Self {
            usage: cmd.usage.clone(),
            deprecated: cmd.deprecated.clone(),
            deprecated_warn_at: cmd.deprecated_warn_at.clone(),
            deprecated_remove_at: cmd.deprecated_remove_at.clone(),
            aliases: cmd.aliases.clone(),
            help: cmd.help.clone(),
            help_long: cmd.help_long.clone(),
            help_heading: cmd.help_heading.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecFlag {
    pub name: String,
    pub effect: Option<crate::spec::effect::SpecCommandEffect>,
    /// What this flag means for how much the CLI says, when it says.
    ///
    /// Skipped when absent, which is almost always: a template asks `{% if flag.verbosity %}`
    /// either way, and a key per flag per page is a cost every CLI pays for a property
    /// almost none of their flags carry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<crate::spec::policy::SpecVerbosityRole>,
    /// What this flag means for color, when it says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<crate::spec::policy::SpecColorRole>,
    pub usage: String,
    pub display_usage: String,
    pub help: Option<String>,
    pub help_long: Option<String>,
    pub help_md: Option<String>,
    pub help_first_line: Option<String>,
    pub short: Vec<char>,
    pub long: Vec<String>,
    pub required: bool,
    pub deprecated: Option<String>,
    pub deprecated_warn_at: Option<String>,
    pub deprecated_remove_at: Option<String>,
    pub var: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    pub hide: bool,
    pub hide_default_value: bool,
    pub hide_env: bool,
    pub hide_env_values: bool,
    pub hide_possible_values: bool,
    pub hide_short_help: bool,
    pub hide_long_help: bool,
    pub global: bool,
    pub count: bool,
    pub arg: Option<SpecArg>,
    pub default: Vec<String>,
    pub negate: Option<String>,
    pub env: Option<String>,
    pub env_fallback: Vec<String>,
    pub deprecated_env: Vec<String>,
    pub help_heading: Option<String>,
    pub display_order: Option<usize>,
    pub rendered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_rendered: Option<String>,
    pub help_is_multiline: bool,
    pub usage_col_width: usize,
}

/// Flags or arguments that share a heading, in the order the headings first
/// appear.
///
/// Grouping happens here rather than in a template because the template language
/// cannot do it: Tera can filter by an attribute's value but not partition on one,
/// and "everything without a heading" is not expressible as a filter. The
/// ungrouped entries come first with `heading` unset, which is what a renderer
/// shows under its default title.
#[derive(Debug, Default, Serialize, Clone)]
pub struct Group<T> {
    pub heading: Option<String>,
    pub items: Vec<T>,
}

/// A CLI's settings, as documentation sees them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpecConfig {
    /// Every property, hidden ones already removed.
    pub props: Vec<SpecConfigProp>,
    /// `props`, partitioned by `help_heading`. Same props, same order.
    pub prop_groups: Vec<Group<SpecConfigProp>>,
    /// Config file locations, in the precedence order the spec declared.
    pub files: Vec<SpecConfigFile>,
    rendered: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpecConfigFile {
    pub path: String,
    pub findup: bool,
    pub scope: String,
    pub format: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpecConfigProp {
    pub key: String,
    pub aliases: Vec<String>,
    pub optional: Option<bool>,
    /// The type as written, or nothing when the spec did not say.
    pub type_: Option<String>,
    pub default: Option<String>,
    pub default_note: Option<String>,
    pub help: Option<String>,
    pub help_long: Option<String>,
    pub help_md: Option<String>,
    pub help_heading: Option<String>,
    /// One line per way of setting it, already worded: "`--jobs`, `-j`",
    /// "`HK_JOBS` (or `HK_JOB`)", "git config `hk.jobs`". Built here rather than in the
    /// template so every renderer says it the same way.
    pub sources: Vec<String>,
    pub choices: Vec<SpecConfigChoice>,
    pub deprecated: Option<String>,
    pub deprecated_remove_at: Option<String>,
    pub since: Option<String>,
    pub examples: Vec<String>,
    /// `union`/`deep` when the spec said so, nothing for the default.
    pub merge: Option<String>,
    /// Where a value may come from, when the spec restricted it.
    pub scope: Option<String>,
    rendered: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpecConfigChoice {
    pub value: String,
    pub help: Option<String>,
}

impl SpecConfig {
    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        for prop in &mut self.props {
            prop.render_md(renderer);
        }
        for group in &mut self.prop_groups {
            for prop in &mut group.items {
                prop.render_md(renderer);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.props.is_empty() && self.files.is_empty()
    }
}

impl SpecConfigProp {
    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        if let Some(help) = &mut self.help_md {
            *help = renderer.replace_code_fences(help.to_string());
        }
    }
}

impl From<&crate::spec::config::SpecConfig> for SpecConfig {
    fn from(config: &crate::spec::config::SpecConfig) -> Self {
        // Hidden props are dropped here rather than in the template: every consumer of
        // this model wants them gone, and a template that forgets the filter leaks them.
        let props: Vec<SpecConfigProp> = config
            .props
            .iter()
            .filter(|(_, prop)| !prop.hide)
            .map(|(key, prop)| SpecConfigProp::new(key, prop, config))
            .collect();
        Self {
            prop_groups: group_by_heading(&props, |p| p.help_heading.as_deref()),
            props,
            files: config
                .files
                .iter()
                .map(|file| SpecConfigFile {
                    path: file.path.clone(),
                    findup: file.findup,
                    scope: file.scope.to_string(),
                    format: file.format.clone(),
                })
                .collect(),
            rendered: false,
        }
    }
}

impl SpecConfigProp {
    fn new(
        key: &str,
        prop: &crate::spec::config::SpecConfigProp,
        config: &crate::spec::config::SpecConfig,
    ) -> Self {
        Self {
            key: key.to_string(),
            aliases: prop.aliases.clone(),
            optional: prop.optional,
            type_: prop.value_type.as_ref().map(|t| t.to_string()),
            default: prop
                .default_note
                .clone()
                .or_else(|| prop.default.as_ref().map(|d| d.display()))
                .or_else(|| {
                    (!prop.default_list.is_empty()).then(|| {
                        prop.default_list
                            .iter()
                            .map(|value| value.display())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                }),
            default_note: prop.default_note.clone(),
            help: prop.help.clone(),
            help_long: prop.long_help.clone(),
            help_md: prop.long_help.clone().or_else(|| prop.help.clone()),
            help_heading: prop.help_heading.clone(),
            sources: describe_sources(prop, config),
            choices: prop
                .choices
                .iter()
                .map(|choice| SpecConfigChoice {
                    value: choice.value.display(),
                    help: choice.help.clone(),
                })
                .collect(),
            deprecated: prop.deprecated.clone(),
            deprecated_remove_at: prop.deprecated_remove_at.clone(),
            since: prop.since.clone(),
            examples: prop.examples.clone(),
            merge: (prop.merge != crate::spec::config::SpecConfigMerge::default())
                .then(|| prop.merge.to_string()),
            scope: (prop.scope != crate::spec::config::SpecConfigScope::default())
                .then(|| prop.scope.to_string()),
            rendered: false,
        }
    }
}

/// Every way of setting one property, in words.
///
/// A custom source kind is rendered from what the spec said about it — `doc_hint` with
/// `{key}` substituted — so a page can describe a git config or an `.npmrc` without usage
/// knowing anything about either.
fn describe_sources(
    prop: &crate::spec::config::SpecConfigProp,
    config: &crate::spec::config::SpecConfig,
) -> Vec<String> {
    let mut described = Vec::new();
    if !prop.cli.is_empty() {
        let flags: Vec<String> = prop.cli.iter().map(|f| format!("`{f}`")).collect();
        described.push(flags.join(", "));
    }
    match prop.envs.as_slice() {
        [] => {}
        [one] => described.push(format!("`{one}`")),
        [first, rest @ ..] => {
            let aliases: Vec<String> = rest.iter().map(|e| format!("`{e}`")).collect();
            described.push(format!("`{first}` (or {})", aliases.join(", ")));
        }
    }
    if !prop.deprecated_envs.is_empty() {
        let names: Vec<String> = prop
            .deprecated_envs
            .iter()
            .map(|env| format!("`{env}`"))
            .collect();
        described.push(format!("{} (deprecated)", names.join(", ")));
    }
    for (kind, keys) in &prop.bindings {
        let declared = config.sources.get(kind);
        let name = declared
            .and_then(|s| s.name.clone())
            .unwrap_or_else(|| kind.clone());
        for key in keys {
            match declared.and_then(|s| s.doc_hint.as_ref()) {
                Some(hint) => described.push(hint.replace("{key}", key)),
                None => described.push(format!("{name} `{key}`")),
            }
        }
    }
    described
}

/// Partition by heading, keeping declaration order within each group and putting
/// the unheaded entries first.
fn group_by_heading<T: Clone>(
    items: &[T],
    heading_of: impl Fn(&T) -> Option<&str>,
) -> Vec<Group<T>> {
    let mut groups: Vec<Group<T>> = Vec::new();
    // The unheaded group exists only if something lands in it, so a CLI that
    // gives every flag a heading does not render an empty default section.
    for item in items {
        let heading = heading_of(item).map(|h| h.to_string());
        match groups.iter_mut().find(|g| g.heading == heading) {
            Some(group) => group.items.push(item.clone()),
            None => groups.push(Group {
                heading,
                items: vec![item.clone()],
            }),
        }
    }
    groups.sort_by_key(|g| g.heading.is_some());
    groups
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct SpecExample {
    pub code: String,
    pub header: Option<String>,
    pub help: Option<String>,
    pub lang: String,
    pub rendered: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecArg {
    pub name: String,
    pub usage: String,
    pub help: Option<String>,
    pub help_long: Option<String>,
    pub help_md: Option<String>,
    pub help_first_line: Option<String>,
    pub required: bool,
    pub var: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    pub hide: bool,
    pub hide_default_value: bool,
    pub hide_env: bool,
    pub hide_env_values: bool,
    pub hide_possible_values: bool,
    pub hide_short_help: bool,
    pub hide_long_help: bool,
    pub default: Vec<String>,
    pub choices: Option<SpecChoices>,
    pub validate: Option<String>,
    pub validate_error: Option<String>,
    pub env: Option<String>,
    pub env_fallback: Vec<String>,
    pub deprecated_env: Vec<String>,
    pub help_heading: Option<String>,
    pub display_order: Option<usize>,
    pub rendered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_rendered: Option<String>,
    pub help_is_multiline: bool,
    pub usage_col_width: usize,
}

impl From<crate::Spec> for Spec {
    fn from(spec: crate::Spec) -> Self {
        Self {
            name: spec.name,
            bin: spec.bin,
            cmd: SpecCommand::from(&spec.cmd),
            config: SpecConfig::from(&spec.config),
            version: spec.version,
            usage: spec.usage,
            // complete: spec.complete,
            source_code_link_template: spec.source_code_link_template,
            repository: spec.repository,
            about: spec.about,
            // The program's own description, trimmed for the reason a command's is: the blank
            // line under it is the renderer's to write.
            about_long: spec.about_long.as_deref().map(|a| a.trim_end().to_string()),
            about_md: spec.about_md,
            author: spec.author,
            license: spec.license,
            before_help: spec.before_help,
            after_help: spec.after_help,
            before_help_long: spec.before_help_long,
            after_help_long: spec.after_help_long,
            disable_help: spec.disable_help,
            min_usage_version: spec.min_usage_version,
            examples: spec.examples.iter().map(SpecExample::from).collect(),
            rendered: false,
        }
    }
}

impl From<&crate::SpecCommand> for SpecCommand {
    fn from(cmd: &crate::SpecCommand) -> Self {
        use crate::docs::layout::{help_width, max_usage_width, render_help_text};

        let terminal_width = help_width(cmd.term_width, cmd.max_term_width);
        let flattened_usage = if cmd.flatten_help {
            let mut lines = Vec::new();
            if !cmd.subcommand_required || cmd.args_conflicts_with_subcommands {
                lines.push(cmd.usage_without_subcommands());
            }
            let mut children: Vec<_> = cmd
                .subcommands
                .values()
                .filter(|sub| !sub.hide)
                .map(|sub| (sub.display_order.unwrap_or(999), sub.usage()))
                .collect();
            children.sort();
            lines.extend(children.into_iter().map(|(_, usage)| usage));
            lines
        } else {
            Vec::new()
        };

        // Calculate layout for args
        let args_usage_col_width = max_usage_width(cmd.args.iter().map(|a| a.usage.as_str()));
        let mut args: Vec<(usize, SpecArg)> = cmd
            .args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                let mut spec_arg = SpecArg::from(arg);

                // Get help text (prefer help_long over help)
                let help_text = spec_arg.help_long.as_deref().or(spec_arg.help.as_deref());

                if let Some(help) = help_text {
                    let (rendered, is_multiline) =
                        render_help_text(help, terminal_width, args_usage_col_width);
                    // Only set help_rendered if we have content (empty string signals block layout)
                    if !rendered.is_empty() {
                        spec_arg.help_rendered = Some(rendered);
                        spec_arg.help_is_multiline = is_multiline;
                    }
                }

                spec_arg.usage_col_width = args_usage_col_width;
                (index, spec_arg)
            })
            .collect();
        args.sort_by_key(|(_, arg)| arg.display_order.unwrap_or(999));
        let args: Vec<SpecArg> = args.into_iter().map(|(_, arg)| arg).collect();

        // Calculate layout for flags
        let mut flags: Vec<(usize, SpecFlag)> = cmd
            .flags
            .iter()
            .enumerate()
            .map(|(index, flag)| (index, SpecFlag::from(flag)))
            .collect();
        flags.sort_by_key(|(_, flag)| flag.display_order.unwrap_or(999));
        let flags: Vec<SpecFlag> = flags.into_iter().map(|(_, flag)| flag).collect();
        let flags_usage_col_width = max_usage_width(flags.iter().map(|f| f.display_usage.as_str()));
        let flags: Vec<SpecFlag> = flags
            .into_iter()
            .map(|mut spec_flag| {
                // Get help text (prefer help_long over help)
                let help_text = spec_flag.help_long.as_deref().or(spec_flag.help.as_deref());

                if let Some(help) = help_text {
                    let (rendered, is_multiline) =
                        render_help_text(help, terminal_width, flags_usage_col_width);
                    // Only set help_rendered if we have content (empty string signals block layout)
                    if !rendered.is_empty() {
                        spec_flag.help_rendered = Some(rendered);
                        spec_flag.help_is_multiline = is_multiline;
                    }
                }

                spec_flag.usage_col_width = flags_usage_col_width;
                spec_flag
            })
            .collect();

        // Destructured exhaustively (no `..`) so that adding a field to the spec
        // model fails to compile until this decides whether docs need it.
        let crate::SpecCommand {
            full_cmd,
            usage,
            subcommands,
            deprecated,
            deprecated_warn_at,
            deprecated_remove_at,
            effect,
            hide,
            help_heading,
            display_order,
            subcommand_required,
            subcommand_help_heading,
            subcommand_value_name: _,
            next_line_help,
            flatten_help,
            // Consumed above while laying help out; templates need only the result.
            term_width: _,
            max_term_width: _,
            help,
            help_long,
            help_md,
            name,
            aliases,
            hidden_aliases,
            before_help,
            before_help_long,
            before_help_md,
            after_help,
            after_help_long,
            after_help_md,
            examples,
            restart_token,
            // How a command line is read, which no rendered page shows.
            unknown_flags: _,
            external_subcommand: _,
            arg_required_else_help: _,
            dont_delimit_trailing_values: _,
            args_override_self: _,
            subcommand_negates_reqs: _,
            args_conflicts_with_subcommands,
            disable_help_flag: _,
            disable_help_subcommand: _,
            disable_version_flag: _,
            subcommand_precedence_over_arg: _,
            allow_missing_positional: _,
            // Rendered above, or deliberately absent from the docs model.
            args: _,
            flags: _,
            // Where a flag was declared is not something a rendered page shows: a `use` is
            // resolved before docs are generated, and the flags it named are in `flags`.
            uses: _,
            mounts: _,
            complete: _,
            mounted: _,
            flags_from_mount: _,
            subcommand_lookup: _,
            // Presentational output does not describe relationships between flags, the
            // way it already does not describe `conflicts`.
            groups: _,
        } = cmd;

        let rendered_subcommands: IndexMap<String, SpecCommand> = subcommands
            .iter()
            .map(|(key, command)| (key.clone(), SpecCommand::from(command)))
            .collect();
        let mut help_order: Vec<_> = subcommands
            .iter()
            .map(|(name, command)| (command.display_order.unwrap_or(999), command.usage(), name))
            .collect();
        help_order.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(b.2))
        });
        let help_subcommands: Vec<HelpCommand> = help_order
            .iter()
            .map(|(_, _, key)| {
                let mut command = HelpCommand::from(
                    rendered_subcommands
                        .get(*key)
                        .expect("rendered subcommand retains its key"),
                );
                command.help_heading = command
                    .help_heading
                    .as_ref()
                    .filter(|heading| {
                        heading.as_str() != subcommand_help_heading.as_deref().unwrap_or("Commands")
                    })
                    .cloned();
                command
            })
            .collect();
        let mut subcommand_groups =
            group_by_heading(&help_subcommands, |command| command.help_heading.as_deref());
        subcommand_groups.sort_by_key(|group| group.heading.is_some());
        if !help_subcommands.is_empty()
            && subcommand_groups
                .first()
                .is_none_or(|group| group.heading.is_some())
        {
            subcommand_groups.insert(
                0,
                Group {
                    heading: None,
                    items: Vec::new(),
                },
            );
        }
        let mut flattened_subcommands = Vec::new();
        if *flatten_help {
            for (_, _, key) in help_order {
                let sub = rendered_subcommands
                    .get(key)
                    .expect("rendered subcommand retains its key");
                if sub.hide {
                    continue;
                }
                let mut section = sub.clone();
                section.flattened_next_line_help = *next_line_help;
                flattened_subcommands.push(section);
                flattened_subcommands.extend(sub.flattened_subcommands.iter().cloned());
            }
        }

        Self {
            full_cmd: full_cmd.clone(),
            usage: usage.clone(),
            subcommands: rendered_subcommands,
            help_subcommands,
            subcommand_groups,
            flag_groups: group_by_heading(&flags, |f| f.help_heading.as_deref()),
            arg_groups: group_by_heading(&args, |a| a.help_heading.as_deref()),
            args,
            flags,
            deprecated: deprecated.clone(),
            deprecated_warn_at: deprecated_warn_at.clone(),
            deprecated_remove_at: deprecated_remove_at.clone(),
            effect: *effect,
            hide: *hide,
            help_heading: help_heading.clone(),
            display_order: *display_order,
            subcommand_required: *subcommand_required,
            subcommand_help_heading: subcommand_help_heading.clone(),
            next_line_help: *next_line_help,
            flatten_help: *flatten_help,
            args_conflicts_with_subcommands: *args_conflicts_with_subcommands,
            flattened_usage,
            flattened_subcommands,
            flattened_next_line_help: false,
            restart_token: restart_token.clone(),
            // The renderer owns the line break after a command description. Keeping one
            // embedded in the text creates an extra blank in next-line help.
            help: help.as_deref().map(|help| help.trim_end().to_string()),
            // Trailing whitespace trimmed, matching `long_commands_section` in usage-argv.
            // Never intent, and it showed: pitchfork's `daemons add` ends its examples block
            // with a newline, which put a stray blank line in the middle of `Commands:`.
            //
            // A decision, recorded: both renderers move together, so the page loses whitespace
            // no one wrote on purpose rather than one side reproducing it faithfully.
            help_long: help_long.as_deref().map(|h| h.trim_end().to_string()),
            help_md: help_md.clone(),
            name: name.clone(),
            aliases: aliases.clone(),
            hidden_aliases: hidden_aliases.clone(),
            before_help: before_help.clone(),
            before_help_long: before_help_long.clone(),
            before_help_md: before_help_md.clone(),
            after_help: after_help.clone(),
            after_help_long: after_help_long.clone(),
            after_help_md: after_help_md.clone(),
            examples: examples.iter().map(SpecExample::from).collect(),
            rendered: false,
        }
    }
}

/// Help text, with whitespace-only treated as none.
///
/// `usage-argv` filters a blank description out everywhere it reads one, so a spec written with
/// `help="   "` produced a padded column and a line of trailing spaces here and nothing there —
/// two renderings of the same metadata. Normalised once, where the model is built, so every
/// renderer downstream sees the same answer.
fn said(help: &Option<String>) -> Option<String> {
    help.as_ref().filter(|h| !h.trim().is_empty()).cloned()
}

/// The width of the short column: `-x, `, or the blank that stands in for it.
///
/// Fixed, because a short form is one character. clap's, measured.
const SHORT_COL: usize = 4;

/// A flag as the *flags section* lists it, with its long form in a column of its own.
///
/// Separate from `SpecFlag::usage`, which feeds the usage line and the markdown and manpage
/// renderers — `Usage: ex [-f --force]` must not be padded, and this must be. clap's shape,
/// measured from clap 4:
///
/// ```text
///       --github-release
///   -n, --dry-run
///   -o, --output <OUTPUT>
///   -j <JOBS>
/// ```
///
/// The short column is only spent where there is a long form to line up *with*: a flag with no
/// long one writes `-j <JOBS>` and does not pad, which is what clap does. A flag whose declared
/// name the forms do not imply — `verbose: -v`, which clap has no equivalent for — takes the
/// same path.
///
/// The twin of `column_usage` in `usage-argv`'s `help` module; the two must agree, and the gate
/// over mise's spec is what says they do.
fn column_usage(flag: &crate::SpecFlag) -> String {
    let usage = flag.usage.trim();
    let rest = match flag.negate.as_deref().map(str::trim) {
        // `SpecFlag::usage` already writes the negation for a flag that has no other
        // spelling — clap's `SetFalse`, tak's `--no-credit` — and appending it again rendered
        // `--color / --color`.
        Some(negate) if negate == usage => usage.to_string(),
        Some(negate) => format!("{usage} / {negate}"),
        None => usage.to_string(),
    };
    let Some(long) = flag.long.first() else {
        return rest;
    };
    // The dashes matter: `long` is stored bare, and searching for `cd` finds the `cd` inside
    // `--cd` — which split `-C --cd` into `-C --` and `cd`, and rendered `-C --,cd`.
    let Some(at) = rest.find(&format!("--{long}")) else {
        return rest;
    };
    let (before, after) = rest.split_at(at);
    let short = before.trim();
    // Only a bare short form belongs in the short column — see the twin in `usage-argv`. A
    // declared name the forms do not imply (`jobs: -j --parallel`) is not one, and gluing a
    // comma to it lost the space before the long form.
    let bare_short = short.is_empty()
        || (short.starts_with('-') && !short.starts_with("--") && short.chars().count() == 2);
    if !bare_short {
        return rest;
    }
    let short = match short {
        "" => String::new(),
        s => format!("{s},"),
    };
    format!("{short:<SHORT_COL$}{after}")
}

/// Every visible spelling for generated reference documentation.
///
/// Interactive help keeps the compact first short/long pair in its aligned
/// column, while a reference page should document aliases users can type.
fn reference_usage(flag: &crate::SpecFlag) -> String {
    let mut forms: Vec<String> = flag
        .short
        .iter()
        .filter(|short| !flag.hidden_short_aliases.contains(short))
        .map(|short| format!("-{short}"))
        .chain(
            flag.long
                .iter()
                .filter(|long| !flag.hidden_aliases.contains(long))
                .map(|long| format!("--{long}")),
        )
        .collect();
    // A flag whose only spelling is its negation — clap's `SetFalse`, tak's `--no-credit` —
    // has no long or short form to list, so without this the reference heading was empty.
    if forms.is_empty() {
        if let Some(negate) = &flag.negate {
            forms.push(negate.clone());
        }
    }
    if flag.usage.trim().starts_with(&format!("{}:", flag.name)) {
        forms.insert(0, format!("{}:", flag.name));
    }
    let mut usage = forms.join(" ");
    if flag.var {
        usage.push('…');
    }
    if let Some(arg) = &flag.arg {
        let arg_usage = arg.usage();
        if flag.require_equals && (flag.value_optional || !arg.required) {
            usage.push_str(&crate::spec::flag::optional_equals_usage(&arg_usage));
        } else {
            usage.push(if flag.require_equals { '=' } else { ' ' });
            usage.push_str(&arg_usage);
        }
    }
    usage
}

impl From<&crate::SpecFlag> for SpecFlag {
    fn from(flag: &crate::SpecFlag) -> Self {
        Self {
            name: flag.name.clone(),
            effect: flag.effect,
            verbosity: flag.verbosity,
            color: flag.color,
            usage: reference_usage(flag),
            display_usage: column_usage(flag),
            help: said(&flag.help),
            help_long: flag.help_long.clone(),
            help_md: flag.help_md.clone(),
            help_first_line: flag.help_first_line.clone(),
            short: flag
                .short
                .iter()
                .filter(|short| !flag.hidden_short_aliases.contains(short))
                .copied()
                .collect(),
            long: flag
                .long
                .iter()
                .filter(|long| !flag.hidden_aliases.contains(long))
                .cloned()
                .collect(),
            required: flag.required,
            deprecated: flag.deprecated.clone(),
            deprecated_warn_at: flag.deprecated_warn_at.clone(),
            deprecated_remove_at: flag.deprecated_remove_at.clone(),
            var: flag.var,
            var_min: flag.var_min,
            var_max: flag.var_max,
            hide: flag.hide,
            hide_default_value: flag.hide_default_value,
            hide_env: flag.hide_env,
            hide_env_values: flag.hide_env_values,
            hide_possible_values: flag.hide_possible_values,
            hide_short_help: flag.hide_short_help,
            hide_long_help: flag.hide_long_help,
            global: flag.global,
            count: flag.count,
            arg: flag.arg.as_ref().map(SpecArg::from),
            default: flag.default.clone(),
            negate: flag.negate.clone(),
            env: flag.env.clone(),
            env_fallback: flag.env_fallback.clone(),
            deprecated_env: flag.deprecated_env.clone(),
            help_heading: flag.help_heading.clone(),
            display_order: flag.display_order,
            rendered: false,
            help_rendered: None,
            help_is_multiline: false,
            usage_col_width: 0,
        }
    }
}

impl From<&crate::spec::cmd::SpecExample> for SpecExample {
    fn from(example: &crate::spec::cmd::SpecExample) -> Self {
        Self {
            code: example.code.clone(),
            header: example.header.clone(),
            help: example.help.clone(),
            lang: example.lang.clone(),
            rendered: false,
        }
    }
}

impl From<&crate::SpecArg> for SpecArg {
    fn from(arg: &crate::SpecArg) -> Self {
        Self {
            name: arg.name.clone(),
            usage: arg.usage.clone(),
            help: said(&arg.help),
            help_long: arg.help_long.clone(),
            help_md: arg.help_md.clone(),
            help_first_line: arg.help_first_line.clone(),
            required: arg.required,
            var: arg.var,
            var_min: arg.var_min,
            var_max: arg.var_max,
            hide: arg.hide,
            hide_default_value: arg.hide_default_value,
            hide_env: arg.hide_env,
            hide_env_values: arg.hide_env_values,
            hide_possible_values: arg.hide_possible_values,
            hide_short_help: arg.hide_short_help,
            hide_long_help: arg.hide_long_help,
            default: arg.default.clone(),
            choices: arg.choices.as_ref().map(|choices| choices.for_help()),
            validate: arg.validate.clone(),
            validate_error: arg.validate_error.clone(),
            env: arg.env.clone(),
            env_fallback: arg.env_fallback.clone(),
            deprecated_env: arg.deprecated_env.clone(),
            help_heading: arg.help_heading.clone(),
            display_order: arg.display_order,
            rendered: false,
            help_rendered: None,
            help_is_multiline: false,
            usage_col_width: 0,
        }
    }
}

impl Spec {
    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        if let Some(h) = &mut self.about_md {
            *h = renderer.replace_code_fences(h.to_string());
        }
        for example in &mut self.examples {
            example.render_md(renderer);
        }
        self.cmd.render_md(renderer);
        self.config.render_md(renderer);
    }
}

impl SpecCommand {
    pub fn all_subcommands(&self) -> Vec<&SpecCommand> {
        let mut cmds = vec![];
        for cmd in self.subcommands.values() {
            cmds.push(cmd);
            cmds.extend(cmd.all_subcommands());
        }
        cmds
    }

    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        if self.before_help_md.is_none() {
            if let Some(h) = self.before_help_long.clone().or(self.before_help.clone()) {
                self.before_help_md = Some(renderer.replace_code_fences(h));
            }
        }
        if self.help_md.is_none() {
            if let Some(h) = self.help_long.clone().or(self.help.clone()) {
                self.help_md = Some(renderer.replace_code_fences(h));
            }
        }
        if self.after_help_md.is_none() {
            if let Some(h) = self.after_help_long.clone().or(self.after_help.clone()) {
                self.after_help_md = Some(renderer.replace_code_fences(h));
            }
        }
        for flag in &mut self.flags {
            flag.render_md(renderer);
        }
        for arg in &mut self.args {
            arg.render_md(renderer);
        }
        for example in &mut self.examples {
            example.render_md(renderer);
        }
        // Regroup from the freshly rendered lists. The groups hold clones, so
        // grouping before this point would publish copies without their rendered
        // markdown — which is exactly what happened the first time.
        self.regroup();
        for cmd in self.subcommands.values_mut() {
            cmd.render_md(renderer);
        }
    }

    /// Rebuild the grouped views from `flags` and `args`.
    ///
    /// Anything that mutates either list has to call this, or the groups go stale.
    fn regroup(&mut self) {
        self.flag_groups = group_by_heading(&self.flags, |f| f.help_heading.as_deref());
        self.arg_groups = group_by_heading(&self.args, |a| a.help_heading.as_deref());
    }
}

impl SpecFlag {
    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        if self.help_md.is_none() {
            if let Some(h) = self.help_long.clone().or(self.help.clone()) {
                self.help_md = Some(renderer.replace_code_fences(h));
            }
        }
    }
}

impl SpecArg {
    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        if self.help_md.is_none() {
            if let Some(h) = self.help_long.clone().or(self.help.clone()) {
                self.help_md = Some(renderer.replace_code_fences(h));
            }
        }
    }
}

impl SpecExample {
    pub fn render_md(&mut self, renderer: &MarkdownRenderer) {
        if self.rendered {
            return;
        }
        self.rendered = true;
        if let Some(h) = self.help.clone() {
            self.help = Some(renderer.replace_code_fences(h));
        }
    }
}

#[cfg(test)]
mod tests {
    /// A flag whose only spelling is its negation — clap's `SetFalse`, tak's `--no-credit` —
    /// carries that spelling as its usage string already, and the flags column appended the
    /// negation a second time: `--color / --color`.
    #[test]
    fn a_flag_spelled_only_as_its_negation_is_listed_once() {
        let spec: crate::Spec = "flag \"color:\" negate=\"--color\"\n"
            .parse()
            .expect("a spec");
        assert_eq!(super::column_usage(&spec.cmd.flags[0]), "--color");
    }

    /// And a flag that has both keeps both, which is what the ` / ` is for.
    #[test]
    fn a_flag_with_a_positive_spelling_lists_its_negation_beside_it() {
        let spec: crate::Spec = "flag \"--color\" negate=\"--no-color\"\n"
            .parse()
            .expect("a spec");
        assert_eq!(
            super::column_usage(&spec.cmd.flags[0]),
            "    --color / --no-color"
        );
    }
}
