//! Checks that what usage-argv emits is a spec usage-lib understands.
//!
//! The emitted KDL is the interface to everything downstream — `usage g
//! markdown`, manpages, the SDK generators, completions — so "it looks right" is
//! not a standard. Here the output is parsed back by usage-lib and the resulting
//! [`Spec`] is compared against what was declared, field by field.
//!
//! The fixture is deliberately awkward: text needing escapes, a hidden alias, a
//! negated flag, choices on both a flag and an argument, every `double_dash` mode,
//! an effect, a mount, a restart token, and two levels of subcommand. Anything the
//! writer quotes wrongly shows up as a parse failure or a changed value rather
//! than as a diff nobody reads.

use usage::Spec as LibSpec;
use usage_argv::policy::{ColorRole, VerbosityRole};
use usage_argv::spec::{ArgMeta, CommandMeta, Effect, Example, FlagMeta, Spec};
use usage_argv::{Arg, Command, DoubleDash, Flag};

static JOBS: Flag = Flag {
    key: 1,
    name: "jobs",
    longs: &["jobs"],
    shorts: b"j",
    global: true,
    ..Flag::VALUE
};
static COLOR: Flag = Flag {
    key: 2,
    name: "color",
    longs: &["color"],
    negate: Some("no-color"),
    ..Flag::BOOL
};
static VERBOSE: Flag = Flag {
    key: 3,
    name: "verbose",
    longs: &["verbose"],
    shorts: b"v",
    ..Flag::BOOL
};
static INCLUDE: Flag = Flag {
    key: 4,
    name: "include",
    longs: &["include"],
    variadic: true,
    ..Flag::VALUE
};
static SHELL: Flag = Flag {
    key: 5,
    name: "shell",
    longs: &["shell"],
    ..Flag::VALUE
};
static FORCE: Flag = Flag {
    key: 6,
    name: "force",
    longs: &["force"],
    shorts: b"f",
    ..Flag::BOOL
};
static PRUNE: Flag = Flag {
    key: 7,
    name: "prune",
    longs: &["prune"],
    ..Flag::BOOL
};
static PATHS: Flag = Flag {
    key: 8,
    name: "paths",
    longs: &["paths"],
    ..Flag::VALUE
};

static FILE: Arg = Arg {
    key: 10,
    name: "file",
    ..Arg::REQUIRED
};
static MODE: Arg = Arg {
    key: 11,
    name: "mode",
    ..Arg::REQUIRED
};
static PASSTHROUGH: Arg = Arg {
    key: 12,
    name: "cmd",
    double_dash: DoubleDash::Required,
    ..Arg::VAR
};
static TOOL: Arg = Arg {
    key: 13,
    name: "tool",
    ..Arg::REQUIRED
};
static TASK_ARGS: Arg = Arg {
    key: 14,
    name: "args",
    double_dash: DoubleDash::Preserve,
    ..Arg::VAR
};
static FILES: Arg = Arg {
    key: 15,
    name: "files",
    double_dash: DoubleDash::Automatic,
    ..Arg::VAR
};

static SET: Command = Command {
    name: "set",
    args: &[&MODE],
    key: 100,
    ..Command::EMPTY
};
static SETTINGS: Command = Command {
    name: "settings",
    subcommands: &[&SET],
    key: 101,
    ..Command::EMPTY
};
static INSTALL: Command = Command {
    name: "install",
    aliases: &["i", "add"],
    flags: &[&FORCE],
    args: &[&TOOL],
    key: 102,
    ..Command::EMPTY
};
static RUN: Command = Command {
    name: "run",
    args: &[&TASK_ARGS],
    key: 103,
    ..Command::EMPTY
};
static EXEC: Command = Command {
    name: "exec",
    args: &[&PASSTHROUGH],
    key: 104,
    ..Command::EMPTY
};
static WATCH: Command = Command {
    name: "watch",
    args: &[&FILES],
    key: 105,
    ..Command::EMPTY
};
static ROOT: Command = Command {
    name: "ex",
    flags: &[&JOBS, &COLOR, &VERBOSE, &INCLUDE, &SHELL, &PRUNE, &PATHS],
    args: &[&FILE],
    subcommands: &[&INSTALL, &SETTINGS, &RUN, &EXEC, &WATCH],
    key: 106,
    ..Command::EMPTY
};

