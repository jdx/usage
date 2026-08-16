use crate::{Spec, SpecCommand};
use std::sync::LazyLock;
use tera::Tera;

pub fn render_help(spec: &Spec, cmd: &SpecCommand, long: bool) -> String {
    // Convert to docs models to get layout calculations
    let docs_spec = crate::docs::models::Spec::from(spec.clone());
    let docs_cmd = crate::docs::models::SpecCommand::from(&without_hidden(cmd));

    let mut ctx = tera::Context::new();
    ctx.insert("spec", &docs_spec);
    ctx.insert("cmd", &docs_cmd);
    ctx.insert("long", &long);
    // Which page this is. The banner and the program's own description belong to the
    // program's page; a subcommand's page describes the subcommand, which is the question
    // that was asked. `full_cmd` is the path a user would type, so the root's is empty.
    ctx.insert("root", &docs_cmd.full_cmd.is_empty());
    let template = if long {
        "spec_template_long.tera"
    } else {
        "spec_template_short.tera"
    };
    TERA.render(template, &ctx).unwrap().trim().to_string() + "\n"
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
fn without_hidden(cmd: &SpecCommand) -> SpecCommand {
    let mut visible = cmd.clone();
    visible.flags.retain(|flag| !flag.hide);
    visible.args.retain(|arg| !arg.hide);
    visible.subcommands.retain(|_, sub| !sub.hide);
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
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [FLAGS]

        Flags:
              --color    Enable color output
            [env: MYCLI_COLOR]
              --verbose  Verbose output
            [env: MYCLI_VERBOSE]
              --debug    Debug mode
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
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @r"
        Usage: testcli <ARGS>…

        Arguments:
          <input>    Input file
            [env: MY_INPUT]
          <output>   Output file
            [env: MY_OUTPUT]
          <extra>    Extra arg without env
          [default]  Arg with default value
            (default: default value)
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
              --compress / --no-compress  Compress output
              --verbose                   Verbose output
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [--compress] [--verbose]

        Flags:
              --compress / --no-compress  Compress output
              --verbose                   Verbose output
        ");
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

        short after
        ");

        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        This is the long version of before help

        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output

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
        ");

        // Long help should show author/license at the bottom
        assert_snapshot!(render_help(&spec, &spec.cmd, true), @"
        Usage: testcli [--verbose]

        Flags:
              --verbose  Enable verbose output

        Author: Test Author
        License: MIT
        ");
    }

    #[test]
    fn test_render_help_with_deprecated_command() {
        let spec = crate::spec! { r#"
bin "testcli"
cmd "old-cmd" help="Do something" deprecated="use new-cmd instead"
cmd "new-cmd" help="Do something better"
        "# }
        .unwrap();

        assert_snapshot!(render_help(&spec, &spec.cmd, false), @r"
        Usage: testcli <SUBCOMMAND>

        Commands:
          new-cmd  Do something better
          old-cmd [deprecated: use new-cmd instead]  Do something
          help  Print this message or the help of the given subcommand(s)
        ");
    }
}
