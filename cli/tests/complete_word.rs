use assert_cmd::assert::Assert;
use assert_cmd::cargo;
use std::env;
use std::fs;
use std::path::PathBuf;
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
///
/// `sh` starting is necessary but not sufficient, so a fixture is run as well. Its shebang is
/// `usage bash`, and on Windows the two need not be the same family: `sh` can be Git Bash while
/// a bare `bash` is the WSL launcher that System32 puts ahead of `PATH`, which cannot open the
/// absolute path `sh` hands its shebang. Probing only `sh` reported such a machine as usable
/// and left these tests failing rather than skipping. `USAGE_SHELL_BASH` is the way out, and
/// the probe sees it because the fixture runs through `usage bash` too.
fn skip_if_posix_shell_missing() -> bool {
    let sh_runs = Command::new("sh")
        .arg("-c")
        .arg("exit 0")
        .output()
        .is_ok_and(|out| out.status.success());
    if sh_runs && mount_fixture_runs() {
        return false;
    }
    if env::var("CI").is_ok_and(|v| !v.is_empty()) {
        panic!("no shell that can run the mount fixtures but CI is set — refusing to skip");
    }
    eprintln!("Skipping test - no shell that can run the mount fixtures");
    true
}

/// Whether a mount fixture actually runs, given as the absolute path `sh` would hand it.
fn mount_fixture_runs() -> bool {
    // Built from `CARGO_MANIFEST_DIR`, not `fs::canonicalize`. On Windows canonicalize returns
    // a `\\?\`-prefixed path, which usage cannot open — the probe would then fail on the shape
    // of the path rather than on the shell, and every mount test would skip on a machine where
    // they work perfectly well.
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join("mounted.sh");
    Command::new(cargo::cargo_bin!("usage"))
        .arg("bash")
        .arg(fixture)
        .arg("--mount")
        .output()
        .is_ok_and(|out| out.status.success())
}

#[test]
fn complete_word_completer() {
    assert_cmd("basic.usage.kdl", &["plugins", "install", "pl"])
        .stdout("plugin-1\nplugin-2\nplugin-3\n");
    assert_cmd("basic.usage.kdl", &["plugins", "install_desc", "pl"])
        .stdout("plugin-1\tdesc\nplugin-2\tdesc\nplugin-3\tdesc\n");
}

