#[test]
fn command_macro_captures_stdout() {
    let output = usage_test::command!("usage", "--version").assert_success();

    assert!(output.stdout_text().starts_with("usage "));
    assert_eq!(output.stderr_text(), "");
}

#[test]
fn command_macro_keeps_a_failed_commands_stderr_and_status() {
    let output = usage_test::command!("usage");

    assert!(!output.status.success());
    assert!(output.stderr_text().contains("Usage:"));
}
