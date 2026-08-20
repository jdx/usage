use clap::{Arg, ArgAction, ColorChoice, Command, ValueHint};
use std::collections::BTreeSet;

/// A clap behavior that the generated usage spec cannot preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FidelityFeature {
    Environment,
    HiddenFlagAlias,
    ValueHint,
    NonAsciiDelimiter,
    NonUtf8Default,
    ValueArity,
    DistinctValueNames,
    GranularHide,
    AllowMissingPositional,
    FlattenHelp,
    NextLineHelp,
    SubcommandHelpHeading,
    SubcommandValueName,
    DisableHelpFlag,
    DisableHelpSubcommand,
    DisableVersionFlag,
    DisableColoredHelp,
    Color,
}

/// One detectable difference between a clap command and its generated spec.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FidelityLoss {
    /// Command path, rooted at the clap command's name.
    pub command: Vec<String>,
    /// clap argument ID when the loss belongs to one argument.
    pub argument: Option<String>,
    /// The behavior that cannot be represented.
    pub feature: FidelityFeature,
    /// The concrete source metadata that triggered the report.
    pub detail: String,
}

/// Structured, deterministic losses found while converting a clap command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FidelityReport {
    losses: Vec<FidelityLoss>,
}

impl FidelityReport {
    /// Every detected loss, sorted by command, argument, feature, and detail.
    pub fn losses(&self) -> &[FidelityLoss] {
        &self.losses
    }

    /// Whether every clap behavior visible through public getters was preserved.
    pub fn is_lossless(&self) -> bool {
        self.losses.is_empty()
    }
}

pub(crate) fn report(cmd: &Command) -> FidelityReport {
    let mut losses = BTreeSet::new();
    visit(cmd, &[], &mut losses);
    FidelityReport {
        losses: losses.into_iter().collect(),
    }
}

fn visit(cmd: &Command, ancestors: &[String], losses: &mut BTreeSet<FidelityLoss>) {
    let mut path = ancestors.to_vec();
    path.push(cmd.get_name().to_string());

    let mut command_loss = |set: bool, feature: FidelityFeature, detail: &str| {
        if set {
            losses.insert(FidelityLoss {
                command: path.clone(),
                argument: None,
                feature,
                detail: detail.to_string(),
            });
        }
    };

    command_loss(
        cmd.is_allow_missing_positional_set(),
        FidelityFeature::AllowMissingPositional,
        "allow_missing_positional",
    );
    command_loss(
        cmd.is_flatten_help_set(),
        FidelityFeature::FlattenHelp,
        "flatten_help",
    );
    command_loss(
        cmd.is_next_line_help_set(),
        FidelityFeature::NextLineHelp,
        "next_line_help",
    );
    command_loss(
        cmd.get_subcommand_help_heading().is_some(),
        FidelityFeature::SubcommandHelpHeading,
        "subcommand_help_heading",
    );
    command_loss(
        cmd.get_subcommand_value_name().is_some(),
        FidelityFeature::SubcommandValueName,
        "subcommand_value_name",
    );
    command_loss(
        cmd.is_disable_help_flag_set(),
        FidelityFeature::DisableHelpFlag,
        "disable_help_flag",
    );
    command_loss(
        cmd.get_subcommands().next().is_some() && cmd.is_disable_help_subcommand_set(),
        FidelityFeature::DisableHelpSubcommand,
        "disable_help_subcommand",
    );
    command_loss(
        cmd.get_version().is_some() && cmd.is_disable_version_flag_set(),
        FidelityFeature::DisableVersionFlag,
        "disable_version_flag",
    );
    command_loss(
        cmd.is_disable_colored_help_set(),
        FidelityFeature::DisableColoredHelp,
        "disable_colored_help",
    );
    command_loss(
        cmd.get_color() != ColorChoice::Auto,
        FidelityFeature::Color,
        "color",
    );
    for arg in cmd.get_arguments() {
        argument_losses(arg, &path, losses);
    }
    for sub in cmd.get_subcommands() {
        visit(sub, &path, losses);
    }
}

fn argument_losses(arg: &Arg, path: &[String], losses: &mut BTreeSet<FidelityLoss>) {
    let id = arg.get_id().to_string();
    let mut add = |feature: FidelityFeature, detail: String| {
        losses.insert(FidelityLoss {
            command: path.to_vec(),
            argument: Some(id.clone()),
            feature,
            detail,
        });
    };

    if let Some(env) = arg.get_env() {
        add(
            FidelityFeature::Environment,
            format!("env={}", env.to_string_lossy()),
        );
    }
    if arg.get_value_hint() != ValueHint::Unknown {
        add(
            FidelityFeature::ValueHint,
            format!("value_hint={:?}", arg.get_value_hint()),
        );
    }
    if let Some(delimiter) = arg
        .get_value_delimiter()
        .filter(|delimiter| !delimiter.is_ascii())
    {
        add(
            FidelityFeature::NonAsciiDelimiter,
            format!("value_delimiter={delimiter:?}"),
        );
    }
    for value in arg.get_default_values() {
        if value.to_str().is_none() {
            add(
                FidelityFeature::NonUtf8Default,
                "default contains non-UTF-8 bytes".to_string(),
            );
            break;
        }
    }
    if let Some(range) = arg.get_num_args() {
        let takes_values = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
        let lost = takes_values
            && (arg.get_value_delimiter().is_some()
                || !arg.is_positional() && range.min_values() == 0);
        if lost {
            add(
                FidelityFeature::ValueArity,
                format!(
                    "num_args={}..={}, action={:?}",
                    range.min_values(),
                    range.max_values(),
                    arg.get_action()
                ),
            );
        }
    }
    let value_names = arg.get_value_names().unwrap_or_default();
    if value_names.len() > 1
        && (arg.get_value_delimiter().is_some()
            || arg.get_num_args().is_some_and(|range| {
                range.min_values() != value_names.len() || range.max_values() != value_names.len()
            }))
    {
        add(
            FidelityFeature::DistinctValueNames,
            format!(
                "{} value names with ranged or delimiter-split arity",
                value_names.len()
            ),
        );
    }
    let mut hidden = Vec::new();
    if arg.is_hide_default_value_set() {
        hidden.push("default_value");
    }
    if arg.is_hide_possible_values_set() {
        hidden.push("possible_values");
    }
    if arg.is_hide_env_set() {
        hidden.push("env");
    }
    if arg.is_hide_env_values_set() {
        hidden.push("env_values");
    }
    if arg.is_hide_short_help_set() {
        hidden.push("short_help");
    }
    if arg.is_hide_long_help_set() {
        hidden.push("long_help");
    }
    if !hidden.is_empty() {
        add(FidelityFeature::GranularHide, hidden.join(","));
    }
}