static SET_META: CommandMeta = CommandMeta {
    cmd: &SET,
    about: Some("set a value"),
    args: &[ArgMeta {
        arg: &MODE,
        choices: &["on", "off"],
        ..ArgMeta::EMPTY
    }],
    ..CommandMeta::EMPTY
};
static SETTINGS_META: CommandMeta = CommandMeta {
    cmd: &SETTINGS,
    about: Some("manage settings"),
    subcommands: &[&SET_META],
    ..CommandMeta::EMPTY
};
static INSTALL_META: CommandMeta = CommandMeta {
    cmd: &INSTALL,
    about: Some("install a tool"),
    long_about: Some("Installs a tool.\n\nTakes a while."),
    // `i` stays visible; `add` works but is not advertised.
    hidden_aliases: &["add"],
    effect: Some(Effect::Write),
    examples: &[Example {
        code: "ex install node@20",
        header: None,
        help: Some("install a specific version"),
    }],
    flags: &[FlagMeta {
        flag: &FORCE,
        help: Some("overwrite an existing install"),
        ..FlagMeta::EMPTY
    }],
    args: &[ArgMeta {
        arg: &TOOL,
        help: Some("the tool to install"),
        ..ArgMeta::EMPTY
    }],
    ..CommandMeta::EMPTY
};
static RUN_META: CommandMeta = CommandMeta {
    cmd: &RUN,
    about: Some("run a task"),
    mount: Some("ex tasks --usage"),
    restart_token: Some(":::"),
    args: &[ArgMeta {
        arg: &TASK_ARGS,
        required: false,
        ..ArgMeta::EMPTY
    }],
    ..CommandMeta::EMPTY
};
static EXEC_META: CommandMeta = CommandMeta {
    cmd: &EXEC,
    about: Some("run a command"),
    effect: Some(Effect::Destructive),
    args: &[ArgMeta {
        arg: &PASSTHROUGH,
        ..ArgMeta::EMPTY
    }],
    ..CommandMeta::EMPTY
};
static WATCH_META: CommandMeta = CommandMeta {
    cmd: &WATCH,
    about: Some("watch files"),
    hide: true,
    args: &[ArgMeta {
        arg: &FILES,
        required: false,
        ..ArgMeta::EMPTY
    }],
    ..CommandMeta::EMPTY
};
static ROOT_META: CommandMeta = CommandMeta {
    cmd: &ROOT,
    // The root's own examples live at the top level of the document rather than
    // inside a `cmd` block, which is easy to forget when writing it out.
    examples: &[Example {
        code: "ex a.txt",
        header: Some("Basic"),
        help: Some("the simplest thing"),
    }],
    flags: &[
        FlagMeta {
            flag: &JOBS,
            help: Some(r#"how many jobs, and a quote: ""#),
            long_help: Some("More about jobs.\nOn two lines."),
            value_name: Some("n"),
            env: Some("EX_JOBS"),
            default: &["4"],
            help_heading: Some("Performance"),
            ..FlagMeta::EMPTY
        },
        FlagMeta {
            flag: &COLOR,
            help: Some("colorize output"),
            default: &["true"],
            // A negatable switch that says which way it means: `--no-color` is the
            // other answer rather than a second flag.
            color: Some(ColorRole::Always),
            ..FlagMeta::EMPTY
        },
        FlagMeta {
            flag: &VERBOSE,
            count: true,
            hide: true,
            verbosity: Some(VerbosityRole::Verbose),
            ..FlagMeta::EMPTY
        },
        FlagMeta {
            flag: &INCLUDE,
            help: Some("patterns to include"),
            value_name: Some("pattern"),
            repeatable: true,
            var_min: Some(1),
            var_max: Some(5),
            overrides: &["--exclude"],
            // One target, which the writer puts on the node as a property.
            required_if: &["--verbose"],
            ..FlagMeta::EMPTY
        },
        FlagMeta {
            flag: &SHELL,
            required: true,
            // Two of them, which cannot be written as repeated properties.
            required_unless: &["--jobs", "--color"],
            choices: &["bash", "zsh", "fish"],
            ..FlagMeta::EMPTY
        },
        // A flag that destroys something: the effect belongs on the flag, not
        // only on the command.
        FlagMeta {
            flag: &PRUNE,
            help: Some("delete anything unused"),
            // A control character, which KDL will not take literally. Help text
            // really does contain these: ANSI-colored help has an escape in it.
            long_help: Some("Deletes things.\u{1b}[0m Carefully."),
            effect: Some(Effect::Destructive),
            overrides: &["--keep", "--dry-run"],
            conflicts: &["--force"],
            ..FlagMeta::EMPTY
        },
        // More than one default, which cannot be written as a property. Neither can
        // more than one conflict or condition, so they go in the same child block.
        FlagMeta {
            flag: &PATHS,
            value_name: Some("path"),
            default: &["/usr/bin", "/usr/local/bin"],
            conflicts: &["--include", "--prune"],
            required_if: &["--force", "--prune"],
            ..FlagMeta::EMPTY
        },
    ],
    args: &[ArgMeta {
        arg: &FILE,
        help: Some("the file"),
        env: Some("EX_FILE"),
        default: &["a.txt"],
        required: false,
        help_heading: Some("Input"),
        ..ArgMeta::EMPTY
    }],
    subcommands: &[
        &INSTALL_META,
        &SETTINGS_META,
        &RUN_META,
        &EXEC_META,
        &WATCH_META,
    ],
    ..CommandMeta::EMPTY
};
static SPEC: Spec = Spec {
    name: "ex",
    bin: Some("ex"),
    version: Some("1.2.3"),
    long_version: Some("1.2.3\ncommit abc123"),
    author: None,
    license: None,
    repository: None,
    // Multi-line and full of quotes, because that is what a real one looks like and the
    // writer has to escape it into a single KDL string rather than a `#"""` block.
    source_code_link_template: Some(concat!(
        "{%- if cmd.subcommands | length > 0 -%}\n",
        "{%- set path = path ~ \"/mod.rs\" -%}\n",
        "{%- endif -%}\n",
        "https://example.com/blob/main/src/{{path}}",
    )),
    min_usage_version: None,
    about: Some("does things"),
    long_about: Some("Does things, at length."),
    usage: Some("Usage: ex <COMMAND>\n       ex --version \"quoted\""),
    default_subcommand: Some("run"),
    multicall: false,
    views: &[],
    root: &ROOT_META,
};

fn parsed() -> LibSpec {
    let kdl = SPEC.to_kdl();
    kdl.parse()
        .unwrap_or_else(|e| panic!("usage-lib could not parse the emitted spec: {e}\n\n{kdl}"))
}

#[test]
fn the_program_itself_survives() {
    let spec = parsed();
    assert_eq!(spec.name, "ex");
    assert_eq!(spec.bin, "ex");
    assert_eq!(spec.version.as_deref(), Some("1.2.3"));
    assert_eq!(spec.about.as_deref(), Some("does things"));
    assert_eq!(spec.about_long.as_deref(), Some("Does things, at length."));
    assert_eq!(spec.default_subcommand.as_deref(), Some("run"));
    assert!(!spec.multicall);
    assert_eq!(
        spec.usage,
        "Usage: ex <COMMAND>\n       ex --version \"quoted\""
    );
    let template = spec
        .source_code_link_template
        .as_deref()
        .expect("the link template survived");
    assert_eq!(template.lines().count(), 4, "{template:?}");
    assert!(
        template.contains("{%- set path = path ~ \"/mod.rs\" -%}"),
        "{template:?}"
    );
}

#[test]
fn flags_keep_their_forms_and_metadata() {
    let spec = parsed();
    let jobs = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "jobs")
        .expect("--jobs should be in the spec");

    assert_eq!(jobs.long, vec!["jobs".to_string()]);
    assert_eq!(jobs.short, vec!['j']);
    assert!(jobs.global);
    assert_eq!(jobs.env.as_deref(), Some("EX_JOBS"));
    assert_eq!(jobs.default, vec!["4".to_string()]);
    assert!(jobs.arg.is_some(), "--jobs takes a value");
    // The help text contains a quote, which is the point of including it.
    assert_eq!(
        jobs.help.as_deref(),
        Some(r#"how many jobs, and a quote: ""#)
    );
    assert_eq!(
        jobs.help_long.as_deref(),
        Some("More about jobs.\nOn two lines.")
    );
}

#[test]
fn a_negated_flag_keeps_its_dashes() {
    let spec = parsed();
    let color = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "color")
        .expect("--color should be in the spec");
    // The table stores the bare name because that is what a token is matched
    // against; the spec wants it written with dashes.
    assert_eq!(color.negate.as_deref(), Some("--no-color"));
}

