use assert_cmd::assert::Assert;
use assert_cmd::cargo;
use std::env;
use std::fs;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use predicates::str::contains;

/// Returns `true` if the test should be skipped because there is no usable POSIX shell.
/// Panics under `CI` (any non-empty value) so a missing shell there is a configuration bug
/// rather than a silent pass.
///
/// The mount fixtures are `#!/usr/bin/env -S usage bash` scripts, so `mount run=` can only
/// work where `sh` can start them. Without this guard, Windows machines with no POSIX shell
/// fall through to the `cmd /c` path, which hands a `.sh` to whatever program is registered
/// for the extension — an editor window per test rather than a failure.
///
/// The probe runs a script and checks that it exited cleanly, rather than only that something
/// spawned. On Windows `sh` can resolve to a program that starts and then fails, which would
/// otherwise read as a working shell and put the tests back in the confusing state above.
fn skip_if_posix_shell_missing() -> bool {
    let usable = Command::new("sh")
        .arg("-c")
        .arg("exit 0")
        .output()
        .is_ok_and(|out| out.status.success());
    if usable {
        return false;
    }
    if env::var("CI").is_ok_and(|v| !v.is_empty()) {
        panic!("no usable POSIX shell (`sh`) but CI is set — refusing to skip");
    }
    eprintln!("Skipping test - no usable POSIX shell (`sh`)");
    true
}

#[test]
fn complete_word_completer() {
    assert_cmd("basic.usage.kdl", &["plugins", "install", "pl"])
        .stdout("plugin-1\nplugin-2\nplugin-3\n");
    assert_cmd("basic.usage.kdl", &["plugins", "install_desc", "pl"])
        .stdout("plugin-1\tdesc\nplugin-2\tdesc\nplugin-3\tdesc\n");
}

#[test]
fn complete_word_variadic_arg_reuses_completer() {
    assert_cmd("variadic-completion.usage.kdl", &["--", "variadic", ""]).stdout("foo\nbar\n");
    assert_cmd(
        "variadic-completion.usage.kdl",
        &["--", "variadic", "foo", ""],
    )
    .stdout("foo\nbar\n");
}

#[test]
fn complete_word_double_dash_required_offers_the_separator() {
    // The parser rejects anything given to a `double_dash="required"` arg before `--`, so
    // there is exactly one useful thing to complete there.
    assert_cmd("double-dash.usage.kdl", &["--", "separator", ""]).stdout("--\n");
    // Once the separator is in, the arg completes normally.
    assert_cmd("double-dash.usage.kdl", &["--", "separator", "--", ""]).stdout("alpha\nbeta\n");
}

#[test]
fn complete_word_double_dash_stops_flag_completion() {
    // Before the separator, a dash-prefixed token completes to flags as usual...
    assert_cmd("double-dash.usage.kdl", &["--", "separator", "--v"]).stdout(contains("--verbose"));
    // ...and after it there are no flags left to complete: the parser reads everything past
    // `--` as a positional value, so offering `--verbose` would suggest something it would
    // hand to <target> verbatim.
    assert_cmd("double-dash.usage.kdl", &["--", "separator", "--", "--v"]).stdout("");
}

