use crate::docs::models::{Spec, SpecArg, SpecCommand, SpecFlag};
use crate::error::UsageErr;
use itertools::Itertools;
use roff::{bold, italic, roman, Roff};

/// Renderer for generating Unix man pages from Usage specifications
#[derive(Debug, Clone)]
pub struct ManpageRenderer {
    spec: Spec,
    section: u8,
}

impl ManpageRenderer {
    /// Create a new manpage renderer for the given spec
    pub fn new(spec: crate::Spec) -> Self {
        Self {
            spec: spec.into(),
            section: 1,
        }
    }

    /// Set the manual section number (default: 1)
    ///
    /// Common sections:
    /// - 1: User commands
    /// - 5: File formats
    /// - 7: Miscellaneous
    /// - 8: System administration commands
    pub fn with_section(mut self, section: u8) -> Self {
        self.section = section;
        self
    }

    /// Render the complete man page
    pub fn render(&self) -> Result<String, UsageErr> {
        let mut roff = Roff::new();

        // TH (Title Header) - program name, section, date, source, manual
        let section_str = self.section.to_string();
        roff.control(
            "TH",
            [self.spec.name.to_uppercase().as_str(), section_str.as_str()],
        );

        // NAME section
        self.render_name(&mut roff);

        // SYNOPSIS section
        self.render_synopsis(&mut roff);

        // DESCRIPTION section
        self.render_description(&mut roff);

        // Render the main command
        self.render_command(&mut roff, &self.spec.cmd, true);

        // Render detailed sections for each subcommand
        self.render_subcommand_details(&mut roff, &self.spec.cmd, &self.spec.bin);

        // EXAMPLES section (spec-level)
        if !self.spec.examples.is_empty() {
            roff.control("SH", ["EXAMPLES"]);
            for (i, example) in self.spec.examples.iter().enumerate() {
                // Add spacing between examples (but not before the first one)
                if i > 0 {
                    roff.control("PP", [] as [&str; 0]);
                }
                if let Some(header) = &example.header {
                    roff.text([bold(header)]);
                }
                if let Some(help) = &example.help {
                    roff.text([roman(help.as_str())]);
                }
                roff.control("PP", [] as [&str; 0]);
                roff.control("RS", ["4"]);
                roff.text([roman(example.code.as_str())]);
                roff.control("RE", [] as [&str; 0]);
            }
        }

        // CONFIGURATION section
        self.render_configuration(&mut roff);

        if let Some(license) = &self.spec.license {
            roff.control("SH", ["LICENSE"]);
            roff.text([roman(license)]);
        }

        if let Some(repository) = &self.spec.repository {
            roff.control("SH", ["SOURCE"]);
            roff.text([roman(repository)]);
        }

        // AUTHOR section (if present)
        if let Some(author) = &self.spec.author {
            roff.control("SH", ["AUTHOR"]);
            roff.text([roman(author)]);
        }

        Ok(roff.to_roff())
    }

    /// The settings, where a man page conventionally describes them: after the commands and
    /// before the author.
    ///
    /// Deliberately terser than the markdown: a man page is read in a terminal, so each
    /// setting gets its type, its default and how to set it, and the long-form prose stays
    /// on the web page.
    fn render_configuration(&self, roff: &mut Roff) {
        let config = &self.spec.config;
        // The same predicate the markdown page uses: a block that declares only where files
        // live is worth a CONFIGURATION section, and gating on props alone meant the same
        // spec documented its file chain in one output format and not the other.
        if config.is_empty() {
            return;
        }
        roff.control("SH", ["CONFIGURATION"]);
        if !config.files.is_empty() {
            roff.text([roman("Read from the following, in ascending precedence:")]);
            roff.control("RS", ["4"]);
            for file in &config.files {
                let mut line = file.path.clone();
                if file.findup {
                    line.push_str(" (and in every parent directory)");
                }
                roff.control("PP", [] as [&str; 0]);
                roff.text([roman(line)]);
            }
            roff.control("RE", [] as [&str; 0]);
        }
        // By heading group, like the markdown page: the docs model already partitions the
        // settings so the two formats stay aligned, and walking the flat list dropped every
        // `help_heading` and interleaved headed settings with unheaded ones.
        for group in &config.prop_groups {
            if let Some(heading) = &group.heading {
                roff.control("SS", [heading.as_str()]);
            }
            for prop in &group.items {
                self.render_prop(roff, prop);
            }
        }
    }