#[test]
fn counted_and_hidden_flags_are_marked() {
    let spec = parsed();
    let verbose = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "verbose")
        .expect("--verbose should be in the spec");
    assert!(verbose.count);
    assert!(verbose.hide);
}

#[test]
fn variadic_bounds_and_overrides_survive() {
    let spec = parsed();
    let include = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "include")
        .expect("--include should be in the spec");
    assert!(include.var);
    assert_eq!(include.var_min, Some(1));
    assert_eq!(include.var_max, Some(5));
    assert_eq!(include.overrides, vec!["--exclude".to_string()]);
    assert_eq!(include.required_if, vec!["--verbose".to_string()]);
}

#[test]
fn conflicts_and_conditions_survive_in_both_spellings() {
    // One target is a property on the node and several are a child block, so each
    // list has to be read back in the form the writer chose for it.
    let spec = parsed();
    let flag = |name: &str| {
        spec.cmd
            .flags
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("--{name} should be in the spec"))
            .clone()
    };

    assert_eq!(flag("prune").conflicts, vec!["--force".to_string()]);
    assert_eq!(
        flag("paths").conflicts,
        vec!["--include".to_string(), "--prune".to_string()]
    );
    assert_eq!(
        flag("paths").required_if,
        vec!["--force".to_string(), "--prune".to_string()]
    );
}

