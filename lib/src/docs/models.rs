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
    pub hide: bool,
    pub global: bool,
    pub count: bool,
    pub arg: Option<SpecArg>,
    pub default: Option<String>,
    pub negate: Option<String>,
    pub rendered: bool,
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
    pub default: Option<String>,
    pub choices: Option<SpecChoices>,
    pub rendered: bool,
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
            rendered: false,
        }
    }
}

impl From<&crate::SpecCommand> for SpecCommand {
    fn from(cmd: &crate::SpecCommand) -> Self {
        Self {
            full_cmd: cmd.full_cmd.clone(),
            usage: cmd.usage.clone(),
            subcommands: cmd
                .subcommands
                .iter()
                .map(|(k, v)| (k.clone(), SpecCommand::from(v)))
                .collect(),
            args: cmd.args.iter().map(SpecArg::from).collect(),
            flags: cmd.flags.iter().map(SpecFlag::from).collect(),
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
            hide: flag.hide,
            global: flag.global,
            count: flag.count,
            arg: flag.arg.as_ref().map(SpecArg::from),
            default: flag.default.clone(),
            negate: flag.negate.clone(),
            rendered: false,
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
            rendered: false,
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
