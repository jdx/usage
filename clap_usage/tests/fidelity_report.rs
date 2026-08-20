use clap::{Arg, ArgAction, ArgGroup, Command, ValueHint};
use clap_usage::{spec_with_report, FidelityFeature};

#[test]
fn long_version_is_lossless() {
    let mut command = Command::new("ex")
        .version("1.2.3")
        .long_version("1.2.3\ncommit abc123");
    let (spec, report) = spec_with_report(&mut command, "ex");

    assert_eq!(spec.version.as_deref(), Some("1.2.3"));
    assert_eq!(spec.long_version.as_deref(), Some("1.2.3\ncommit abc123"));
    assert!(report.is_lossless(), "{report:#?}");
}

#[test]
fn disabled_builtin_entries_are_preserved() {
    let mut command = Command::new("ex")
        .long_version("commit abc123")
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .subcommand(Command::new("run"));
    let (spec, report) = spec_with_report(&mut command, "ex");

    assert!(spec.cmd.disable_help_flag);
    assert!(spec.cmd.disable_help_subcommand);
    assert!(spec.cmd.disable_version_flag);
    assert!(report.is_lossless(), "{report:#?}");
}

#[test]
fn custom_builtin_actions_are_preserved() {
    let mut command = Command::new("ex")
        .version("1.0.0")
        .disable_help_flag(true)
        .arg(
            Arg::new("assist")
                .long("assist")
                .help("Show concise help")
                .action(ArgAction::HelpShort),
        )
        .arg(
            Arg::new("release")
                .long("release")
                .help("Show version")
                .action(ArgAction::Version),
        );
    let (spec, report) = spec_with_report(&mut command, "ex");

    assert_eq!(spec.cmd.flags[0].action, usage::SpecFlagAction::HelpShort);
    assert_eq!(spec.cmd.flags[1].action, usage::SpecFlagAction::Version);
    assert!(report.is_lossless(), "{report:#?}");
}

#[test]
fn display_order_is_lossless() {
    let mut command = Command::new("ex")
        .arg(Arg::new("second").long("second").display_order(20))
        .arg(Arg::new("first").long("first").display_order(10))
        .subcommand(Command::new("second").display_order(20))
        .subcommand(Command::new("first").display_order(10));
    let (spec, report) = spec_with_report(&mut command, "ex");

    assert_eq!(spec.cmd.flags[0].display_order, Some(20));
    assert_eq!(spec.cmd.flags[1].display_order, Some(10));
    assert_eq!(spec.cmd.subcommands[0].display_order, Some(20));
    assert_eq!(spec.cmd.subcommands[1].display_order, Some(10));
    assert!(report.is_lossless(), "{report:#?}");
}

#[test]
fn reports_detectable_losses_with_locations() {
    let mut command = Command::new("ex")
        .arg_required_else_help(true)
        .arg(
            Arg::new("config")
                .long("config")
                .env("EX_CONFIG")
                .alias("cfg")
                .value_hint(ValueHint::FilePath)
                .value_terminator(";")
                .hide_env_values(true),
        )
        .arg(
            Arg::new("pair")
                .long("pair")
                .action(ArgAction::Append)
                .num_args(2)
                .value_names(["START", "END"]),
        )
        .arg(Arg::new("target"))
        .group(ArgGroup::new("input").args(["config", "target"]));

    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(spec.cmd.arg_required_else_help);
    assert_eq!(spec.cmd.complete["config"].type_.as_deref(), Some("path"));
    let features: Vec<_> = report.losses().iter().map(|loss| loss.feature).collect();
    let expected = FidelityFeature::Environment;
    assert!(
        features.contains(&expected),
        "missing {expected:?}: {report:#?}"
    );
    let pair = spec
        .cmd
        .flags
        .iter()
        .find(|flag| flag.name == "pair")
        .unwrap();
    assert_eq!(pair.arg.as_ref().unwrap().value_names, ["START", "END"]);
    assert!(
        spec.cmd
            .flags
            .iter()
            .find(|flag| flag.name == "config")
            .unwrap()
            .hide_env_values
    );
    assert_eq!(
        (
            pair.arg.as_ref().unwrap().var_min,
            pair.arg.as_ref().unwrap().var_max
        ),
        (Some(2), Some(2))
    );
    assert!(!features.contains(&FidelityFeature::DistinctValueNames));
    assert!(report.losses().iter().all(|loss| loss.command == ["ex"]));
    assert!(!report.is_lossless());
}

