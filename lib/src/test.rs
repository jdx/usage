use crate::Spec;
use once_cell::sync::Lazy;

#[macro_export]
macro_rules! spec {
    { $spec:literal } => {
        $spec.parse::<$crate::spec::Spec>()
    };
}

pub static SPEC_KITCHEN_SINK: Lazy<Spec> = Lazy::new(|| {
    spec! {r#"
bin "mycli"
arg "arg1" help="arg1 description"
arg "arg2" help="arg2 description" default="default value" {
    choices "choice1" "choice2" "choice3"
}
arg "arg3" help="arg3 description" required=true long_help="arg3 long description"
arg "argrest" var=true

flag "--flag1" help="flag1 description"
flag "--flag2" help="flag2 description" long_help="flag2 long description"
flag "--flag3" help="flag3 description" negate="--no-flag3"

flag "--shell <shell>" {
  choices "bash" "zsh" "fish"
}

cmd "plugin" {
  cmd "install" {
    arg "plugin"
    arg "version"
    flag "-g --global"
    flag "-d --dir <dir>"
    flag "-f --force" negate="--no-force"
  }
}

complete "plugin" run="echo \"plugin-1\nplugin-2\nplugin-3\""
"#}
    .unwrap()
});

#[test]
fn test_parse() {
    assert_eq!(SPEC_KITCHEN_SINK.name, "mycli");
}
