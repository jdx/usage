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
    // pub config: SpecConfig,
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
    pub args: Vec<SpecArg>,
    pub flags: Vec<SpecFlag>,
    /// `flags`, partitioned by `help_heading`. Same flags, same order.
    pub flag_groups: Vec<Group<SpecFlag>>,
    /// `args`, partitioned by `help_heading`.
    pub arg_groups: Vec<Group<SpecArg>>,
    // pub mounts: Vec<SpecMount>,
    pub deprecated: Option<String>,
    pub effect: Option<SpecCommandEffect>,
    pub hide: bool,
    pub subcommand_required: bool,
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

#[derive(Debug, Default, Clone, Serialize)]
pub struct SpecFlag {
    pub name: String,
    pub effect: Option<crate::spec::effect::SpecCommandEffect>,
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
    pub var: bool,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    pub hide: bool,
    pub global: bool,
    pub count: bool,
    pub arg: Option<SpecArg>,
    pub default: Vec<String>,
    pub negate: Option<String>,
    pub env: Option<String>,
    pub help_heading: Option<String>,
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
    pub default: Vec<String>,
    pub choices: Option<SpecChoices>,
    pub env: Option<String>,
    pub help_heading: Option<String>,
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
            // config: SpecConfig::from(&spec.config),
            version: spec.version,
            usage: spec.usage,
            // complete: spec.complete,
            source_code_link_template: spec.source_code_link_template,
            repository: spec.repository,
            about: spec.about,
            about_long: spec.about_long,
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
        use crate::docs::layout::{get_terminal_width, max_usage_width, render_help_text};

        let terminal_width = get_terminal_width();

        // Calculate layout for args
        let args_usage_col_width = max_usage_width(cmd.args.iter().map(|a| a.usage.as_str()));
        let args: Vec<SpecArg> = cmd
            .args
            .iter()
            .map(|arg| {
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
                spec_arg
            })
            .collect();

        // Calculate layout for flags
        let flags: Vec<SpecFlag> = cmd.flags.iter().map(SpecFlag::from).collect();
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
            effect,
            hide,
            subcommand_required,
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
            // Rendered above, or deliberately absent from the docs model.
            args: _,
            flags: _,
            mounts: _,
            complete: _,
            mounted: _,
            flags_from_mount: _,
            subcommand_lookup: _,
        } = cmd;

        Self {
            full_cmd: full_cmd.clone(),
            usage: usage.clone(),
            subcommands: subcommands
                .iter()
                .map(|(k, v)| (k.clone(), SpecCommand::from(v)))
                .collect(),
            flag_groups: group_by_heading(&flags, |f| f.help_heading.as_deref()),
            arg_groups: group_by_heading(&args, |a| a.help_heading.as_deref()),
            args,
            flags,
            deprecated: deprecated.clone(),
            effect: *effect,
            hide: *hide,
            subcommand_required: *subcommand_required,
            restart_token: restart_token.clone(),
            help: help.clone(),
            help_long: help_long.clone(),
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

impl From<&crate::SpecFlag> for SpecFlag {
    fn from(flag: &crate::SpecFlag) -> Self {
        Self {
            name: flag.name.clone(),
            effect: flag.effect,
            usage: flag.usage.clone(),
            display_usage: flag.negate.as_ref().map_or_else(
                || flag.usage.trim().to_string(),
                |negate| format!("{} / {}", flag.usage.trim(), negate.trim()),
            ),
            help: flag.help.clone(),
            help_long: flag.help_long.clone(),
            help_md: flag.help_md.clone(),
            help_first_line: flag.help_first_line.clone(),
            short: flag.short.clone(),
            long: flag.long.clone(),
            required: flag.required,
            deprecated: flag.deprecated.clone(),
            var: flag.var,
            var_min: flag.var_min,
            var_max: flag.var_max,
            hide: flag.hide,
            global: flag.global,
            count: flag.count,
            arg: flag.arg.as_ref().map(SpecArg::from),
            default: flag.default.clone(),
            negate: flag.negate.clone(),
            env: flag.env.clone(),
            help_heading: flag.help_heading.clone(),
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
            help: arg.help.clone(),
            help_long: arg.help_long.clone(),
            help_md: arg.help_md.clone(),
            help_first_line: arg.help_first_line.clone(),
            required: arg.required,
            var: arg.var,
            var_min: arg.var_min,
            var_max: arg.var_max,
            hide: arg.hide,
            default: arg.default.clone(),
            choices: arg.choices.clone(),
            env: arg.env.clone(),
            help_heading: arg.help_heading.clone(),
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