#[test]
fn every_clap_value_hint_is_portable() {
    let cases = [
        (ValueHint::Other, Some("none")),
        (ValueHint::AnyPath, Some("path")),
        (ValueHint::FilePath, Some("path")),
        (ValueHint::DirPath, Some("dir")),
        (ValueHint::ExecutablePath, Some("executable")),
        (ValueHint::CommandName, Some("command")),
        (ValueHint::CommandString, Some("command")),
        (ValueHint::CommandWithArguments, Some("command_args")),
        (ValueHint::Username, Some("username")),
        (ValueHint::Hostname, Some("hostname")),
        (ValueHint::Url, Some("url")),
        (ValueHint::EmailAddress, Some("email")),
        (ValueHint::Unknown, None),
    ];
    for (hint, expected) in cases {
        let mut command = if hint == ValueHint::CommandWithArguments {
            Command::new("ex").trailing_var_arg(true).arg(
                Arg::new("value")
                    .action(ArgAction::Append)
                    .num_args(1..)
                    .value_hint(hint),
            )
        } else {
            Command::new("ex").arg(
                Arg::new("value")
                    .long("value")
                    .action(ArgAction::Set)
                    .value_hint(hint),
            )
        };
        let (spec, report) = spec_with_report(&mut command, "ex");
        assert!(report.is_lossless(), "{hint:?}: {report:#?}");
        assert_eq!(
            spec.cmd
                .complete
                .get("value")
                .and_then(|complete| complete.type_.as_deref()),
            expected,
            "{hint:?}"
        );
    }
}

#[test]
fn fixed_arity_distinct_value_names_are_lossless() {
    let mut command = Command::new("ex").arg(
        Arg::new("pair")
            .long("pair")
            .num_args(2)
            .value_names(["START", "END"]),
    );
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    let arg = spec.cmd.flags[0].arg.as_ref().unwrap();
    assert_eq!(arg.value_names, ["START", "END"]);
    assert_eq!((arg.var_min, arg.var_max), (Some(2), Some(2)));
}

#[test]
fn subcommand_presentation_is_lossless() {
    let mut command = Command::new("ex")
        .subcommand(Command::new("run"))
        .subcommand_help_heading("Actions")
        .subcommand_value_name("ACTION");
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    assert_eq!(spec.cmd.subcommand_help_heading.as_deref(), Some("Actions"));
    assert_eq!(spec.cmd.subcommand_value_name.as_deref(), Some("ACTION"));
}

#[test]
fn next_line_help_is_lossless() {
    let mut command = Command::new("ex")
        .next_line_help(true)
        .arg(Arg::new("config").long("config").help("Config file"))
        .subcommand(Command::new("run").about("Run it"));
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    assert!(spec.cmd.next_line_help);
}

#[test]
fn flatten_help_is_lossless() {
    let mut command = Command::new("ex")
        .flatten_help(true)
        .subcommand(Command::new("run").about("Run it"));
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    assert!(spec.cmd.flatten_help);
}

#[test]
fn ranged_distinct_value_names_are_reported_as_lossy() {
    let mut command = Command::new("ex").arg(
        Arg::new("range")
            .long("range")
            .num_args(2..=4)
            .value_names(["START", "END"]),
    );
    let report = spec_with_report(&mut command, "ex").1;
    assert!(report
        .losses()
        .iter()
        .any(|loss| loss.feature == FidelityFeature::DistinctValueNames));
}

#[test]
fn delimited_distinct_value_names_are_reported_and_not_emitted_as_fixed_arity() {
    let mut command = Command::new("ex").arg(
        Arg::new("pair")
            .long("pair")
            .num_args(2)
            .value_delimiter(',')
            .value_names(["START", "END"]),
    );
    let (spec, report) = spec_with_report(&mut command, "ex");
    let arg = spec.cmd.flags[0].arg.as_ref().unwrap();
    assert_eq!(arg.value_names, ["START"]);
    assert_eq!((arg.var_min, arg.var_max), (None, None));
    assert!(report
        .losses()
        .iter()
        .any(|loss| loss.feature == FidelityFeature::DistinctValueNames));
    spec.to_string().parse::<usage::Spec>().unwrap();
}