#[test]
fn choices_survive_on_both_flags_and_args() {
    let spec = parsed();
    let shell = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "shell")
        .expect("--shell should be in the spec");
    let choices = shell
        .arg
        .as_ref()
        .and_then(|a| a.choices.as_ref())
        .expect("--shell should declare choices");
    assert_eq!(choices.choices, vec!["bash", "zsh", "fish"]);
    assert!(shell.required);
    assert_eq!(
        shell.required_unless,
        vec!["--jobs".to_string(), "--color".to_string()]
    );

    let set = spec
        .cmd
        .subcommands
        .get("settings")
        .and_then(|s| s.subcommands.get("set"))
        .expect("settings set should be in the spec");
    let mode_choices = set.args[0]
        .choices
        .as_ref()
        .expect("<mode> should declare choices");
    assert_eq!(mode_choices.choices, vec!["on", "off"]);
}

#[test]
fn positionals_keep_arity_optionality_and_fallbacks() {
    let spec = parsed();
    let file = &spec.cmd.args[0];
    assert_eq!(file.name, "file");
    assert!(!file.required, "a defaulted argument is optional");
    assert_eq!(file.env.as_deref(), Some("EX_FILE"));
    assert_eq!(file.default, vec!["a.txt".to_string()]);
    assert_eq!(file.help.as_deref(), Some("the file"));
}

#[test]
fn every_double_dash_mode_survives() {
    use usage::SpecDoubleDashChoices as Mode;
    let spec = parsed();
    let mode_of = |cmd: &str| {
        spec.cmd
            .subcommands
            .get(cmd)
            .unwrap_or_else(|| panic!("{cmd} should be in the spec"))
            .args[0]
            .double_dash
            .clone()
    };
    assert!(matches!(mode_of("exec"), Mode::Required));
    assert!(matches!(mode_of("run"), Mode::Preserve));
    assert!(matches!(mode_of("watch"), Mode::Automatic));
    assert!(matches!(
        spec.cmd.args[0].double_dash.clone(),
        Mode::Optional
    ));
}

#[test]
fn subcommands_nest_and_keep_visible_and_hidden_aliases() {
    let spec = parsed();
    let install = spec
        .cmd
        .subcommands
        .get("install")
        .expect("install should be in the spec");
    assert_eq!(install.aliases, vec!["i".to_string()]);
    assert_eq!(install.hidden_aliases, vec!["add".to_string()]);
    assert_eq!(install.help.as_deref(), Some("install a tool"));
    assert_eq!(
        install.help_long.as_deref(),
        Some("Installs a tool.\n\nTakes a while.")
    );

    // Two levels down, reached through the parent.
    let set = spec
        .cmd
        .subcommands
        .get("settings")
        .and_then(|s| s.subcommands.get("set"))
        .expect("settings set should be in the spec");
    assert_eq!(set.help.as_deref(), Some("set a value"));

    let watch = spec
        .cmd
        .subcommands
        .get("watch")
        .expect("watch should be in the spec");
    assert!(watch.hide);
}