#[test]
fn complete_word_double_dash_keeps_file_fallback_for_dash_prefixed_values() {
    // A dash-prefixed word means a flag before the separator and a value after it, so path
    // completion has to be withheld in the first case and offered in the second. Needs a file
    // whose name starts with `-` to tell the two apart, hence the scratch directory.
    let dir = std::env::temp_dir().join(format!("usage_dash_file_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("-dashed.txt"), "").unwrap();

    let spec = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join("double-dash.usage.kdl");
    let run = |args: &[&str]| {
        let mut c = Command::new(cargo::cargo_bin!("usage"));
        c.current_dir(&dir)
            .args(["cw", "--shell", "fish", "-f"])
            .arg(&spec)
            .arg("mycli")
            .args(args);
        String::from_utf8(c.output().unwrap().stdout).unwrap()
    };

    // The leading `--` is clap's own escape, so the words start after it.
    assert!(
        !run(&["--", "paths", "-d"]).contains("-dashed.txt"),
        "before `--`, `-d` is a flag prefix"
    );
    assert!(
        run(&["--", "paths", "--", "-d"]).contains("-dashed.txt"),
        "after `--`, `-d` is the start of a value"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn complete_word_double_dash_applies_after_a_restart_token() {
    // A restart_token starts a fresh invocation, so the separator has to be typed again.
    assert_cmd(
        "double-dash.usage.kdl",
        &["--", "restarted", "--", "alpha", ":::", ""],
    )
    .stdout("--\n");
    assert_cmd(
        "double-dash.usage.kdl",
        &["--", "restarted", "--", "alpha", ":::", "--", ""],
    )
    .stdout("alpha\nbeta\n");
}

#[test]
fn complete_word_double_dash_applies_to_a_default_subcommand() {
    // Root-level completion reaches the default subcommand's first arg by its own path.
    assert_cmd("double-dash-default-subcommand.usage.kdl", &["--", ""])
        .stdout(contains("--\n"))
        .stdout(contains("alpha").not());
    assert_cmd(
        "double-dash-default-subcommand.usage.kdl",
        &["--", "--", ""],
    )
    .stdout(contains("alpha"))
    .stdout(contains("beta"));
}

#[test]
fn complete_word_double_dash_routes_past_greedy_variadic() {
    // Before the separator the greedy variadic is still the target...
    assert_cmd("double-dash.usage.kdl", &["--", "routed", ""]).stdout("one\ntwo\n");
    assert_cmd("double-dash.usage.kdl", &["--", "routed", "one", ""]).stdout("one\ntwo\n");
    // ...and after it the completion follows the parser onto the arg that required it.
    assert_cmd("double-dash.usage.kdl", &["--", "routed", "one", "--", ""]).stdout("alpha\nbeta\n");
}

#[test]
fn complete_word_variadic_arg_respects_var_max() {
    assert_cmd("variadic-completion.usage.kdl", &["--", "bounded", ""]).stdout("foo\nbar\n");
    assert_cmd(
        "variadic-completion.usage.kdl",
        &["--", "bounded", "foo", ""],
    )
    .stdout("foo\nbar\n");
    assert_cmd(
        "variadic-completion.usage.kdl",
        &["--", "bounded", "foo", "bar", ""],
    )
    .stdout(contains("Cargo.toml"));
}

#[test]
fn complete_word_subcommands() {
    assert_cmd("basic.usage.kdl", &["plugins", "install"]).stdout(contains("install"));
}

#[test]
fn complete_word_cword() {
    assert_cmd("basic.usage.kdl", &["--cword=3", "plugins", "install"])
        .stdout(contains("plugin-2"));
}

#[test]
fn complete_word_long_flag() {
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "--"]).stdout("--dir\n--global\n");
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "--g"]).stdout("--global\n");
    assert_cmd(
        "basic.usage.kdl",
        &["--", "plugins", "install", "--global", "pl"],
    )
    .stdout(contains("plugin-2"));
}

#[test]
fn complete_word_long_flag_val() {
    assert_cmd(
        "basic.usage.kdl",
        &["--", "plugins", "install", "--dir", ""],
    )
    .stdout(contains("src").and(contains("tests")));
}

#[test]
fn complete_word_short_flag() {
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "-"])
        .stdout("-d\n-g\n--dir\n--global\n");
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "-g"]).stdout("-g\n");
    assert_cmd("basic.usage.kdl", &["--", "plugins", "install", "-g", "pl"])
        .stdout(contains("plugin-2"));
}

#[test]
fn complete_word_kitchen_sink() {
    assert_cmd("kitchen-sink.usage.kdl", &["--", "install", "--"])
        .stdout("--dir\n--force\n--global\n--no-force\n");
    assert_cmd("kitchen-sink.usage.kdl", &["--", "--shell", ""]).stdout("bash\nzsh\nfish\n");
}

#[test]
fn complete_word_choices() {
    assert_cmd("mise.usage.kdl", &["--", "env", "--shell", ""])
        .stdout("bash\nelvish\nfish\nnu\nxonsh\nzsh\npwsh\n");
}

#[test]
fn complete_word_choices_from_env() {
    cmd("env-choices.usage.kdl", Some("fish"))
        .env("DEPLOY_ENVS", "foo,bar baz")
        .args(["--", "--env", ""])
        .assert()
        .success()
        .stdout("foo\nbar\nbaz\n");
}

#[test]
fn complete_word_choices_from_env_unset_returns_empty() {
    cmd("env-choices.usage.kdl", Some("fish"))
        .env_remove("DEPLOY_ENVS")
        .args(["--", "--env", ""])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn complete_word_default_subcommand_choices_do_not_block_root_file_fallback() {
    assert_cmd("default-subcommand-root-fallback.usage.kdl", &["--", "C"])
        .stdout(contains("Cargo.toml"));
}

#[test]
fn complete_word_shebang() {
    assert_cmd("example.sh", &["--", "-"])
        .stdout("--bar\tOption value\n--defaulted\tDefaulted value\n--foo\tFlag value\n");
}

#[test]
fn complete_word_arg_completer() {
    assert_cmd("example.sh", &["--", "v"]).stdout("val-1\nval-2\nval-3\n");
}

#[test]
fn complete_word_mounted() {
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());
    assert_cmd("mounted.sh", &["--", "-"]).stdout("--mount\tDisplay kdl spec for mounted tasks\n");
    assert_cmd("mounted.sh", &["--", ""]).stdout("exec-task\n");
    assert_cmd("mounted.sh", &["--", "exec-task", ""]).stdout("task-a\ntask-b\n");
}