#[test]
fn complete_word_run_outranks_a_builtin_inferred_from_the_argument_name() {
    assert_cmd("run-named-builtin.usage.kdl", &["ru"]).stdout("run-result\n");
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
    assert_spec(
        "../benches/mise.usage.kdl",
        &["--", "activate", "--shell", ""],
    )
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
fn complete_word_command_args_starts_with_executables() {
    let usage = cargo::cargo_bin!("usage");
    let executable = usage.file_name().unwrap().to_string_lossy().into_owned();
    let spec = r#"
name "mycli"
bin "mycli"
arg "<COMMAND>..." double_dash="automatic"
complete "command" type="command_args"
"#;
    Command::new(usage)
        .args(["cw", "--shell", "fish", "--spec", spec, "--", "mycli", ""])
        .env("PATH", usage.parent().unwrap())
        .assert()
        .success()
        .stdout(contains(executable));
}

#[test]
fn complete_word_command_args_uses_paths_after_the_command() {
    let usage = cargo::cargo_bin!("usage");
    let spec = r#"
name "mycli"
bin "mycli"
arg "<COMMAND>..." double_dash="automatic"
complete "command" type="command_args"
"#;
    Command::new(usage)
        .args([
            "cw", "--shell", "fish", "--spec", spec, "--", "mycli", "usage", "Cargo",
        ])
        .env("PATH", usage.parent().unwrap())
        .assert()
        .success()
        .stdout(contains("Cargo.toml"));
}

#[test]
fn complete_word_command_args_restarts_with_executables() {
    let usage = cargo::cargo_bin!("usage");
    let executable = usage.file_name().unwrap().to_string_lossy().into_owned();
    let spec = r#"
name "mycli"
bin "mycli"
cmd "run" restart_token=":::" {
    arg "<COMMAND>..." double_dash="automatic"
}
complete "command" type="command_args"
"#;
    Command::new(usage)
        .args([
            "cw", "--shell", "fish", "--spec", spec, "--", "mycli", "run", "usage", ":::", "",
        ])
        .env("PATH", usage.parent().unwrap())
        .assert()
        .success()
        .stdout(contains(executable));
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

#[test]
fn complete_word_config_keys_come_from_the_spec() {
    // No subprocess and no `run=`: the `config` block already says what the keys are, which
    // is the whole point of declaring them there.
    assert_cmd("config.usage.kdl", &["config", "get", ""]).stdout(
        "ancient_log_level\tdeprecated — How much to say\n\
         bool_or_path\tEither a switch or somewhere to put it\n\
         cache_dir\tWhere to keep the cache\n\
         color\tColorize output\n\
         either\tA union with bool second\n\
         jobs\tNumber of parallel jobs\n\
         log_level\tHow much to say\n\
         old_jobs\tdeprecated — How many jobs\n\
         old_log_level\tdeprecated — How much to say\n\
         python.compile\tCompile python from source\n\
         task.output\tHow to print task output\n",
    );
}

#[test]
fn complete_word_config_keys_hide_what_is_hidden() {
    // `internal_thing` is `hide=#true`. It stays in the JSON schema, because it is still
    // settable and a schema that rejected it would be wrong — but nothing should suggest it.
    assert_cmd("config.usage.kdl", &["config", "get", ""]).stdout(contains("internal_thing").not());
    // A dotted key completes whole, one segment at a time being the shell's business.
    assert_cmd("config.usage.kdl", &["config", "get", "python."])
        .stdout("python.compile\tCompile python from source\n");
}

#[test]
fn complete_word_config_values_come_from_the_key_on_the_line() {
    // Choices, in the order the spec lists them, each with its own help.
    assert_cmd("config.usage.kdl", &["config", "set", "log_level", ""]).stdout(
        "error\tonly failures\n\
         warn\tfailures and warnings\n\
         info\tthe usual\n\
         debug\tevery decision\n",
    );
    assert_cmd("config.usage.kdl", &["config", "set", "log_level", "d"])
        .stdout("debug\tevery decision\n");
    // A boolean has two values whether or not it lists them, and `option<bool>` is still a
    // boolean as far as what a user types goes.
    assert_cmd("config.usage.kdl", &["config", "set", "color", ""]).stdout("false\ntrue\n");
    assert_cmd("config.usage.kdl", &["config", "set", "python.compile", ""])
        .stdout("false\ntrue\n");
    // The key's position on the line is the CLI's business, so the scan looks backwards for
    // it rather than assuming it sits directly before the cursor — before the flag, and
    // after it, where the word next to the cursor is `--global` rather than the key.
    for words in [
        ["--", "config", "set", "--global", "log_level", ""],
        ["--", "config", "set", "log_level", "--global", ""],
    ] {
        assert_cmd("config.usage.kdl", &words).stdout(contains("debug\tevery decision"));
    }
}

#[test]
fn complete_word_config_values_read_the_key_from_the_parser() {
    // The key is a positional, and only the parser knows which words are positionals. Here
    // `color` is the value of `--tag` *and* a setting in its own right: scanning the raw words
    // backwards found it and offered its booleans, for a line that is setting `log_level`.
    assert_cmd(
        "config.usage.kdl",
        &["--", "config", "set", "log_level", "--tag", "color", ""],
    )
    .stdout(contains("debug\tevery decision"))
    .stdout(contains("false").not());
}

#[test]
fn complete_word_a_union_offers_booleans_wherever_bool_appears_in_it() {
    // `bool|string` and `string|bool` are the same type written two ways, so which member the
    // spec happens to list first must not decide whether `true` and `false` are offered.
    assert_cmd("config.usage.kdl", &["config", "set", "either", ""]).stdout("false\ntrue\n");
}

#[test]
fn complete_word_the_key_is_the_argument_the_spec_says_holds_one() {
    // Not whichever positional happens to name a setting. Both guesses were wrong in their own
    // direction: scanning backwards took a variadic's own last value, and scanning forwards took
    // an unrelated positional. The argument completed with `config_keys` is the key, and the
    // spec says which one that is.
    //
    // A value after the key that names another setting:
    assert_cmd(
        "config.usage.kdl",
        &["config", "set-many", "log_level", "color", ""],
    )
    .stdout(contains("debug\tevery decision"))
    .stdout(contains("false").not());
    // And an unrelated positional *before* it that names one:
    assert_cmd(
        "config.usage.kdl",
        &["config", "for-profile", "color", "log_level", ""],
    )
    .stdout(contains("debug\tevery decision"))
    .stdout(contains("false").not());
    // With two key arguments, the nearest governs — the same rule as a variadic's last element.
    assert_cmd(
        "config.usage.kdl",
        &["config", "move-to", "color", "log_level", ""],
    )
    .stdout(contains("debug\tevery decision"));
    assert_cmd(
        "config.usage.kdl",
        &["config", "move-to", "log_level", "color", ""],
    )
    .stdout("false\ntrue\n");
    // And only the nearest is consulted: when it names no setting, looking further back offered
    // an earlier key's values for a line whose own key is a typo — where a single unknown key
    // correctly offers nothing of its own.
    assert_cmd(
        "config.usage.kdl",
        &["config", "move-to", "log_level", "nonsense", ""],
    )
    .stdout(contains("only failures").not())
    .stdout(contains("every decision").not());
    // A key argument that has a `default=` of its own, sitting after the value being completed:
    // the typed key governs. A partial parse does not bind an argument from its default, so this
    // passes today by construction — it is here to fail if that ever changes, since such a
    // binding would otherwise win the nearest-wins rule and offer values for a setting nobody
    // wrote.
    assert_cmd(
        "config.usage.kdl",
        &["config", "with-default-key", "log_level", ""],
    )
    .stdout(contains("every decision"))
    .stdout(contains("false").not());
}

#[test]
fn complete_word_config_values_follow_the_names_a_setting_answers_to() {
    // The key a user types is not always the key the spec declares. An `alias` is accepted by
    // the config layer without so much as a warning, so it reaches completion the same way any
    // other key does — and answering it with the working directory, as this did, tells the user
    // an accepted setting is not one.
    assert_cmd("config.usage.kdl", &["config", "set", "loglevel", ""]).stdout(
        "error\tonly failures\n\
         warn\tfailures and warnings\n\
         info\tthe usual\n\
         debug\tevery decision\n",
    );
    // A prefix still filters, and still does not reach for files.
    assert_cmd("config.usage.kdl", &["config", "set", "loglevel", "d"])
        .stdout("debug\tevery decision\n");
    // `renamed_to` is followed for the same reason, and the old name is by definition the one
    // people still have written down. One hop, and two: a setting renamed twice is reachable
    // from the oldest name it ever had.
    assert_cmd("config.usage.kdl", &["config", "set", "old_log_level", ""])
        .stdout(contains("debug\tevery decision"));
    assert_cmd(
        "config.usage.kdl",
        &["config", "set", "ancient_log_level", ""],
    )
    .stdout(contains("debug\tevery decision"));
    // The keys offered stay canonical: an alias is a spelling of a setting already in the
    // list, and a second row for it would be a second setting as far as the menu shows.
    assert_cmd("config.usage.kdl", &["config", "get", "loglev"]).stdout("");
}

#[test]
fn complete_word_a_rename_that_leads_nowhere_still_answers() {
    // A cycle is walked at most as far as there are settings, so this returns rather than
    // following the rename forever. The test is that it terminates at all; both ends of the
    // cycle are boolean so that giving up early could not pass by accident.
    assert_cmd(
        "config-rename-edges.usage.kdl",
        &["config", "set", "cycle_a", ""],
    );
    // A rename pointing at nothing leaves a setting that is still real, with its own type.
    assert_cmd(
        "config-rename-edges.usage.kdl",
        &["config", "set", "dangling", ""],
    )
    .stdout("false\ntrue\n");
}

#[test]
fn complete_word_a_union_that_takes_free_form_values_keeps_the_file_fallback() {
    // `bool|path` accepts the two words *and* any path. Offering the words is right; claiming
    // they are the whole set is not — it closed the candidate list and a path prefix therefore
    // completed to nothing, for a setting whose whole point is that it can be a path.
    assert_cmd("config.usage.kdl", &["config", "set", "bool_or_path", ""]).stdout("false\ntrue\n");
    assert_cmd(
        "config.usage.kdl",
        &["config", "set", "bool_or_path", "src"],
    )
    .stdout(contains("src/"));
}

#[test]
fn complete_word_a_description_is_one_row() {
    // Candidates go to the shell one per line with tab-separated columns, so a description
    // containing a newline splits one candidate into several rows of nonsense. `config_keys`
    // already took the first line; choice help did not.
    let out = assert_cmd("config.usage.kdl", &["config", "set", "task.output", ""]);
    out.stdout("prefix\tprefix each line with the task,\ninterleave\tprint lines as they arrive\n");
}

#[test]
fn complete_word_config_completions_do_not_fall_back_to_files() {
    // The set of settings is known, so a prefix that matches none of them has no completions
    // — not the contents of the working directory. Offering `src/` for `config get zzz` tells
    // the user a filename is a setting, which it is not.
    // `s` on purpose: it matches no setting and no declared value, but it *does* match a
    // directory in the crate this test runs in, so a fallback would be visible. A prefix that
    // matched no file either could not tell the two behaviours apart — as an earlier version
    // of this test could not.
    for words in [
        vec!["config", "get", "s"],
        vec!["config", "set", "log_level", "s"], // a closed set of choices
        vec!["config", "set", "color", "s"],     // a boolean: two values, neither matching
    ] {
        assert_cmd("config.usage.kdl", &words).stdout("");
    }
    // The control: the same prefix on a setting whose values the spec does not enumerate.
    assert_cmd("config.usage.kdl", &["config", "set", "cache_dir", "s"]).stdout(contains("src/"));
}

#[test]
fn complete_word_config_values_say_nothing_when_they_know_nothing() {
    // A path-valued setting, and a key that is not a setting at all: both fall through to
    // the file completion every other unconstrained argument gets, which for `cache_dir` is
    // exactly what a user wants. Anchored on one directory of the crate this test runs in,
    // rather than a whole listing that would depend on the working tree.
    for key in ["cache_dir", "not_a_setting"] {
        assert_cmd("config.usage.kdl", &["config", "set", key, "src"]).stdout(contains("src/"));
    }
}

fn cmd(spec: &str, shell: Option<&str>) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("usage"));
    cmd.args(["cw"]);
    if let Some(shell) = shell {
        cmd.args(["--shell", shell]);
    }
    cmd.args(["-f", spec, "mycli"]);
    cmd
}

fn assert_cmd(example: &str, args: &[&str]) -> Assert {
    assert_spec(&format!("../examples/{example}"), args)
}

fn assert_spec(spec: &str, args: &[&str]) -> Assert {
    cmd(spec, Some("fish")).args(args).assert().success()
}
