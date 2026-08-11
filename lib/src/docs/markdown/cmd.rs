use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::models::SpecCommand;
use crate::error::UsageErr;

impl MarkdownRenderer {
    pub fn render_cmd(&self, cmd: &crate::SpecCommand) -> Result<String, UsageErr> {
        let mut cmd = SpecCommand::from(cmd);
        cmd.render_md(self);
        let mut ctx = self.clone();
        ctx.insert("cmd", &cmd);
        ctx.render("cmd_template.md.tera")
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use crate::test::SPEC_KITCHEN_SINK;
    use crate::Spec;
    use insta::assert_snapshot;

    #[test]
    fn test_render_markdown_cmd() {
        let ctx = MarkdownRenderer::new(SPEC_KITCHEN_SINK.clone())
            .with_multi(true)
            .with_replace_pre_with_code_fences(true);
        assert_snapshot!(ctx.render_cmd(&SPEC_KITCHEN_SINK.cmd).unwrap(), @r"
        # `mycli`

        - **Usage**: `mycli [FLAGS] <ARGS>… <SUBCOMMAND>`

        ## Arguments

        ### `<arg1>`

        arg1 description

        ### `[arg2]`

        arg2 description

        **Choices:**

        - `choice1`
        - `choice2`
        - `choice3`

        **Default:** `default value`

        ### `<arg3>`

        arg3 long description

        ### `<argrest>…`

        ### `[with-default]`

        **Default:** `default value`

        ## Flags

        ### `--flag1`

        flag1 description

        ### `--flag2`

        flag2 long description

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

        ### `--flag3`

        flag3 description

        ### `--with-default`

        **Default:** `default value`

        ### `--shell <shell>`

        **Choices:**

        - `bash`
        - `zsh`
        - `fish`

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
        assert_snapshot!(rendered, @r"
        # `mise ls`

        - **Usage**: `mise ls`
        - **Effect**: read-only

        List installed tools

        # `mise use`

        - **Usage**: `mise use`
        - **Effect**: modifies state

        Install a tool

        # `mise uninstall`

        - **Usage**: `mise uninstall`
        - **Effect**: destructive — may delete or irreversibly overwrite

        Remove a tool

        # `mise version`

        - **Usage**: `mise version`

        Show the version
        ");
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

        - **Usage**: `mycli [--verbose] [--filter <pattern>] <file> <mode>`

        ## Arguments

        ### `<file>`

        The file

        ## Behaviour

        ### `<mode>`

        How to run

        ## Flags

        ### `--verbose`

        Verbose output

        ## Filtering

        ### `--filter <pattern>`

        Only matching
        ");
    }
}
