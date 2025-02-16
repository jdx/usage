use pretty_assertions::assert_str_eq;
use usage::parse;
use usage::Spec;

macro_rules! tests {
    ($($name:ident: spec=$spec:expr, args=$args:expr, expected=$expected:expr,)*) => {
    $(
        #[test]
        fn $name() {
            let spec: Spec = $spec.parse().unwrap();
            let mut args = shell_words::split($args).unwrap();
            args.insert(0, "test".to_string());
            match parse(&spec, &args) {
                Ok(env) => assert_str_eq!(format!("{:?}", env.as_env()).trim(), $expected.trim()),
                Err(e) => assert_str_eq!(format!("{e}").trim(), $expected.trim()),
            }
        }
    )*
    }
}

tests! {
required_arg:
    spec=r#"arg "<name>""#,
    args="",
    expected=r#"Missing required arg: <name>"#,

required_flag:
    spec=r#"flag "--name <name>" required=#true"#,
    args="",
    expected=r#"Missing required flag: --name <name>"#,

negate:
    spec=r#"flag "--force" negate="--no-force""#,
    args="--no-force",
    expected=r#"{"usage_force": "false"}"#,

flag_short_next:
    spec=r#"flag "-s <shell>""#,
    args="-sbash",
    expected=r#"{"usage_s": "bash"}"#,

flag_short_space:
    spec=r#"flag "-s <shell>""#,
    args="-s bash",
    expected=r#"{"usage_s": "bash"}"#,

flag_choices_ok:
    spec=r#"flag "--shell <shell>" {
    choices "bash" "fish" "zsh"
}"#,
    args="--shell bash",
    expected=r#"{"usage_shell": "bash"}"#,

flag_choices_err:
    spec=r#"flag "-s --shell <shell>" {
    choices "bash" "fish" "zsh"
}"#,
    args="-s invalid",
    expected=r#"Invalid choice for option shell: invalid, expected one of bash, fish, zsh"#,

arg_choices_ok:
    spec=r#"arg "<shell>" {
    choices "bash" "fish" "zsh"
}"#,
    args="bash",
    expected=r#"{"usage_shell": "bash"}"#,

arg_choices_err:
    spec=r#"arg "<shell>" {
    choices "bash" "fish" "zsh"
}"#,
    args="invalid",
    expected=r#"Invalid choice for arg shell: invalid, expected one of bash, fish, zsh"#,

arg_choices_help_short:
    spec=r#"arg "<shell>" help="shorthelp" help_long="help\nfooo\nbar"{
    choices "bash" "fish" "zsh"
}"#,
    args="-h",
    expected=r#"Usage: <shell>

Arguments:
  <shell>  shorthelp [bash, fish, zsh]
"#,

arg_choices_help_long:
    spec=r#"arg "<shell>" help="shorthelp" help_long="help\nfooo\nbar"{
    choices "bash" "fish" "zsh"
}"#,
    args="--help",
    expected=r#"Usage: <shell>

Arguments:
  <shell>
    help
    fooo
    bar
    [possible values: bash, fish, zsh]
"#,

flag_choices_help_short:
    spec=r#"flag "--shell <shell>" help="shorthelp" help_long="help\nfooo\nbar"{
    choices "bash" "fish" "zsh"
}"#,
    args="-h",
    expected=r#"Usage: [--shell <shell>]

Flags:
  --shell <shell>  shorthelp [bash, fish, zsh]
"#,

flag_choices_help_long:
    spec=r#"flag "--shell <shell>" help="shorthelp" help_long="help\nfooo\nbar"{
    choices "bash" "fish" "zsh"
}"#,
    args="--help",
    expected=r#"Usage: [--shell <shell>]

Flags:
  --shell <shell>
    help
    fooo
    bar
    [possible values: bash, fish, zsh]
"#,

cmd_help_short:
    spec=r#"cmd "cmd" help="shorthelp" help_long="help\nfooo\nbar""#,
    args="-h",
    expected=r#"Usage: <SUBCOMMAND>

Commands:
  cmd  shorthelp
  help  Print this message or the help of the given subcommand(s)
"#,

cmd_help_long:
    spec=r#"cmd "cmd" help="shorthelp" help_long="help\nfooo\nbar""#,
    args="--help",
    expected=r#"Usage: <SUBCOMMAND>

Commands:
  cmd
    help
    fooo
    bar

  help
    Print this message or the help of the given subcommand(s)
    "#,

subcommand_help_short:
    spec=r#"cmd "plugins" {
    cmd "install" help="shorthelp" help_long="help\nfooo\nbar"
}"#,
    args="plugins -h",
    expected=r#"Usage: plugins <SUBCOMMAND>

Commands:
  plugins install  shorthelp
  help  Print this message or the help of the given subcommand(s)
"#,

flag_default:
    spec=r#"
    flag "--port <port>" default="8080"
    flag "--host <host>" default="localhost"
    "#,
    args="--port 8081",
    expected=r#"{"usage_host": "localhost", "usage_port": "8081"}"#,

arg_default:
    spec=r#"
    arg "<port>" default="8080"
    arg "<host>" default="localhost"
    "#,
    args="8081",
    expected=r#"{"usage_host": "localhost", "usage_port": "8081"}"#,

multi_arg:
    spec=r#"
    arg "<vars>" var=#true
    "#,
    args="a b c",
    expected=r#"{"usage_vars": "a b c"}"#,

multi_arg_spaces:
    spec=r#"
    arg "<vars>" var=#true
    "#,
    args=r#"a "b c""#,
    expected=r#"{"usage_vars": "a 'b c'"}"#,

multi_flag:
    spec=r#"
    flag "-v --vars <vars>" var=#true
    "#,
    args=r#"--vars a --vars "b c""#,
    expected=r#"{"usage_vars": "a 'b c'"}"#,

//shell_escape_arg:
//    spec=r#"
//    arg "<vars>" shell_escape=#true
//    "#,
//    args=r#"a "b c""#,
//    expected=r#"{"usage_vars": "a 'b c'"}"#,
}