#[test]
fn effects_mounts_restart_tokens_and_examples_survive() {
    use usage::SpecCommandEffect as Eff;
    let spec = parsed();

    let install = spec.cmd.subcommands.get("install").unwrap();
    assert!(matches!(install.effect, Some(Eff::Write)));
    assert_eq!(install.examples.len(), 1);
    assert_eq!(install.examples[0].code, "ex install node@20");

    let exec = spec.cmd.subcommands.get("exec").unwrap();
    assert!(matches!(exec.effect, Some(Eff::Destructive)));

    let run = spec.cmd.subcommands.get("run").unwrap();
    assert_eq!(run.restart_token.as_deref(), Some(":::"));
    assert_eq!(run.mounts.len(), 1, "run should declare a mount");
}

#[test]
fn a_help_heading_survives() {
    let spec = parsed();
    let jobs = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "jobs")
        .expect("--jobs should be in the spec");
    assert_eq!(jobs.help_heading.as_deref(), Some("Performance"));

    // Positionals can be grouped too, since the spec field is on both.
    assert_eq!(spec.cmd.args[0].help_heading.as_deref(), Some("Input"));
}

#[test]
fn a_flag_can_carry_an_effect() {
    use usage::SpecCommandEffect as Eff;
    let spec = parsed();
    let prune = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "prune")
        .expect("--prune should be in the spec");
    assert!(matches!(prune.effect, Some(Eff::Destructive)));
}

#[test]
fn a_flag_can_say_what_it_means_for_verbosity_and_color() {
    let spec = parsed();
    let flag = |name: &str| {
        spec.cmd
            .flags
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("--{name} should be in the spec"))
    };
    assert_eq!(
        flag("verbose").verbosity,
        Some(usage::SpecVerbosityRole::Verbose)
    );
    assert_eq!(flag("color").color, Some(usage::SpecColorRole::Always));
    // And a flag that says nothing about either stays silent about both.
    assert_eq!(flag("jobs").verbosity, None);
    assert_eq!(flag("jobs").color, None);
}

#[test]
fn several_defaults_all_survive() {
    // KDL properties are unique per node, so writing these as `default="a"
    // default="b"` would keep only the last. They need a child block.
    let spec = parsed();
    let paths = spec
        .cmd
        .flags
        .iter()
        .find(|f| f.name == "paths")
        .expect("--paths should be in the spec");
    assert_eq!(
        paths.default,
        vec!["/usr/bin".to_string(), "/usr/local/bin".to_string()]
    );
}

#[test]
fn root_level_examples_survive() {
    let spec = parsed();
    // A top-level `example` node lands on the spec itself rather than on its root
    // command, which is worth pinning: writing it into the root's `cmd` block
    // instead would put it somewhere nothing reads.
    assert_eq!(
        spec.examples.len(),
        1,
        "the root's examples belong at the top level of the document"
    );
    assert_eq!(spec.examples[0].code, "ex a.txt");
    assert_eq!(spec.examples[0].header.as_deref(), Some("Basic"));
}

#[test]
fn the_emitted_spec_is_stable() {
    // A snapshot of the text itself, so a change to the writer has to be looked
    // at rather than only inferred from the assertions above passing.
    insta::assert_snapshot!(SPEC.to_kdl());
}

#[test]
fn usage_lib_can_reserialize_what_we_emit() {
    // A weaker claim than it looks, and worth being honest about: this shows
    // usage-lib's serializer is a fixed point on our input. It cannot catch a
    // field we never wrote, because the field would be missing from both sides.
    // Loss is caught by the field-by-field assertions above and by the counts
    // below.
    let once = parsed();
    let twice: LibSpec = once
        .to_string()
        .parse()
        .expect("usage-lib should reparse its own output");
    assert_eq!(once.to_string(), twice.to_string());
}