#[test]
fn complete_word_mounted_with_global_flags() {
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // Without --dir flag, should get default tasks
    assert_cmd("mounted-global-flags.sh", &["--", "run", ""])
        .stdout("task-a\tTask from default dir\ntask-b\tTask from default dir\n");

    // With --dir=dir2 flag, should get dir2 tasks
    assert_cmd(
        "mounted-global-flags.sh",
        &["--", "--dir", "dir2", "run", ""],
    )
    .stdout("task-bar\tTask from dir2\ntask-foo\tTask from dir2\n");

    // Edge case: embedded value (--dir=dir2) should also work
    assert_cmd("mounted-global-flags.sh", &["--", "--dir=dir2", "run", ""])
        .stdout("task-bar\tTask from dir2\ntask-foo\tTask from dir2\n");

    // Edge case: multiple flags with embedded values
    assert_cmd("mounted-global-flags.sh", &["--", "--dir=dir2", "run", ""])
        .stdout("task-bar\tTask from dir2\ntask-foo\tTask from dir2\n");

    // Edge case: short flag (-d) should work the same as long flag
    assert_cmd("mounted-global-flags.sh", &["--", "-d", "dir2", "run", ""])
        .stdout("task-bar\tTask from dir2\ntask-foo\tTask from dir2\n");
}

