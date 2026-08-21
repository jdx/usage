use crate::{cli::Cli, env};

#[ctor::ctor(unsafe)]
fn init() {
    env::set_var("USAGE_BIN", "usage");
}

#[test]
fn shell_commands_keep_their_verbatim_long_help() {
    let bash = Cli::spec()
        .root
        .subcommands
        .iter()
        .find(|cmd| cmd.cmd.name == "bash")
        .expect("bash command");
    assert_eq!(bash.about, Some("Execute a shell script using bash"));
    assert_eq!(
        bash.long_about,
        Some(
            "Execute a shell script with the specified shell\n\n\
             Typically, this will be called by a script's shebang.\n\n\
             If using `var=#true` on args/flags, they will be joined with spaces using \
             `shell_words::join()`\n\
             to properly escape and quote values with spaces in them."
        )
    );
}

/// Every level has to be a word the logger actually takes.
///
/// `log_filter` exists because `silent` — what the fleet calls the bottom of the scale, and
/// what a spec spells it — is not a level to `log` or `env_logger`; `off` is. Handing them
/// the spec's word would have `env_logger` read it as the name of a module to filter on, so
/// `usage --log-level silent` would answer by logging *more*. This is the CLI that would have
/// done it, so this is where the guarantee is held.
#[test]
fn every_level_names_a_filter_the_logger_understands() {
    use std::str::FromStr as _;
    use usage_rs::Verbosity;

    for level in Verbosity::SCALE {
        let filter = log::LevelFilter::from_str(level.log_filter())
            .unwrap_or_else(|_| panic!("`{}` is not a log filter", level.log_filter()));
        let expected = match level {
            Verbosity::Silent => log::LevelFilter::Off,
            Verbosity::Error => log::LevelFilter::Error,
            Verbosity::Warn => log::LevelFilter::Warn,
            Verbosity::Info => log::LevelFilter::Info,
            Verbosity::Debug => log::LevelFilter::Debug,
            Verbosity::Trace => log::LevelFilter::Trace,
        };
        assert_eq!(filter, expected, "{}", level.as_str());
    }

    // And the spec's own spelling of silence is the one that would not have worked.
    assert!(log::LevelFilter::from_str(Verbosity::Silent.as_str()).is_err());
}
