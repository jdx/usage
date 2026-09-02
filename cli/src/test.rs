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
    // Every line break of the doc comment is kept, which is what `verbatim_doc_comment`
    // promises; without it the derive would reflow each paragraph into one line.
    let shared = [
        "Run a script whose usage spec is written in its own comments",
        "",
        "Usually reached through the script's shebang: the kernel hands the",
        "script and its arguments to `usage`, which parses them against the",
        "`#USAGE` lines at the top of the file, then runs the script with this",
        "shell. Each flag and argument reaches the script as an environment",
        "variable named `usage_<name>`, so `--force` becomes `usage_force` and",
        "`<file>` becomes `usage_file`. A value declared `var=#true` arrives as",
        "one string, joined with `shell_words::join()` so that an element",
        "containing a space stays quoted.",
        "",
        "`-h` and `--help` print the script's help page rather than this one.",
        "",
        // Joining leaves the blank line that separates the shared text from the
        // per-shell tail, so `shared` can be concatenated with either.
        "",
    ]
    .join("\n");
    let expected = format!(
        "{shared}This command's shebang is `#!/usr/bin/env -S usage bash`, and the\n\
         program it runs is `bash` from PATH. `USAGECLI_SHELL_BASH` names a\n\
         different one, which is how it is pointed at Git Bash on a Windows\n\
         machine where `bash` is the WSL launcher."
    );
    assert_eq!(bash.long_about, Some(expected.as_str()));

    // The shell-specific tail is per command, not shared: a `usage zsh` reader must not be
    // told to write a `usage bash` shebang. This is what the macro's fourth argument is for.
    let zsh = Cli::spec()
        .root
        .subcommands
        .iter()
        .find(|cmd| cmd.cmd.name == "zsh")
        .expect("zsh command");
    let zsh_long = zsh.long_about.expect("zsh long help");
    assert!(zsh_long.starts_with(&shared), "{zsh_long}");
    assert!(
        zsh_long.contains("`#!/usr/bin/env -S usage zsh`"),
        "{zsh_long}"
    );
    assert!(zsh_long.contains("USAGECLI_SHELL_ZSH"), "{zsh_long}");
    assert!(!zsh_long.contains("bash"), "{zsh_long}");
    assert!(!zsh_long.contains("Git Bash"), "{zsh_long}");
}
