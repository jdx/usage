//! What the command line contributes when a setting-bound flag also has an `env`.
//!
//! Its own test binary, like `post_binding_env`, and for the same reason: an environment
//! variable is process-wide, so a test that sets one races every other test in the binary that
//! parses a CLI reading it.

use std::ffi::OsStr;

use usage_config::{resolve, EnvLayer, Layers, PropMeta, Registry, Ty, Value};
use usage_derive::Cli;

/// A CLI whose flag and variable set the same setting
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    /// How many at once
    #[usage(long, env = "EX_SETTINGS_JOBS", setting = "jobs")]
    jobs: Option<usize>,
}

static PROPS: &[PropMeta] = &[PropMeta {
    envs: &["EX_SETTINGS_JOBS"],
    cli: &["--jobs"],
    ..PropMeta::new("jobs", Ty::Uint)
}];
const REGISTRY: Registry = Registry::new(PROPS);

#[test]
fn a_value_the_environment_filled_in_is_not_a_command_line_value() {
    unsafe { std::env::set_var("EX_SETTINGS_JOBS", "6") };

    // The field is filled, because that is what `env` is for.
    let argv = [OsStr::new("ex")][1..].to_vec();
    let (ex, layer) = Ex::parse_from_with_settings(&argv).expect("should parse");
    assert_eq!(ex.jobs, Some(6), "the field still comes from the variable");

    // The layer is not, because nothing was given on the command line. The marker the layer is
    // gated on says "this field was filled", and the environment fallback sets it too — so a
    // variable's value was arriving as a command-line one: named `--jobs` in an explanation the
    // user could not act on, ahead of the layers the environment is supposed to sit among, and
    // counted twice by a `union` setting that also has an `EnvLayer`.
    assert!(layer.is_empty(), "the environment is not the command line");

    // So the variable is what sets it, through the layer that knows it is a variable.
    let env = EnvLayer::from_process();
    let resolved = resolve(REGISTRY, Layers::new().then(&layer).then(&env)).expect("resolves");
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(6)));
    assert_eq!(
        resolved.origin_key("jobs").map(|o| o.describe()),
        Some("EX_SETTINGS_JOBS"),
        "and it is named as one"
    );

    // And a flag still is one, over the top of it.
    let argv = [OsStr::new("--jobs"), OsStr::new("8")];
    let (ex, layer) = Ex::parse_from_with_settings(&argv).expect("should parse");
    assert_eq!(ex.jobs, Some(8));
    let resolved = resolve(REGISTRY, Layers::new().then(&layer).then(&env)).expect("resolves");
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
    assert_eq!(
        resolved.origin_key("jobs").map(|o| o.describe()),
        Some("--jobs")
    );
}