    /// One setting: a paragraph, its help, and its facts on one line.
    fn render_prop(&self, roff: &mut Roff, prop: &crate::docs::models::SpecConfigProp) {
        {
            roff.control("PP", [] as [&str; 0]);
            roff.text([bold(&prop.key)]);
            roff.control("RS", ["4"]);
            if let Some(help) = prop.help.as_deref() {
                roff.text([roman(help)]);
            }
            let mut facts = Vec::new();
            if let Some(ty) = &prop.type_ {
                facts.push(format!("type: {ty}"));
            }
            if !prop.aliases.is_empty() {
                facts.push(format!("aliases: {}", prop.aliases.join(", ")));
            }
            if let Some(optional) = prop.optional {
                facts.push(format!("optional: {optional}"));
            }
            if let Some(default) = &prop.default {
                facts.push(format!("default: {default}"));
            }
            if !prop.sources.is_empty() {
                // The markdown's backticks would be literal here.
                let plain: Vec<String> = prop
                    .sources
                    .iter()
                    .map(|source| source.replace('`', ""))
                    .collect();
                facts.push(format!("set with: {}", plain.join(", ")));
            }
            // What the setting accepts, which for a constrained one is the fact a reader most
            // needs and the manpage did not carry at all. Values only: a choice's own help
            // belongs on the page, where there is room for it.
            if !prop.choices.is_empty() {
                let values: Vec<&str> = prop.choices.iter().map(|c| c.value.as_str()).collect();
                facts.push(format!("one of: {}", values.join(", ")));
            }
            if !facts.is_empty() {
                roff.control("PP", [] as [&str; 0]);
                roff.text([roman(facts.join("; "))]);
            }
            if let Some(deprecated) = &prop.deprecated {
                roff.control("PP", [] as [&str; 0]);
                // With the version it goes away in, as the markdown page says: a deprecation
                // notice without the date leaves the reader with nothing to plan around, and
                // the terminal is the one place this is *supposed* to surface.
                let mut notice = format!("Deprecated: {deprecated}");
                if let Some(remove_at) = &prop.deprecated_remove_at {
                    notice.push_str(&format!(" Removed in {remove_at}."));
                }
                roff.text([roman(notice)]);
            }
            roff.control("RE", [] as [&str; 0]);
        }
    }

    fn render_name(&self, roff: &mut Roff) {
        roff.control("SH", ["NAME"]);
        let description = self
            .spec
            .about
            .as_deref()
            .unwrap_or("No description available");
        roff.text([roman(format!("{} - {}", self.spec.name, description))]);
    }

    fn render_synopsis(&self, roff: &mut Roff) {
        roff.control("SH", ["SYNOPSIS"]);

        if !self.spec.usage.trim().is_empty() {
            for line in self.spec.usage.lines() {
                let line = line.trim().strip_prefix("Usage: ").unwrap_or(line.trim());
                if let Some(rest) = line.strip_prefix(&self.spec.bin) {
                    roff.text([bold(&self.spec.bin), roman(rest)]);
                } else {
                    roff.text([roman(line)]);
                }
            }
            return;
        }

        let synopsis = self.build_synopsis(&self.spec.cmd, &self.spec.bin);
        roff.text([bold(&self.spec.bin), roman(" "), roman(&synopsis)]);
    }

