use crate::docs::markdown::MarkdownRenderer;
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
    pub author: Option<String>,
    pub about: Option<String>,
    pub about_long: Option<String>,
    pub about_md: Option<String>,
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
    // pub mounts: Vec<SpecMount>,
    pub deprecated: Option<String>,
    pub hide: bool,
    pub subcommand_required: bool,
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
    pub usage: String,
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
    pub rendered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_rendered: Option<String>,
    pub help_is_multiline: bool,
    pub usage_col_width: usize,
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
            about: spec.about,
            about_long: spec.about_long,
            about_md: spec.about_md,
            author: spec.author,
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
        let flags_usage_col_width = max_usage_width(cmd.flags.iter().map(|f| f.usage.as_str()));
        let flags: Vec<SpecFlag> = cmd
            .flags
            .iter()
            .map(|flag| {
                let mut spec_flag = SpecFlag::from(flag);

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

        Self {
            full_cmd: cmd.full_cmd.clone(),
            usage: cmd.usage.clone(),
            subcommands: cmd
                .subcommands
                .iter()
                .map(|(k, v)| (k.clone(), SpecCommand::from(v)))
                .collect(),
            args,
            flags,
            // mounts: cmd.mounts.iter().map(SpecMount::from).collect(),
            deprecated: cmd.deprecated.clone(),
            hide: cmd.hide,
            subcommand_required: cmd.subcommand_required,
            help: cmd.help.clone(),
            help_long: cmd.help_long.clone(),
            help_md: cmd.help_md.clone(),
            name: cmd.name.clone(),
            aliases: cmd.aliases.clone(),
            hidden_aliases: cmd.hidden_aliases.clone(),
            before_help: cmd.before_help.clone(),
            before_help_long: cmd.before_help_long.clone(),
            before_help_md: cmd.before_help_md.clone(),
            after_help: cmd.after_help.clone(),
            after_help_long: cmd.after_help_long.clone(),
            after_help_md: cmd.after_help_md.clone(),
            examples: cmd.examples.iter().map(SpecExample::from).collect(),
            // complete: cmd.complete.clone(),
            rendered: false,
        }
    }
}

impl From<&crate::SpecFlag> for SpecFlag {
    fn from(flag: &crate::SpecFlag) -> Self {
        Self {
            name: flag.name.clone(),
            usage: flag.usage.clone(),
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
        for cmd in self.subcommands.values_mut() {
            cmd.render_md(renderer);
        }
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
