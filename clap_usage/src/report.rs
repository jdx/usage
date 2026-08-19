use clap::{Arg, ArgAction, ColorChoice, Command, ValueHint};
use std::collections::BTreeSet;

/// A clap behavior that the generated usage spec cannot preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FidelityFeature {
    Environment,
    HiddenFlagAlias,
    ValueHint,
    ValueTerminator,
    NonAsciiDelimiter,
    NonUtf8Default,
    AllowNegativeNumbers,
    ValueArity,
    DistinctValueNames,
    GranularHide,
    PositionalRelationship,
    PositionalGroupMember,
    DontDelimitTrailingValues,
    ArgRequiredElseHelp,
    AllowMissingPositional,
    ArgsConflictWithSubcommands,
    ArgsOverrideSelf,
    SubcommandPrecedenceOverArg,
    SubcommandNegatesRequirements,
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
        cmd.is_dont_delimit_trailing_values_set(),
        FidelityFeature::DontDelimitTrailingValues,
        "dont_delimit_trailing_values",
    );
    command_loss(
        cmd.is_arg_required_else_help_set(),
        FidelityFeature::ArgRequiredElseHelp,
        "arg_required_else_help",
    );
    command_loss(
        cmd.is_allow_missing_positional_set(),
        FidelityFeature::AllowMissingPositional,
        "allow_missing_positional",
    );
    command_loss(
        cmd.is_args_conflicts_with_subcommands_set(),
        FidelityFeature::ArgsConflictWithSubcommands,
        "args_conflicts_with_subcommands",
    );
    command_loss(
        cmd.is_args_override_self(),
        FidelityFeature::ArgsOverrideSelf,
        "args_override_self",
    );
    command_loss(
        cmd.is_subcommand_precedence_over_arg_set(),
        FidelityFeature::SubcommandPrecedenceOverArg,
        "subcommand_precedence_over_arg",
    );
    command_loss(
        cmd.is_subcommand_negates_reqs_set(),
        FidelityFeature::SubcommandNegatesRequirements,
        "subcommand_negates_reqs",
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
    command_loss(
        cmd.is_allow_negative_numbers_set(),
        FidelityFeature::AllowNegativeNumbers,
        "command allow_negative_numbers",
    );

    for arg in cmd.get_arguments() {
        argument_losses(cmd, arg, &path, losses);
    }
    for group in cmd.get_groups() {
        // A non-required group that allows multiple members imposes no parsing rule. Clap
        // derive creates these groups for ordinary structs, so treating them as a bridge loss
        // would make an otherwise exact conversion look lossy.
        if group.clone().is_multiple() && !group.is_required_set() {
            continue;
        }
        for member in group.get_args() {
            if cmd
                .get_arguments()
                .find(|arg| arg.get_id() == member)
                .is_some_and(Arg::is_positional)
            {
                losses.insert(FidelityLoss {
                    command: path.clone(),
                    argument: Some(member.to_string()),
                    feature: FidelityFeature::PositionalGroupMember,
                    detail: format!("group {} contains positional {member}", group.get_id()),
                });
            }
        }
    }
    for sub in cmd.get_subcommands() {
        visit(sub, &path, losses);
    }
}

fn argument_losses(cmd: &Command, arg: &Arg, path: &[String], losses: &mut BTreeSet<FidelityLoss>) {
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
    let hidden_longs: Vec<_> = arg.get_aliases().unwrap_or_default();
    let visible_shorts = arg.get_visible_short_aliases().unwrap_or_default();
    let hidden_shorts: Vec<_> = arg
        .get_all_short_aliases()
        .unwrap_or_default()
        .into_iter()
        .filter(|alias| !visible_shorts.contains(alias))
        .collect();
    if !hidden_longs.is_empty() || !hidden_shorts.is_empty() {
        add(
            FidelityFeature::HiddenFlagAlias,
            format!("long={hidden_longs:?}, short={hidden_shorts:?}"),
        );
    }
    if arg.get_value_hint() != ValueHint::Unknown {
        add(
            FidelityFeature::ValueHint,
            format!("value_hint={:?}", arg.get_value_hint()),
        );
    }
    if let Some(terminator) = arg.get_value_terminator() {
        add(
            FidelityFeature::ValueTerminator,
            format!("value_terminator={terminator}"),
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
    if arg.is_allow_negative_numbers_set() {
        add(
            FidelityFeature::AllowNegativeNumbers,
            "allow_negative_numbers".to_string(),
        );
    }
    if let Some(names) = arg.get_value_names().filter(|names| names.len() > 1) {
        add(
            FidelityFeature::DistinctValueNames,
            format!("value_names={names:?}"),
        );
    }
    if let Some(range) = arg.get_num_args() {
        let takes_values = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
        let lost = takes_values
            && (arg.get_value_delimiter().is_some()
                || !arg.is_positional()
                    && (range.min_values() == 0 || matches!(arg.get_action(), ArgAction::Append)));
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

    for other in cmd.get_arg_conflicts_with(arg) {
        // `exclusive` is represented directly on a flag. Clap expands it into conflicts
        // with every other argument, but reporting those generated edges would describe a
        // behavior the bridge already preserved as lost. Ordinary conflicts are lossy in
        // this dialect whenever either endpoint is positional; check both endpoints because
        // the public blacklist need not expose an unbuilt declaration symmetrically.
        if (arg.is_positional() || other.is_positional())
            && !arg.is_exclusive_set()
            && !other.is_exclusive_set()
        {
            add(
                FidelityFeature::PositionalRelationship,
                format!("conflicts_with={}", other.get_id()),
            );
        }
    }
}