#[test]
fn nothing_is_dropped_on_the_way_out() {
    // Counts, so an entry the writer skips entirely shows up here rather than in
    // whichever assertion happened to name it.
    let spec = parsed();
    assert_eq!(
        spec.cmd.flags.len(),
        ROOT.flags.len(),
        "every declared flag should reach the spec"
    );
    assert_eq!(
        spec.cmd.args.len(),
        ROOT.args.len(),
        "every declared argument should reach the spec"
    );
    assert_eq!(
        spec.cmd.subcommands.len(),
        ROOT.subcommands.len(),
        "every declared subcommand should reach the spec"
    );

    // And one level down, since nesting is where a writer tends to lose things.
    let settings = spec.cmd.subcommands.get("settings").unwrap();
    assert_eq!(settings.subcommands.len(), 1);

    let shell = spec.cmd.flags.iter().find(|f| f.name == "shell").unwrap();
    assert_eq!(
        shell.required_unless,
        vec!["--jobs".to_string(), "--color".to_string()],
        "several values need a child node, or all but the last are lost"
    );
    let prune = spec.cmd.flags.iter().find(|f| f.name == "prune").unwrap();
    assert_eq!(
        prune.overrides,
        vec!["--keep".to_string(), "--dry-run".to_string()]
    );
    assert_eq!(
        prune.help_long.as_deref(),
        Some("Deletes things.\u{1b}[0m Carefully."),
        "a control character has to survive being escaped and read back"
    );
}

#[test]
fn the_docs_pipeline_accepts_it() {
    // The end the spec exists for: an adopter runs `usage g markdown` and
    // `usage g manpage` over this text at build time. Rendering through
    // usage-lib's own generators is the same code those commands use, so a spec
    // that parses but renders to nothing fails here rather than in someone's docs
    // build.
    let spec = parsed();

    let markdown = usage::docs::markdown::MarkdownRenderer::new(spec.clone())
        .with_multi(true)
        .render_index()
        .expect("the emitted spec should render as markdown");
    assert!(markdown.contains("install"), "subcommands should be listed");
    assert!(
        !markdown.contains("watch"),
        "a hidden command should stay out of generated docs"
    );

    let manpage = usage::docs::manpage::ManpageRenderer::new(spec)
        .render()
        .expect("the emitted spec should render as a manpage");
    assert!(
        manpage.contains("ex"),
        "the manpage should name the program"
    );
}

/// What answers for a value the spec says a command runs for.
fn tools(
    ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    let _ = ctx;
    vec![
        usage_argv::complete::Candidate::described("node", "JavaScript"),
        usage_argv::complete::Candidate::new("python"),
    ]
}

