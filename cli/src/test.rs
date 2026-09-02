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
    // Every line break of the doc comment is kept, which is what `verbatim_doc_comment` promises;
    // without it the derive would reflow each paragraph into one line.
    let expected = [
        "Run a script whose usage spec is written in its own comments",
        "",
        "Usually reached through the script's shebang, `#!/usr/bin/env -S usage bash`: the kernel",
        "hands the script and its arguments to `usage`, which parses them against the `#USAGE`",
        "lines at the top of the file and then runs the script with this shell. Each flag and",
        "argument reaches the script as an environment variable named `usage_<name>`, so",
        "`--force` becomes `usage_force` and `<file>` becomes `usage_file`. A value declared",
        "`var=#true` arrives as one string, joined with `shell_words::join()` so that an element",
        "containing a space stays quoted.",
        "",
        "`-h` and `--help` print the script's help page rather than this one. The shell is found",
        "on PATH; `USAGECLI_SHELL_BASH`, `USAGECLI_SHELL_ZSH`, `USAGECLI_SHELL_FISH` and",
        "`USAGECLI_SHELL_PWSH` each name a different program to run instead, which is how",
        "`usage bash` is pointed at Git Bash on a Windows machine where `bash` is WSL.",
    ]
    .join("\n");
    assert_eq!(bash.long_about, Some(expected.as_str()));
}
