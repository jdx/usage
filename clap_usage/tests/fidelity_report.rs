use clap::{Arg, ArgAction, ArgGroup, Command, ValueHint};
use clap_usage::{spec_with_report, FidelityFeature};

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

    let (_spec, report) = spec_with_report(&mut command, "ex");
    let features: Vec<_> = report.losses().iter().map(|loss| loss.feature).collect();
    for expected in [
        FidelityFeature::ArgRequiredElseHelp,
        FidelityFeature::Environment,
        FidelityFeature::HiddenFlagAlias,
        FidelityFeature::ValueHint,
        FidelityFeature::ValueTerminator,
        FidelityFeature::GranularHide,
        FidelityFeature::ValueArity,
        FidelityFeature::DistinctValueNames,
        FidelityFeature::PositionalGroupMember,
    ] {
        assert!(
            features.contains(&expected),
            "missing {expected:?}: {report:#?}"
        );
    }
    assert!(report.losses().iter().all(|loss| loss.command == ["ex"]));
    assert!(!report.is_lossless());
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
    let (_, report) = spec_with_report(&mut nested, "ex");
    assert_eq!(report.losses()[0].command, ["ex", "run"]);
    assert_eq!(report.losses()[0].argument.as_deref(), Some("number"));
    assert_eq!(
        report.losses()[0].feature,
        FidelityFeature::AllowNegativeNumbers
    );
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