#[test]
fn a_declared_completer_becomes_a_run_the_reference_can_read() {
    // The point of generating the `run=` rather than writing one: there is one place a completer
    // is said to exist — the Rust function — and the spec still says something a reader can act
    // on. What it says is a command asking *this binary*, which is the thing the binary answers.
    static ARG: Arg = Arg {
        key: 90,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static CMD: Command = Command {
        name: "ex",
        args: &[&ARG],
        ..Command::EMPTY
    };
    static META: CommandMeta = CommandMeta {
        cmd: &CMD,
        args: &[ArgMeta {
            arg: &ARG,
            help: Some("Which tool"),
            complete: Some(tools),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static SPEC: Spec = Spec {
        name: "ex",
        bin: Some("ex"),
        version: None,
        long_version: None,
        author: None,
        license: None,
        repository: None,
        source_code_link_template: None,
        min_usage_version: None,
        about: None,
        long_about: None,
        usage: None,
        default_subcommand: None,
        multicall: false,
        views: &[],
        root: &META,
    };

    let kdl = SPEC.to_kdl();
    assert!(
        kdl.contains(r#"complete tool run="ex __complete_word__ --candidates tool --line"#),
        "{kdl}"
    );

    // And usage-lib reads it as a completer for that argument — by the lowercased name, which is
    // the rule both sides resolve one by.
    let lib: LibSpec = kdl.parse().expect("valid spec");
    let complete = lib.complete.get("tool").expect("a complete block");
    let run = complete.run.as_deref().expect("a run command");
    assert!(
        run.starts_with("ex __complete_word__ --candidates tool"),
        "{run}"
    );
    // The line goes with the request, interpolated by whoever runs it — without it a completer
    // that reads an earlier flag answers against nothing, which is a wrong answer rather than a
    // missing one.
    // shell_join preserves the argv boundaries before shell_quote makes the reconstructed line
    // one inert shell argument. The reference owns both filters, so generated producers do not
    // duplicate an incomplete hand-written quote transform.
    assert!(
        run.contains("--line={{ words | shell_join | shell_quote }}"),
        "{run}"
    );
    assert!(!run.contains("replace(from="), "{run}");
    // And no `descriptions=#true`, which would tell the reference to read a description after an
    // unescaped colon: what this answers with is values, and mise's task names are full of colons.
    assert!(!complete.descriptions, "{run}");

    // The binary answers exactly that, filtered by whatever has been typed.
    let words = ["ex".to_string(), "n".to_string()];
    let ctx = usage_argv::complete::CompleteCtx {
        words: &words,
        cword: 1,
        prefix: "n",
        command_words: &[],
        command_path: &[],
    };
    let found = usage_argv::complete::for_name(&SPEC, "tool", &ctx).expect("a completer");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].value, "node");

    // A name nothing declares is an empty answer rather than a failure: a stale script should
    // complete nothing, not print into the prompt.
    assert!(usage_argv::complete::for_name(&SPEC, "nonesuch", &ctx).is_none());
}

fn local_tools(
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    vec![usage_argv::complete::Candidate::new("installed")]
}

fn root_tools(
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    vec![usage_argv::complete::Candidate::new("root")]
}

fn remote_tools(
    _ctx: &usage_argv::complete::CompleteCtx<'_>,
) -> Vec<usage_argv::complete::Candidate<'static>> {
    vec![usage_argv::complete::Candidate::new("available")]
}

#[test]
fn two_commands_can_mean_different_things_by_one_name() {
    // `ex use <TOOL>` and `ex install <TOOL>` take the same *kind* of thing under the same name
    // and answer differently — the installed ones against the available ones. A single top-level
    // block could not say that, and answering with whichever came first in tree order would be
    // answering about the wrong command.
    static USE_TOOL: Arg = Arg {
        key: 91,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static INSTALL_TOOL: Arg = Arg {
        key: 92,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static USE_CMD: Command = Command {
        name: "use",
        args: &[&USE_TOOL],
        ..Command::EMPTY
    };
    static INSTALL_CMD: Command = Command {
        name: "install",
        args: &[&INSTALL_TOOL],
        ..Command::EMPTY
    };
    static ROOT: Command = Command {
        name: "ex",
        subcommands: &[&USE_CMD, &INSTALL_CMD],
        ..Command::EMPTY
    };
    static USE_META: CommandMeta = CommandMeta {
        cmd: &USE_CMD,
        args: &[ArgMeta {
            arg: &USE_TOOL,
            complete: Some(local_tools),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static INSTALL_META: CommandMeta = CommandMeta {
        cmd: &INSTALL_CMD,
        args: &[ArgMeta {
            arg: &INSTALL_TOOL,
            complete: Some(remote_tools),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static ROOT_META_TWO: CommandMeta = CommandMeta {
        cmd: &ROOT,
        subcommands: &[&USE_META, &INSTALL_META],
        ..CommandMeta::EMPTY
    };
    static SPEC_TWO: Spec = Spec {
        name: "ex",
        bin: Some("ex"),
        version: None,
        long_version: None,
        author: None,
        license: None,
        repository: None,
        source_code_link_template: None,
        min_usage_version: None,
        about: None,
        long_about: None,
        usage: None,
        default_subcommand: None,
        multicall: false,
        views: &[],
        root: &ROOT_META_TWO,
    };

    // Each block is written inside the command that declares it, which is where usage-lib looks
    // for one first.
    let kdl = SPEC_TWO.to_kdl();
    let lib: LibSpec = kdl.parse().expect("valid spec");
    assert!(
        lib.complete.is_empty(),
        "neither belongs at the top level: {kdl}"
    );
    for (name, expected) in [("use", "use"), ("install", "install")] {
        let cmd = lib.cmd.subcommands.get(name).expect("the command");
        assert!(
            cmd.complete.contains_key("tool"),
            "{expected} should declare its own: {kdl}"
        );
    }

    // A root may also declare the same name. Cursor identity still wins over that name-based
    // fallback: otherwise a promoted view or an ordinary child argument could silently run the
    // root completer merely because both fields use a conventional name such as `tool`.
    static ROOT_TOOL: Arg = Arg {
        key: 93,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static ROOT_WITH_ARG: Command = Command {
        name: "ex",
        args: &[&ROOT_TOOL],
        subcommands: &[&USE_CMD, &INSTALL_CMD],
        ..Command::EMPTY
    };
    static ROOT_META_THREE: CommandMeta = CommandMeta {
        cmd: &ROOT_WITH_ARG,
        args: &[ArgMeta {
            arg: &ROOT_TOOL,
            complete: Some(root_tools),
            ..ArgMeta::EMPTY
        }],
        subcommands: &[&USE_META, &INSTALL_META],
        ..CommandMeta::EMPTY
    };
    static SPEC_THREE: Spec = Spec {
        name: "ex",
        bin: Some("ex"),
        version: None,
        long_version: None,
        author: None,
        license: None,
        repository: None,
        source_code_link_template: None,
        min_usage_version: None,
        about: None,
        long_about: None,
        usage: None,
        default_subcommand: None,
        multicall: false,
        views: &[],
        root: &ROOT_META_THREE,
    };
    let words = ["ex".to_string(), "use".to_string(), String::new()];
    let ctx = usage_argv::complete::CompleteCtx {
        words: &words,
        cword: 2,
        prefix: "",
        command_words: &[],
        command_path: &[],
    };
    let found = usage_argv::complete::for_name(&SPEC_THREE, "tool", &ctx).expect("a completer");
    assert_eq!(
        found.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
        ["installed"],
        "the argument under the cursor wins before the root name fallback"
    );

    // And the binary answers about the command the line reached, not the first one in the tree.
    for (line, expected) in [("ex use ", "installed"), ("ex install ", "available")] {
        let words: Vec<String> = line.split_whitespace().map(String::from).collect();
        let ctx = usage_argv::complete::CompleteCtx {
            words: &[words.clone(), vec![String::new()]].concat(),
            cword: words.len(),
            prefix: "",
            command_words: &[],
            command_path: &[],
        };
        let found = usage_argv::complete::for_name(&SPEC_TWO, "tool", &ctx).expect("a completer");
        assert_eq!(
            found.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            [expected],
            "{line}"
        );
    }
}

#[test]
fn two_fields_on_one_command_can_mean_different_things_by_one_name() {
    // A spec has one `complete` block per normalized name, but the line sent back to the binary
    // still says whether the cursor is in the positional or the flag. Dispatch by that position,
    // not by whichever field happened to be declared first.
    static TOOL: Arg = Arg {
        key: 94,
        name: "TOOL",
        ..Arg::REQUIRED
    };
    static SOURCE: Flag = Flag {
        key: 95,
        name: "source",
        longs: &["source"],
        ..Flag::VALUE
    };
    static CMD: Command = Command {
        name: "ex",
        flags: &[&SOURCE],
        args: &[&TOOL],
        ..Command::EMPTY
    };
    static META: CommandMeta = CommandMeta {
        cmd: &CMD,
        flags: &[FlagMeta {
            flag: &SOURCE,
            value_name: Some("TOOL"),
            complete: Some(remote_tools),
            ..FlagMeta::EMPTY
        }],
        args: &[ArgMeta {
            arg: &TOOL,
            complete: Some(local_tools),
            ..ArgMeta::EMPTY
        }],
        ..CommandMeta::EMPTY
    };
    static SPEC: Spec = Spec {
        name: "ex",
        bin: Some("ex"),
        root: &META,
        ..Spec::EMPTY
    };

    let kdl = SPEC.to_kdl();
    assert_eq!(
        kdl.matches("complete tool").count(),
        1,
        "one name-keyed block: {kdl}"
    );

    for (words, expected) in [
        (vec!["ex".to_string(), String::new()], "installed"),
        (
            vec!["ex".to_string(), "--source".to_string(), String::new()],
            "available",
        ),
    ] {
        let ctx = usage_argv::complete::CompleteCtx {
            cword: words.len() - 1,
            words: &words,
            prefix: "",
            command_words: &[],
            command_path: &[],
        };
        let found = usage_argv::complete::for_name(&SPEC, "tool", &ctx).expect("a completer");
        assert_eq!(
            found.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            [expected]
        );
    }
}

#[test]
fn multicall_survives_the_round_trip() {
    static SPEC: Spec = Spec {
        name: "busybox",
        bin: Some("busybox"),
        multicall: true,
        root: &CommandMeta::EMPTY,
        ..Spec::EMPTY
    };

    let kdl = SPEC.to_kdl();
    assert!(
        kdl.contains("multicall #true"),
        "lost on the way out: {kdl}"
    );
    let spec: LibSpec = kdl
        .parse()
        .unwrap_or_else(|e| panic!("usage-lib could not parse the emitted spec: {e}\n\n{kdl}"));
    assert!(spec.multicall);
}