    fn build_synopsis(&self, cmd: &SpecCommand, _prefix: &str) -> String {
        let mut parts = Vec::new();

        // Add flags summary
        if !cmd.flags.is_empty() {
            parts.push("[OPTIONS]".to_string());
        }

        // Add arguments
        for arg in &cmd.args {
            if arg.required {
                parts.push(format!("<{}>", arg.name));
            } else {
                parts.push(format!("[<{}>]", arg.name));
            }
            if arg.var {
                parts.push("...".to_string());
            }
        }

        // Add subcommands indicator
        if !cmd.subcommands.is_empty() {
            if cmd.subcommand_required {
                parts.push("<COMMAND>".to_string());
            } else {
                parts.push("[COMMAND]".to_string());
            }
        }

        parts.join(" ")
    }

    fn render_description(&self, roff: &mut Roff) {
        roff.control("SH", ["DESCRIPTION"]);

        if let Some(about) = &self.spec.about_long.as_ref().or(self.spec.about.as_ref()) {
            // Split into paragraphs and render each
            for paragraph in about.split("\n\n") {
                roff.text([roman(paragraph.trim())]);
                roff.control("PP", [] as [&str; 0]);
            }
        }

        if let Some(help) = &self
            .spec
            .cmd
            .help_long
            .as_ref()
            .or(self.spec.cmd.help.as_ref())
        {
            for paragraph in help.split("\n\n") {
                roff.text([roman(paragraph.trim())]);
                roff.control("PP", [] as [&str; 0]);
            }
        }
        if let Some(notice) = deprecation_notice(
            self.spec.cmd.deprecated.as_deref(),
            self.spec.cmd.deprecated_warn_at.as_deref(),
            self.spec.cmd.deprecated_remove_at.as_deref(),
        ) {
            roff.text([italic(notice)]);
            roff.control("PP", [] as [&str; 0]);
        }
    }

    fn render_command(&self, roff: &mut Roff, cmd: &SpecCommand, is_root: bool) {
        // OPTIONS section. A hidden flag is one the CLI does not offer, so a manual that
        // lists it publishes a control its own `--help` withholds — the markdown reference
        // and the Fig spec have always filtered these, and this page did not.
        let visible: Vec<_> = cmd.flags.iter().filter(|flag| !flag.hide).collect();
        if !visible.is_empty() {
            roff.control("SH", ["OPTIONS"]);
            for flag in visible {
                self.render_flag(roff, flag);
            }
        }

        // ARGUMENTS section (if not root or has notable args)
        if !cmd.args.is_empty()
            && (!is_root
                || cmd
                    .args
                    .iter()
                    .any(|a| a.help.is_some() || a.help_long.is_some()))
        {
            if is_root {
                roff.control("SH", ["ARGUMENTS"]);
            }
            for arg in &cmd.args {
                self.render_arg(roff, arg);
            }
        }

        // SUBCOMMANDS section - show all subcommands recursively
        let all_subcommands = cmd.all_subcommands();
        if !all_subcommands.is_empty() {
            roff.control("SH", ["COMMANDS"]);
            self.render_all_subcommands(roff, &self.spec.cmd, "");
        }

        // EXAMPLES section
        if !cmd.examples.is_empty() {
            roff.control("SH", ["EXAMPLES"]);
            for (i, example) in cmd.examples.iter().enumerate() {
                // Add spacing between examples (but not before the first one)
                if i > 0 {
                    roff.control("PP", [] as [&str; 0]);
                }
                if let Some(header) = &example.header {
                    roff.text([bold(header)]);
                }
                if let Some(help) = &example.help {
                    roff.text([roman(help.as_str())]);
                }
                roff.control("PP", [] as [&str; 0]);
                roff.control("RS", ["4"]);
                roff.text([roman(example.code.as_str())]);
                roff.control("RE", [] as [&str; 0]);
            }
        }
    }

