use pretty_assertions::assert_str_eq;
use usage::Spec;

macro_rules! tests_same {
    ($($name:ident: $spec:expr,)*) => {
    $(
        #[test]
        fn $name() {
            let spec: Spec = $spec.parse().unwrap();
            assert_str_eq!(format!("{spec}").trim(), $spec.trim());
        }
    )*
    }
}

tests_same! {
    negate: r#"flag "--force" negate="--no-force""#,

    flag_choices: r#"flag "--shell" {
    arg "<shell>" {
        choices "bash" "fish" "zsh"
    }
}"#,

    arg_choices: r#"arg "<shell>" {
    choices "bash" "fish" "zsh"
}"#,

    double_dash: r#"arg "<-- shell...>""#,
}
