use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::models::SpecCommand;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_cmd(&self, cmd: &crate::SpecCommand) -> Result<String, UsageErr> {
        let mut cmd = SpecCommand::from(cmd);
        // Anything that folds with the spec's CLI-wide declarations is taken from the
        // renderer's own model, where the fold already happened. Converting the raw command
        // gets only what that command declared, so a page would show a command's exit codes
        // and silently omit the ones every command has.
        if let Some(folded) = self.folded(&cmd.full_cmd) {
            cmd.outputs = folded.outputs.clone();
            cmd.exit_codes = folded.exit_codes.clone();
        }
        cmd.render_md(self);
        self.render_with("cmd_template.md.tera", |ctx| ctx.insert("cmd", &cmd))
    }

    /// The command at this path in the renderer's folded model.
    fn folded(&self, path: &[String]) -> Option<&SpecCommand> {
        let mut cmd = &self.spec.cmd;
        for name in path {
            cmd = cmd.subcommands.get(name)?;
        }
        Some(cmd)
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::{MarkdownRenderer, MarkdownTheme};
    use crate::test::SPEC_KITCHEN_SINK;
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_cmd() {
        let ctx = MarkdownRenderer::new(SPEC_KITCHEN_SINK.clone())
            .with_multi(true)
            .with_replace_pre_with_code_fences(true);
        assert_snapshot!(ctx.render_cmd(&SPEC_KITCHEN_SINK.cmd).unwrap(), @"
        # `mycli`

        - **Usage:** `mycli [FLAGS] <ARGS>… <SUBCOMMAND>`

        ## Arguments
        - **`<arg1>`** — arg1 description
        - **`[arg2]`** — arg2 description

          **Choices:** `choice1`, `choice2`, `choice3`

          **Default:** `default value`
        - **`<arg3>`** — arg3 long description
        - **`<argrest>…`**
        - **`[with-default]`**

          **Default:** `default value`

        ## Flags
        - **`--flag1`** — flag1 description
        - **`--flag2`** — flag2 long description

          includes a code block:

          ```
          $ echo hello world
          hello world

          more code
          ```

          Examples:

          ```
          # run with no arguments to use the interactive selector
          $ mise use

          # set the current version of node to 20.x in mise.toml of current directory
          # will write the fuzzy version (e.g.: 20)
          ```

          some docs

          ```
          $ echo hello world
          hello world
          ```
        - **`--flag3`** — flag3 description
        - **`--with-default`**

          **Default:** `default value`
        - **`--shell <shell>`**

          **Choices:** `bash`, `zsh`, `fish`

        ## Subcommands

        - [`mycli plugin <SUBCOMMAND>`](/plugin.md)
        ");
    }

    #[test]
    fn test_render_markdown_cmd_effect() {
        let spec: Spec = r#"
name "mise"
bin "mise"
cmd "ls" effect="read" help="List installed tools"
cmd "use" effect="write" help="Install a tool"
cmd "uninstall" effect="destructive" help="Remove a tool"
cmd "version" help="Show the version"
        "#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);
        let rendered = spec
            .cmd
            .subcommands
            .values()
            .map(|cmd| ctx.render_cmd(cmd).unwrap())
            .collect::<Vec<_>>()
            .join("\n\n");

        // Every effect value must render its own label, and a command without
        // one must not render the line at all.
        assert_snapshot!(rendered, @"
        # `mise ls`

        - **Usage:** `mise ls`
        - **Effect:** read-only

        List installed tools

        # `mise use`

        - **Usage:** `mise use`
        - **Effect:** modifies state

        Install a tool

        # `mise uninstall`

        - **Usage:** `mise uninstall`
        - **Effect:** destructive — may delete or irreversibly overwrite

        Remove a tool

        # `mise version`

        - **Usage:** `mise version`

        Show the version
        ");
    }

    #[test]
    fn clause_fields_are_documented_like_the_arguments_users_type() {
        let spec: Spec = r#"
bin "mycli"
cmd "use" {
    clause tools {
        flag "--postinstall <command>" help="Run after installation"
        arg <tool> help="Tool to install"
    }
}
        "#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);
        let rendered = ctx
            .render_cmd(spec.cmd.subcommands.get("use").unwrap())
            .unwrap();

        assert!(rendered.contains("## Arguments"), "{rendered}");
        assert!(
            rendered.contains("**`<tool>`** — Tool to install"),
            "{rendered}"
        );
        assert!(rendered.contains("## Flags"), "{rendered}");
        assert!(
            rendered.contains("**`--postinstall <command>`** — Run after installation"),
            "{rendered}"
        );
    }

    #[test]
    fn test_render_markdown_groups_by_heading() {
        let spec: Spec = r#"
bin "mycli"
flag "--verbose" help="Verbose output"
flag "--filter <pattern>" help="Only matching" help_heading="Filtering"
flag "--hidden-one" help="Not shown" help_heading="Filtering" hide=#true
arg "<file>" help="The file"
arg "<mode>" help="How to run" help_heading="Behaviour"
"#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);

        // Each heading becomes its own section, hidden entries stay out, and a
        // heading whose every entry is hidden produces no section at all.
        assert_snapshot!(ctx.render_cmd(&spec.cmd).unwrap(), @"
        # `mycli`

        - **Usage:** `mycli [--verbose] [--filter <pattern>] <file> <mode>`

        ## Arguments
        - **`<file>`** — The file

        ## Behaviour
        - **`<mode>`** — How to run

        ## Flags
        - **`--verbose`** — Verbose output

        ## Filtering
        - **`--filter <pattern>`** — Only matching
        ");
    }

    #[test]
    fn test_render_markdown_groups_global_flags_too() {
        // Global flags are rendered in their own section, which used to come from
        // the flat list and so ignored headings that help output honored.
        let spec: Spec = r#"
bin "mycli"
flag "--verbose" help="Verbose output" global=#true
flag "--filter <pattern>" help="Only matching" help_heading="Filtering" global=#true
flag "--local-one" help="Not global"
cmd "sub" help="a subcommand"
"#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);

        assert_snapshot!(ctx.render_cmd(&spec.cmd).unwrap(), @"
        # `mycli`

        - **Usage:** `mycli [FLAGS] <SUBCOMMAND>`

        ## Global Flags
        - **`--verbose`** — Verbose output

        ## Filtering
        - **`--filter <pattern>`** — Only matching

        ## Flags
        - **`--local-one`** — Not global

        ## Subcommands

        - [`mycli sub`](/sub.md)
        ");
    }

    #[test]
    fn generated_reference_separates_visible_flag_aliases() {
        let spec: Spec = r#"
bin "mycli"
flag "-t -f --tail --follow" help="Follow output"
"#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);
        let rendered = ctx.render_cmd(&spec.cmd).unwrap();
        assert!(rendered.contains("- **`-t --tail`**"), "{rendered}");
        assert!(
            rendered.contains("**Aliases:** `-f`, `--follow`"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("**`-t -f --tail --follow`**"),
            "{rendered}"
        );
    }

    #[test]
    fn test_render_markdown_cmd_outputs_and_exit_codes() {
        let spec: Spec = r#"
name "ex"
bin "ex"
exit_code 0 "success"
exit_code 130 "interrupted | terminated"
cmd "check" help="Check the project" {
    flag "--format <FMT>" help="Output format"
    output "human" default=#true help="A table"
    output "jsonl" framing="jsonl" help="One event per line"
    select "--format"
    exit_code 1 "a check failed"
}
cmd "version" help="Show the version"
        "#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);
        let rendered = spec
            .cmd
            .subcommands
            .values()
            .map(|cmd| ctx.render_cmd(cmd).unwrap())
            .collect::<Vec<_>>()
            .join("\n\n");

        // `check` shows what it writes, how to ask for it, and the CLI-wide codes folded
        // together with its own. `version` declares no outputs, so it renders no Output
        // Formats section — but the CLI-wide exit codes still reach it, because those are the
        // program's, not the command's.
        assert!(rendered.contains("## Output Formats"), "{rendered}");
        assert!(rendered.contains("- **`human`** (default)"), "{rendered}");
        assert!(
            rendered.contains("**Select:** `--format jsonl`"),
            "{rendered}"
        );
        assert!(
            rendered.contains("one document per line, read as it arrives"),
            "{rendered}"
        );
        assert!(rendered.contains("| `1` | a check failed |"), "{rendered}");
        assert!(
            rendered.contains(r"| `130` | interrupted \| terminated |"),
            "{rendered}"
        );

        let version = ctx.render_cmd(&spec.cmd.subcommands["version"]).unwrap();
        assert!(!version.contains("## Output Formats"), "{version}");
        assert!(version.contains("| `0` | success |"), "{version}");
    }

    #[test]
    fn a_command_with_no_outputs_renders_no_output_section() {
        let spec: Spec = r#"
name "ex"
bin "ex"
cmd "ls" help="List things"
        "#
        .parse()
        .unwrap();
        let ctx = MarkdownRenderer::new(spec.clone()).with_multi(true);
        let rendered = ctx.render_cmd(&spec.cmd.subcommands["ls"]).unwrap();
        assert!(!rendered.contains("## Output Formats"), "{rendered}");
        assert!(!rendered.contains("## Exit Status"), "{rendered}");
    }

    #[test]
    fn compact_output_formats_collapse_only_long_catalogs() {
        let short: Spec = r#"
name "short"
output "text"
output "json" framing="json"
        "#
        .parse()
        .unwrap();
        let short = MarkdownRenderer::new(short).render_spec().unwrap();
        assert!(short.contains("## Output Formats"), "{short}");
        assert!(!short.contains("<details>"), "{short}");

        let boundary: Spec = r#"
name "boundary"
output "text"
output "json" framing="json"
output "jsonl" framing="jsonl"
output "xml"
output "yaml"
        "#
        .parse()
        .unwrap();
        let boundary = MarkdownRenderer::new(boundary).render_spec().unwrap();
        assert!(!boundary.contains("<details>"), "{boundary}");

        let long: Spec = r#"
name "long"
output "text"
output "json" framing="json"
output "jsonl" framing="jsonl"
output "xml"
output "yaml"
output "csv"
        "#
        .parse()
        .unwrap();
        let compact = MarkdownRenderer::new(long.clone()).render_spec().unwrap();
        assert!(
            compact.contains("<summary>6 available formats</summary>"),
            "{compact}"
        );
        assert!(compact.contains("- **`csv`**"), "{compact}");

        let detailed = MarkdownRenderer::new(long)
            .with_theme(MarkdownTheme::Detailed)
            .render_spec()
            .unwrap();
        assert!(detailed.contains("## Output Formats"), "{detailed}");
        assert!(!detailed.contains("<details>"), "{detailed}");
    }
}