#[test]
fn complete_word_mounted_global_flag_choices() {
    // Regression for the parser-side root cause referenced by jdx/mise#10069:
    // a value-taking global flag placed before a mounted subcommand must not leak its
    // consumed tokens into the mounted task's `choices` positional arg.
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // Baseline: no global flag prefix. The mount sees no `usage_cd`, so it returns the
    // default choices. This must complete (not error) and not be polluted by stray tokens.
    assert_cmd(
        "mounted-global-flags-choices.sh",
        &["--", "run", "sample:run", ""],
    )
    .stdout("one\ntwo\n");

    // Value-taking global flag before the mounted subcommand (the failing case). The choices
    // switch to the dir2 set, which also proves `usage_cd=dir2` propagated to the mount even
    // though `run` re-declares `-C/--cd` as non-global.
    assert_cmd(
        "mounted-global-flags-choices.sh",
        &["--", "-C", "dir2", "run", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");

    // Long form.
    assert_cmd(
        "mounted-global-flags-choices.sh",
        &["--", "--cd", "dir2", "run", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");

    // Embedded-value form.
    assert_cmd(
        "mounted-global-flags-choices.sh",
        &["--", "--cd=dir2", "run", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");
}

#[test]
fn complete_word_mounted_orphan_short_flag_choices() {
    // Follow-up to jdx/mise#10069: a long-only global flag re-declared as a non-global flag
    // with an ADDED short (`-r --raw`, `-S --silent`) must keep the orphan short recognized.
    // Completing a mounted task with the short in front must return the task's choices rather
    // than bailing with "unexpected word" / "Invalid choice" (which mise worked around by
    // promoting the short back to global).
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // Orphan short `-r` before the mounted task (the failing case).
    assert_cmd(
        "mounted-orphan-short-flags.sh",
        &["--", "run", "-r", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");

    // A different orphan short (`-S`) on the same subcommand.
    assert_cmd(
        "mounted-orphan-short-flags.sh",
        &["--", "run", "-S", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");

    // The nested `tasks run` path exercises a second descent level.
    assert_cmd(
        "mounted-orphan-short-flags.sh",
        &["--", "tasks", "run", "-r", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");

    // The original long alias still works after the merge.
    assert_cmd(
        "mounted-orphan-short-flags.sh",
        &["--", "run", "--raw", "sample:run", ""],
    )
    .stdout("alpha\nbeta\ngamma\n");
}

#[test]
fn complete_word_mounted_does_not_offer_mounting_cli_flags() {
    // Regression for jdx/mise#11282: the mounting CLI's global flags must not be offered
    // inside a mounted command, and must not shadow the mounted command's own flags.
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // Only the mounted task's own flags are offered. Previously this also listed the root's
    // `--env`/`--silent`, which the mounted program rejects.
    assert_cmd("mounted-global-flag-leak.sh", &["--", "run", "mytask", "--"])
        .stdout("--bump\tVersion bump\n--env\tEnvironment to deploy to\n--output-dir\tWhere to write output\n");

    // Shorts of the mounting CLI's globals (`-E`) are not offered either, only the task's.
    assert_cmd("mounted-global-flag-leak.sh", &["--", "run", "mytask", "-"])
        .stdout("--bump\tVersion bump\n--env\tEnvironment to deploy to\n--output-dir\tWhere to write output\n");

    // The task's `--env` wins over the root global of the same name, so its choices complete
    // instead of the global's (previously: file completion from the global's `<ENV>` arg).
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "run", "mytask", "--env", ""],
    )
    .stdout("dev\nstage\nprod\n");

    // A non-colliding task flag is unaffected.
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "run", "mytask", "--bump", ""],
    )
    .stdout("auto\nmajor\nminor\npatch\n");

    // The mounting CLI's flags still complete *before* the mounted command.
    assert_cmd("mounted-global-flag-leak.sh", &["--", "run", "--"]).stdout(
        "--env\tSet the environment\n--force\tForce the tasks to run\n--silent\tSilent output\n",
    );

    // And a global before the task still parses: the task's arg choices complete after it,
    // and the global's value reaches the mount (jdx/mise#10069 behavior is preserved).
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "-E", "prod", "run", "mytask", ""],
    )
    .stdout("alpha\nbeta\n");

    // Even when the global's value would be rejected by the mounted flag of the same name, it
    // keeps parsing as the global, because Phase 1 recorded which flag it was read as.
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "--env", "not-a-task-choice", "run", "mytask", ""],
    )
    .stdout("alpha\nbeta\n");

    // ...and the mounted `--env` still owns the name after the mounted command, so its choices
    // complete there rather than the global's file-path fallback.
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "--env", "prod", "run", "mytask", "--env", ""],
    )
    .stdout("dev\nstage\nprod\n");

    // Inside the mounted tree the mounted program's own commands are ordinary commands: a
    // global it declares is still offered in its subcommands, while the mounting CLI's are not.
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "run", "grouped", "leaf", "--"],
    )
    .stdout("--group-wide\tApplies to the whole group\n--leaf-only\tOnly on the leaf\n");

    // A NON-global flag of `run` before the task name must not hide the mounted command either
    // (`mise run --force build <TAB>` used to fail with `unexpected word: build`).
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "run", "--force", "mytask", "--"],
    )
    .stdout("--bump\tVersion bump\n--env\tEnvironment to deploy to\n--output-dir\tWhere to write output\n");
    assert_cmd(
        "mounted-global-flag-leak.sh",
        &["--", "run", "-f", "mytask", ""],
    )
    .stdout("alpha\nbeta\n");
}

#[test]
fn complete_word_boolean_flags_dont_consume_subcommands() {
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // Boolean flag --verbose before subcommand 'run' should not consume 'run'
    assert_cmd("test-boolean-flags.sh", &["--", "--verbose", "run", ""])
        .stdout("task-verbose\tTask with verbose\n");

    // Multiple boolean flags before subcommand
    assert_cmd(
        "test-boolean-flags.sh",
        &["--", "--verbose", "--debug", "run", ""],
    )
    .stdout("task-verbose-debug\tTask with verbose and debug\n");

    // Edge case: short boolean flags should work
    assert_cmd("test-boolean-flags.sh", &["--", "-v", "run", ""])
        .stdout("task-verbose\tTask with verbose\n");

    // Edge case: mixed short and long boolean flags
    assert_cmd("test-boolean-flags.sh", &["--", "-v", "-d", "run", ""])
        .stdout("task-verbose-debug\tTask with verbose and debug\n");
}

#[test]
fn complete_word_non_global_flags_do_not_stop_search() {
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // A non-global flag before a subcommand is consumed like a global one, so the subcommand
    // (and its mount) is still found. It used to stop the search, leaving `run` to be read as a
    // positional: `unexpected word: run`, with no completions at all. Only the flag's scope
    // differs — being non-global, it is not forwarded to the mount, so the mount still reports
    // the default task rather than the `--verbose` one.
    assert_cmd("test-boolean-flags.sh", &["--", "--local", "run", ""])
        .stdout("task-default\tTask default\n");

    // An *unknown* flag still stops the search: the parser can't know whether it takes a value,
    // so `run` may well be that value.
    let mut cmd = cmd("test-boolean-flags.sh", Some("fish"));
    cmd.args(["--", "--nope", "run", ""]);
    cmd.assert().failure().stderr(contains("unexpected word"));
}