    fn render_flag(&self, roff: &mut Roff, flag: &SpecFlag) {
        roff.control("TP", [] as [&str; 0]);

        // Build flag usage line
        let mut flag_parts = Vec::new();

        for short in &flag.short {
            flag_parts.push(format!("-{}", short));
        }
        for long in &flag.long {
            flag_parts.push(format!("--{}", long));
        }

        let flag_usage = flag_parts.join(", ");

        if let Some(arg) = &flag.arg {
            roff.text([
                bold(&flag_usage),
                roman(" "),
                italic(format!("<{}>", arg.name)),
            ]);
        } else {
            roff.text([bold(&flag_usage)]);
        }

        // Flag help text
        if let Some(help) = &flag.help_long.as_ref().or(flag.help.as_ref()) {
            roff.text([roman(help.as_str())]);
        }
        if let Some(notice) = deprecation_notice(
            flag.deprecated.as_deref(),
            flag.deprecated_warn_at.as_deref(),
            flag.deprecated_remove_at.as_deref(),
        ) {
            roff.text([italic(notice)]);
        }

        // Default value
        if !flag.default.is_empty() {
            roff.control("RS", [] as [&str; 0]);
            let default_str = flag.default.join(", ");
            roff.text([italic("Default: "), roman(default_str.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }

        // Environment variable
        if let Some(env) = &flag.env {
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Environment: "), bold(env.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
        for env in &flag.env_fallback {
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Environment fallback: "), bold(env.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
        for env in &flag.deprecated_env {
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Deprecated environment: "), bold(env.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
    }

    fn render_arg(&self, roff: &mut Roff, arg: &SpecArg) {
        if arg.help.is_none() && arg.help_long.is_none() {
            return;
        }

        roff.control("TP", [] as [&str; 0]);
        roff.text([bold(format!("<{}>", arg.name))]);

        if let Some(help) = &arg.help_long.as_ref().or(arg.help.as_ref()) {
            roff.text([roman(help.as_str())]);
        }

        if !arg.default.is_empty() {
            roff.control("RS", [] as [&str; 0]);
            let default_str = arg.default.join(", ");
            roff.text([italic("Default: "), roman(default_str.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }

        if let Some(env) = &arg.env {
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Environment: "), bold(env.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
        for env in &arg.env_fallback {
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Environment fallback: "), bold(env.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
        for env in &arg.deprecated_env {
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Deprecated environment: "), bold(env.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
    }

    fn render_all_subcommands(&self, roff: &mut Roff, cmd: &SpecCommand, prefix: &str) {
        for (name, subcmd) in &cmd.subcommands {
            if subcmd.hide {
                continue;
            }

            let full_name = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{} {}", prefix, name)
            };

            self.render_subcommand_summary(roff, &full_name, subcmd);

            // Recursively render nested subcommands
            self.render_all_subcommands(roff, subcmd, &full_name);
        }
    }

    fn render_subcommand_details(&self, roff: &mut Roff, cmd: &SpecCommand, prefix: &str) {
        for (name, subcmd) in &cmd.subcommands {
            if subcmd.hide {
                continue;
            }

            let full_name = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{} {}", prefix, name)
            };

            // Only render detailed section if the subcommand has flags, args with help, or examples
            let has_flags = !subcmd.flags.is_empty();
            let has_documented_args = subcmd
                .args
                .iter()
                .any(|a| a.help.is_some() || a.help_long.is_some());
            let has_examples = !subcmd.examples.is_empty();

            if has_flags || has_documented_args || has_examples {
                // Section header for this subcommand
                roff.control("SH", [full_name.to_uppercase().as_str()]);

                // Description
                if let Some(help) = &subcmd.help_long.as_ref().or(subcmd.help.as_ref()) {
                    roff.text([roman(help.as_str())]);
                    roff.control("PP", [] as [&str; 0]);
                }
                if let Some(notice) = deprecation_notice(
                    subcmd.deprecated.as_deref(),
                    subcmd.deprecated_warn_at.as_deref(),
                    subcmd.deprecated_remove_at.as_deref(),
                ) {
                    roff.text([italic(notice)]);
                    roff.control("PP", [] as [&str; 0]);
                }

                // Synopsis
                let synopsis = self.build_synopsis(subcmd, &full_name);
                roff.text([
                    bold("Usage:"),
                    roman(" "),
                    roman(&full_name),
                    roman(" "),
                    roman(&synopsis),
                ]);
                roff.control("PP", [] as [&str; 0]);

                // Render flags if any, hidden ones excluded as above
                let visible: Vec<_> = subcmd.flags.iter().filter(|flag| !flag.hide).collect();
                if !visible.is_empty() {
                    roff.text([bold("Options:")]);
                    roff.control("PP", [] as [&str; 0]);
                    for flag in visible {
                        self.render_flag(roff, flag);
                    }
                }

                // Render args if any with help
                if has_documented_args {
                    roff.text([bold("Arguments:")]);
                    roff.control("PP", [] as [&str; 0]);
                    for arg in &subcmd.args {
                        self.render_arg(roff, arg);
                    }
                }

                // Render examples if any
                if has_examples {
                    roff.text([bold("Examples:")]);
                    roff.control("PP", [] as [&str; 0]);
                    for (i, example) in subcmd.examples.iter().enumerate() {
                        // Add spacing between examples (but not before the first one)
                        if i > 0 {
                            roff.control("PP", [] as [&str; 0]);
                        }
                        if let Some(header) = &example.header {
                            roff.text([bold(header)]);
                        }
                        if let Some(help) = &example.help {
                            roff.text([roman(help.as_str())]);
                        }
                        roff.control("PP", [] as [&str; 0]);
                        roff.control("RS", ["4"]);
                        roff.text([roman(example.code.as_str())]);
                        roff.control("RE", [] as [&str; 0]);
                    }
                }
            }

            // Recursively render nested subcommands
            self.render_subcommand_details(roff, subcmd, &full_name);
        }
    }

    fn render_subcommand_summary(&self, roff: &mut Roff, name: &str, cmd: &SpecCommand) {
        roff.control("TP", [] as [&str; 0]);
        roff.text([bold(name)]);

        // Prefer help_long, fall back to help
        if let Some(help) = &cmd.help_long.as_ref().or(cmd.help.as_ref()) {
            // Take just the first line for the summary
            let first_line = help.lines().next().unwrap_or("");
            roff.text([roman(first_line)]);
        }
        if let Some(notice) = deprecation_notice(
            cmd.deprecated.as_deref(),
            cmd.deprecated_warn_at.as_deref(),
            cmd.deprecated_remove_at.as_deref(),
        ) {
            roff.text([italic(notice)]);
        }

        // Show aliases if any
        if !cmd.aliases.is_empty() {
            let aliases = cmd.aliases.iter().join(", ");
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Aliases: "), roman(aliases.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
    }
}

fn deprecation_notice(
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
    Some(format!("Deprecated: {}", parts.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Spec;

    #[test]
    fn the_settings_get_a_section_of_their_own() {
        let spec: Spec = r##"
name "hk"
bin "hk"
config {
    source "git" name="git config" doc_hint="git config `{key}`"
    file "hk.toml" findup=#true
    prop "jobs" type="uint" default=4 help="Number of parallel jobs" {
        cli "--jobs" "-j"
        env "HK_JOBS"
        source "git" "hk.jobs"
    }
    prop "old" deprecated="Use jobs instead." deprecated_remove_at="2027.12.0" help="Old"
    prop "stash" type="string" help="How to stash" {
        choices {
            choice "git" help="Use `git stash`"
            choice "none" help="No stashing"
        }
    }
    prop "secret" hide=#true help="Not in the page"
}
"##
        .parse()
        .unwrap();
        let page = ManpageRenderer::new(spec).render().unwrap();

        assert!(page.contains(".SH CONFIGURATION"), "{page}");
        assert!(
            page.contains("hk.toml (and in every parent directory)"),
            "{page}"
        );
        assert!(page.contains("jobs"), "{page}");
        // Facts on one line. Hyphens arrive as `\-`, which is how roff spells them.
        assert!(
            page.contains(
                "type: uint; default: 4; set with: \\-\\-jobs, \\-j, HK_JOBS, git config hk.jobs"
            ),
            "{page}"
        );
        // With the version it goes away in, which is the part a reader can plan around.
        assert!(
            page.contains("Deprecated: Use jobs instead. Removed in 2027.12.0."),
            "{page}"
        );
        // And what a constrained setting accepts, which is the fact a reader most needs.
        assert!(page.contains("one of: git, none"), "{page}");
        assert!(
            !page.contains("secret"),
            "a hidden prop should not be here:\n{page}"
        );
        assert!(!page.contains('`'), "no backticks in a man page:\n{page}");
    }

    #[test]
    fn the_manpage_groups_settings_by_heading_like_the_page_does() {
        // The docs model already partitions settings by `help_heading` so the two formats stay
        // aligned. The manpage walked the flat list instead, dropping every heading and
        // interleaving headed settings with unheaded ones in one alphabetical run.
        let spec: Spec = r##"
name "hk"
bin "hk"
config {
    prop "jobs" type="uint" help="How many" help_heading="Performance"
    prop "cache" type="bool" help="Cache things" help_heading="Performance"
    prop "colour" type="bool" help="Colourize"
}
"##
        .parse()
        .unwrap();
        let page = ManpageRenderer::new(spec).render().unwrap();
        assert!(page.contains(".SS Performance"), "{page}");
        // The unheaded setting comes first, as the markdown page also orders it, and the two
        // headed ones sit together under the heading rather than either side of it.
        let colour = page.find("colour").expect("colour");
        let heading = page.find(".SS Performance").expect("heading");
        let jobs = page.find("jobs").expect("jobs");
        let cache = page.find("cache").expect("cache");
        assert!(colour < heading, "unheaded settings come first:\n{page}");
        assert!(heading < cache && heading < jobs, "{page}");
    }

    #[test]
    fn a_cli_with_no_settings_has_no_configuration_section() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\n".parse().unwrap();
        let page = ManpageRenderer::new(spec).render().unwrap();
        assert!(!page.contains("CONFIGURATION"), "{page}");
    }

    #[test]
    fn an_explicit_usage_renders_each_alternative_in_the_synopsis() {
        let spec: Spec = r#"
name "ex"
bin "ex"
usage "Usage: ex <COMMAND>\n       ex --print-spec"
cmd "run"
"#
        .parse()
        .unwrap();
        let page = ManpageRenderer::new(spec).render().unwrap();
        assert!(page.contains("\\fBex\\fR <COMMAND>"), "{page}");
        assert!(page.contains("\\fBex\\fR \\-\\-print\\-spec"), "{page}");
        assert!(!page.contains("[COMMAND]"), "{page}");
    }

    #[test]
    fn where_the_files_live_is_documented_even_with_nothing_to_put_in_them() {
        // A CLI can describe its config file chain before it declares a single setting —
        // usefully, since the chain is the part a reader cannot guess. Gating the section on
        // props meant this spec documented its files on the markdown page and nowhere else.
        let spec: Spec = r##"
name "ex"
bin "ex"
config {
    file "/etc/ex/config.toml" scope="system"
    file "ex.toml" findup=#true
}
"##
        .parse()
        .unwrap();
        let page = ManpageRenderer::new(spec).render().unwrap();
        assert!(page.contains(".SH CONFIGURATION"), "{page}");
        assert!(
            page.contains("ex.toml (and in every parent directory)"),
            "{page}"
        );
    }

    #[test]
    fn test_basic_manpage() {
        let spec: Spec = r#"
            name "mycli"
            bin "mycli"
            about "A sample CLI tool"

            flag "-v --verbose" help="Enable verbose output"
            flag "-o --output <file>" help="Output file path"
            arg "<input>" help="Input file to process"
        "#
        .parse()
        .unwrap();

        let renderer = ManpageRenderer::new(spec);
        let output = renderer.render().unwrap();

        println!("Generated manpage:\n{}", output);

        // Basic checks
        assert!(output.contains(".TH MYCLI 1"));
        assert!(output.contains(".SH NAME"));
        assert!(output.contains(".SH SYNOPSIS"));
        assert!(output.contains(".SH DESCRIPTION"));
        assert!(output.contains(".SH OPTIONS"));
        assert!(output.contains("verbose"));
        assert!(output.contains("output"));
    }

    #[test]
    fn package_metadata_reaches_the_manpage() {
        let spec: Spec = r#"
            name "metadata"
            bin "metadata"
            author "Example Maintainers"
            license "MIT OR Apache-2.0"
            repository "https://example.com/tool"
        "#
        .parse()
        .unwrap();
        let output = ManpageRenderer::new(spec).render().unwrap();

        assert!(output.contains(".SH LICENSE"), "{output}");
        assert!(output.contains("MIT OR Apache\\-2.0"), "{output}");
        assert!(output.contains(".SH SOURCE"), "{output}");
        assert!(output.contains("https://example.com/tool"), "{output}");
        assert!(output.contains(".SH AUTHOR"), "{output}");
    }

    #[test]
    fn test_with_custom_section() {
        let spec: Spec = r#"
            name "myconfig"
            bin "myconfig"
            about "A configuration file format"
        "#
        .parse()
        .unwrap();

        let renderer = ManpageRenderer::new(spec).with_section(5);
        let output = renderer.render().unwrap();

        assert!(output.contains(".TH MYCONFIG 5"));
    }

    #[test]
    fn test_with_subcommands() {
        let spec: Spec = r#"
            name "git"
            bin "git"
            about "The Git version control system"

            cmd "clone" help="Clone a repository"
            cmd "commit" help="Record changes to the repository"
        "#
        .parse()
        .unwrap();

        let renderer = ManpageRenderer::new(spec);
        let output = renderer.render().unwrap();

        assert!(output.contains(".SH COMMANDS"));
        assert!(output.contains("clone"));
        assert!(output.contains("commit"));
    }

    #[test]
    fn test_arguments_with_only_long_help() {
        let spec: Spec = r#"
            name "mycli"
            bin "mycli"
            about "A CLI tool"

            arg "<input>" help_long="This is a long help text for the input argument"
        "#
        .parse()
        .unwrap();

        let renderer = ManpageRenderer::new(spec);
        let output = renderer.render().unwrap();

        // Should include ARGUMENTS section even though only help_long is present
        assert!(output.contains(".SH ARGUMENTS"));
        assert!(output.contains("<input>"));
        assert!(output.contains("long help text"));
    }

    #[test]
    fn test_subcommand_with_only_long_help() {
        let spec: Spec = r#"
            name "mycli"
            bin "mycli"
            about "A CLI tool"

            cmd "deploy" help_long="This is a detailed deployment command description that should appear in the summary"
        "#
        .parse()
        .unwrap();

        let renderer = ManpageRenderer::new(spec);
        let output = renderer.render().unwrap();

        // Should use help_long for subcommand summary
        assert!(output.contains("deploy"));
        assert!(output.contains("detailed deployment command"));
    }

    #[test]
    fn test_subcommand_prefers_long_over_short_help() {
        let spec: Spec = r#"
            name "mycli"
            bin "mycli"
            about "A CLI tool"

            cmd "test" help="Short help" help_long="Long detailed help that should be preferred"
        "#
        .parse()
        .unwrap();

        let renderer = ManpageRenderer::new(spec);
        let output = renderer.render().unwrap();

        // Should prefer help_long over help
        assert!(output.contains("Long detailed help"));
    }
}