#[test]
fn reports_nested_paths_and_leaves_supported_commands_clean() {
    let mut clean = Command::new("ex")
        .arg(Arg::new("verbose").long("verbose"))
        .arg(Arg::new("input"))
        .arg(Arg::new("values").num_args(0..=3))
        .group(ArgGroup::new("derived_struct").arg("input").multiple(true));
    let (_, report) = spec_with_report(&mut clean, "ex");
    assert!(report.is_lossless(), "{report:#?}");

    let mut nested = Command::new("ex").subcommand(
        Command::new("run").arg(
            Arg::new("number")
                .long("number")
                .allow_negative_numbers(true),
        ),
    );
    let (spec, report) = spec_with_report(&mut nested, "ex");
    assert!(!spec.cmd.subcommands.contains_key("help"));
    assert!(report.is_lossless(), "{report:#?}");
    assert!(spec.cmd.subcommands["run"].flags[0]
        .arg
        .as_ref()
        .is_some_and(|arg| arg.allow_negative_numbers));
}

#[test]
fn reports_delimited_arity_that_the_bridge_cannot_count() {
    let mut command = Command::new("ex").arg(
        Arg::new("values")
            .long("values")
            .num_args(2)
            .value_delimiter(','),
    );
    let (_, report) = spec_with_report(&mut command, "ex");
    assert!(report
        .losses()
        .iter()
        .any(|loss| loss.feature == FidelityFeature::ValueArity));
}

#[test]
fn append_action_fixed_arity_is_lossless() {
    let mut command = Command::new("ex").arg(
        Arg::new("values")
            .long("values")
            .action(ArgAction::Append)
            .num_args(2),
    );
    let (_, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
}

#[test]
fn value_names_infer_fixed_arity_without_num_args() {
    let mut command = Command::new("ex")
        .arg(Arg::new("pair").long("pair").value_names(["LEFT", "RIGHT"]))
        .arg(Arg::new("coords").value_names(["X", "Y"]));
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    let flag = spec.cmd.flags[0].arg.as_ref().unwrap();
    assert_eq!((flag.var_min, flag.var_max), (Some(2), Some(2)));
    assert_eq!(
        (spec.cmd.args[0].var_min, spec.cmd.args[0].var_max),
        (Some(2), Some(2))
    );
    spec.to_string().parse::<usage::Spec>().unwrap();
}

#[test]
fn trailing_delimiter_policy_is_lossless() {
    let mut command = clap::Command::new("ex")
        .dont_delimit_trailing_values(true)
        .arg(clap::Arg::new("value"));
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    assert!(spec.cmd.dont_delimit_trailing_values);
}

#[test]
fn building_metadata_does_not_mutate_the_callers_command() {
    let mut command = Command::new("ex").subcommand(Command::new("run"));
    let argument_count = command.get_arguments().count();
    let subcommand_count = command.get_subcommands().count();
    let (spec, _) = spec_with_report(&mut command, "ex");

    assert_eq!(command.get_arguments().count(), argument_count);
    assert_eq!(command.get_subcommands().count(), subcommand_count);
    assert!(!spec.cmd.subcommands.contains_key("help"));
}

#[test]
fn hidden_flag_aliases_are_lossless() {
    let mut command = Command::new("ex").arg(
        Arg::new("output")
            .long("output")
            .alias("quietly")
            .short('o')
            .short_alias('q'),
    );
    let (spec, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
    let output = &spec.cmd.flags[0];
    assert_eq!(output.hidden_aliases, ["quietly"]);
    assert_eq!(output.hidden_short_aliases, ['q']);
}

#[test]
fn positional_conflicts_are_lossless_from_either_endpoint() {
    for command in [
        Command::new("ex")
            .arg(Arg::new("file").conflicts_with("force"))
            .arg(Arg::new("force").long("force")),
        Command::new("ex")
            .arg(Arg::new("file"))
            .arg(Arg::new("force").long("force").conflicts_with("file")),
    ] {
        let mut command = command;
        let (_, report) = spec_with_report(&mut command, "ex");
        assert!(report.is_lossless(), "{report:#?}");
    }
}

#[test]
fn exclusive_flags_are_not_reported_as_positional_losses() {
    let mut command = Command::new("ex")
        .arg(Arg::new("dump").long("dump").exclusive(true))
        .arg(Arg::new("file"));
    let (_, report) = spec_with_report(&mut command, "ex");
    assert!(report.is_lossless(), "{report:#?}");
}
