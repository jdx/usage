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

        // AUTHOR section (if present)
        if let Some(author) = &self.spec.author {
            roff.control("SH", ["AUTHOR"]);
            roff.text([roman(author)]);
        }

        Ok(roff.to_roff())
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
    }

    fn render_command(&self, roff: &mut Roff, cmd: &SpecCommand, is_root: bool) {
        // OPTIONS section
        if !cmd.flags.is_empty() {
            roff.control("SH", ["OPTIONS"]);
            for flag in &cmd.flags {
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

                // Render flags if any
                if !subcmd.flags.is_empty() {
                    roff.text([bold("Options:")]);
                    roff.control("PP", [] as [&str; 0]);
                    for flag in &subcmd.flags {
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

        // Show aliases if any
        if !cmd.aliases.is_empty() {
            let aliases = cmd.aliases.iter().join(", ");
            roff.control("RS", [] as [&str; 0]);
            roff.text([italic("Aliases: "), roman(aliases.as_str())]);
            roff.control("RE", [] as [&str; 0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Spec;

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
