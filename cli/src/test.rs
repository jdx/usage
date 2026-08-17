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
