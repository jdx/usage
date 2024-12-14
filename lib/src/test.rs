use crate::Spec;
use once_cell::sync::Lazy;

#[macro_export]
macro_rules! spec {
    { $spec:literal } => {
        $spec.parse::<$crate::spec::Spec>()
    };
}

pub static SPEC_KITCHEN_SINK: Lazy<Spec> = Lazy::new(|| {
    spec! {r##"
bin "mycli"
source_code_link_template "https://github.com/jdx/mise/blob/main/src/cli/{{path}}.rs"
arg "arg1" help="arg1 description"
arg "arg2" help="arg2 description" default="default value" {
    choices "choice1" "choice2" "choice3"
}
arg "arg3" help="arg3 description" required=true long_help="arg3 long description"
arg "argrest" var=true
arg "with-default" default="default value"

flag "--flag1" help="flag1 description"
flag "--flag2" help="flag2 description" long_help=r#"flag2 long description

includes a code block:

    $ echo hello world
    hello world

    more code

Examples:

    # run with no arguments to use the interactive selector
    $ mise use

    # set the current version of node to 20.x in mise.toml of current directory
    # will write the fuzzy version (e.g.: 20)

some docs

    $ echo hello world
    hello world
"#
flag "--flag3" help="flag3 description" negate="--no-flag3"
flag "--with-default" required=true default="default value"

flag "--shell <shell>" {
  choices "bash" "zsh" "fish"
}

cmd "plugin" {
  cmd "install" long_help="install a plugin" {
    arg "plugin"
    arg "version"
    flag "-g --global" global=true
    flag "-d --dir <dir>"
    flag "-f --force" negate="--no-force"
  }
}

complete "plugin" run="echo \"plugin-1\nplugin-2\nplugin-3\""
"##}
    .unwrap()
});

#[test]
fn test_parse() {
    assert_eq!(SPEC_KITCHEN_SINK.name, "mycli");
}

#[test]
fn test_arg_not_required_if_default() {
    assert!(
        !SPEC_KITCHEN_SINK
            .cmd
            .args
            .iter()
            .find(|f| f.name == "with-default")
            .unwrap()
            .required
    );
}

#[test]
fn test_flag_not_required_if_default() {
    assert!(
        !SPEC_KITCHEN_SINK
            .cmd
            .flags
            .iter()
            .find(|f| f.name == "with-default")
            .unwrap()
            .required
    );
}