#[test]
fn complete_word_mixed_global_flags() {
    if skip_if_posix_shell_missing() {
        return;
    }
    let mut path = env::split_paths(&env::var("PATH").unwrap()).collect::<Vec<_>>();
    path.insert(
        0,
        env::current_dir()
            .unwrap()
            .join("..")
            .join("target")
            .join("debug"),
    );
    path.insert(0, env::current_dir().unwrap().join("..").join("examples"));
    env::set_var("PATH", env::join_paths(path).unwrap());

    // Mix of boolean and valued flags, long and short, with embedded values
    // This test uses the test-boolean-flags fixture but we only care about
    // verifying that the flags are parsed correctly (not the actual completion)
    // Note: This won't actually change the tasks since test-boolean-flags doesn't
    // have a --dir flag, but it tests that the parser handles the combination
    assert_cmd("test-boolean-flags.sh", &["--", "-v", "--debug", "run", ""])
        .stdout("task-verbose-debug\tTask with verbose and debug\n");
}

#[test]
fn complete_word_fallback_to_files() {
    // Use a minimal spec with no args or subcommands, so any argument is unknown
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args([
        "cw",
        "--shell",
        "fish",
        "-f",
        "../examples/basic.usage.kdl",
        "mycli",
        "plugins",
        "install",
        "foo",
        "",
    ]);
    // Assert for files always present in the project root
    cmd.assert()
        .success()
        .stdout(contains("Cargo.toml").and(contains("src")));
}

#[test]
fn complete_word_subcommands_without_shell() {
    let mut cmd = cmd("basic.usage.kdl", None);
    cmd.args(["plugins", "install"]);
    cmd.assert().success().stdout(contains("install"));
}

#[test]
fn complete_word_escaped_colons_in_completions() {
    // When completions contain colons, they are escaped as \: in the name:description format.
    // Prefix matching should work against the unescaped names.

    // Typing "test:" should match "test:unit" and "test:integration" (unescaped)
    assert_cmd("colon-in-completions.usage.kdl", &["--", "run", "test:"])
        .stdout("test:unit\tRun unit tests\ntest:integration\tRun integration tests\n");

    // Typing "test" should also match (prefix of the unescaped name)
    assert_cmd("colon-in-completions.usage.kdl", &["--", "run", "test"])
        .stdout("test:unit\tRun unit tests\ntest:integration\tRun integration tests\n");

    // Typing "build" should only match "build"
    assert_cmd("colon-in-completions.usage.kdl", &["--", "run", "build"])
        .stdout("build\tBuild the project\n");

    // Empty input should match all completions
    assert_cmd("colon-in-completions.usage.kdl", &["--", "run", ""])
        .stdout("test:unit\tRun unit tests\ntest:integration\tRun integration tests\nbuild\tBuild the project\n");
}

#[test]
fn complete_word_zsh_three_columns_with_descriptions() {
    // zsh output is three tab-separated columns per line: raw value, raw
    // description, shell-quoted insert. No `\:`/`\(`/`\[` escaping is needed
    // because the generated completion script builds the menu display from
    // the value/description columns directly (no `_describe`).
    // See: https://github.com/jdx/usage/issues/558
    let mut c = cmd("parens-in-descriptions.usage.kdl", Some("zsh"));
    c.args(["--", "run", ""]);
    c.assert().success().stdout(
        "connect:server\tConnect server (Hot Reload)\tconnect:server\n\
         test:unit\tRun tests [fast]\ttest:unit\n\
         build\tBuild project\tbuild\n",
    );
}

#[test]
fn complete_word_zsh_three_columns_without_descriptions() {
    // Description column is empty when no description is set, but the three
    // tab-separated columns are still emitted so the receiving template can
    // parse uniformly with `read -r value desc insert`.
    let mut c = cmd("zsh-colons-without-descriptions.usage.kdl", Some("zsh"));
    c.args(["--", "run", ""]);
    c.assert()
        .success()
        .stdout("test:git\t\ttest:git\ntest:nvim\t\ttest:nvim\n");
}

fn cmd(example: &str, shell: Option<&str>) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["cw"]);
    if let Some(shell) = shell {
        cmd.args(["--shell", shell]);
    }
    cmd.args(["-f", &format!("../examples/{example}"), "mycli"]);
    cmd
}

fn assert_cmd(example: &str, args: &[&str]) -> Assert {
    cmd(example, Some("fish")).args(args).assert().success()
}
