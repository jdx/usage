//! A flag with `setting = "key"` as the declaration of that setting.
//!
//! Everything a registry entry says about a flag-backed setting is already on the field that parses
//! the flag: the key, the type, the environment names, the help, and the flags themselves. So the
//! root joins what its flags declare into `SETTINGS_REGISTRY`, and a `usage::Config` struct is left
//! for the settings no flag sets — files only, or ones that need a default, a merge or a scope.
//! A key both declare is the `Config` struct's.

use std::ffi::OsStr;
use std::path::PathBuf;

use usage_config::{EnvLayer, Layers, Parser, Ty, Value, resolve};
use usage_derive::{Args, Cli, Config, Subcommands};

/// How the relay listens for TLS
#[derive(Args, Debug)]
struct ListenTls {
    /// Load the given certificate from disk.
    #[usage(
        long = "listen-tls-cert",
        env = "EX_LISTEN_TLS_CERT",
        deprecated_env = "EX_TLS_CERT",
        delimiter = ',',
        setting = "listen.tls.cert"
    )]
    cert: Vec<PathBuf>,
}

/// Run the relay
#[derive(Args, Debug)]
struct Serve {
    /// How many jobs to run at once
    #[usage(long, short = 'j', setting = "jobs")]
    jobs: Option<u16>,
}

/// Check the relay's configuration
#[derive(Args, Debug)]
struct Check {
    /// How many jobs to run at once
    #[usage(long, setting = "jobs")]
    jobs: Option<u16>,
}

#[derive(Subcommands, Debug)]
enum Cmd {
    Serve(Serve),
    Check(Check),
}

/// The settings no flag sets.
#[derive(Config)]
struct Settings {
    /// Where the relay announces itself
    #[usage(env = "EX_NAME")]
    name: Option<String>,

    /// Whether to colour the output
    #[usage(default = true, cli("--colour", "--no-colour"))]
    colour: bool,
}

/// A relay
#[derive(Cli, Debug)]
#[usage(bin = "ex", config = Settings)]
struct Ex {
    /// Whether to colour the output
    #[usage(long, negate = "no-colour", setting = "colour")]
    colour: bool,

    /// How chatty to be
    #[usage(short, count, setting = "log.verbose")]
    verbose: u8,

    #[usage(flatten)]
    tls: ListenTls,

    #[usage(subcommand)]
    cmd: Cmd,
}

fn parse(tokens: &[&str]) -> (Ex, usage_config::CliLayer) {
    let argv: Vec<&OsStr> = tokens.iter().map(OsStr::new).collect();
    Ex::parse_from_with_settings(&argv).expect("should parse")
}

fn prop(key: &str) -> usage_config::PropMeta {
    *Ex::SETTINGS_PROPS
        .iter()
        .find(|p| p.key == key)
        .unwrap_or_else(|| panic!("`{key}` is declared"))
}

#[test]
fn the_registry_declares_exactly_the_flags_the_cli_binds() {
    // The adopter's drift test, with nothing hand-written on either side.
    assert_eq!(
        Ex::SETTINGS_REGISTRY.drift(Ex::SETTINGS_BINDINGS),
        Vec::<String>::new()
    );
}

#[test]
fn a_flag_says_what_its_setting_is() {
    let cert = prop("listen.tls.cert");
    assert_eq!(cert.ty, Ty::List(&Ty::Path));
    assert_eq!(cert.envs, ["EX_LISTEN_TLS_CERT"]);
    assert_eq!(cert.deprecated_envs, ["EX_TLS_CERT"]);
    assert_eq!(cert.parse, Some(Parser::ListByComma));
    assert_eq!(cert.cli, ["--listen-tls-cert"]);
    assert_eq!(cert.help, Some("Load the given certificate from disk."));
    // No registry default, so absence is a state the reader handles.
    assert_eq!(cert.optional, Some(true));

    let verbose = prop("log.verbose");
    assert_eq!(verbose.ty, Ty::Uint);
    assert_eq!(verbose.cli, ["-v"]);
}

#[test]
fn a_key_several_commands_bind_is_one_setting_with_all_their_flags() {
    let jobs: Vec<_> = Ex::SETTINGS_PROPS
        .iter()
        .filter(|p| p.key == "jobs")
        .collect();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].ty, Ty::Uint);
    assert_eq!(jobs[0].cli, ["--jobs", "-j"]);
}

#[test]
fn the_config_type_keeps_the_keys_it_declares_and_adds_the_rest() {
    // `colour` is declared by both, and the `Config` struct's declaration — with its default —
    // is the one the registry holds.
    let colour = prop("colour");
    assert_eq!(colour.default, Some(usage_config::Const::Bool(true)));
    assert_eq!(
        Ex::SETTINGS_PROPS
            .iter()
            .filter(|p| p.key == "colour")
            .count(),
        1
    );
    assert_eq!(prop("name").envs, ["EX_NAME"]);
}

#[test]
fn the_flag_and_the_environment_reach_the_same_setting() {
    let env = EnvLayer::new([("EX_LISTEN_TLS_CERT".to_string(), "a.pem,b.pem".to_string())]);
    let (_, layer) = parse(&["serve"]);
    let resolved =
        resolve(Ex::SETTINGS_REGISTRY, Layers::new().then(&layer).then(&env)).expect("resolves");
    assert_eq!(
        resolved.get_key("listen.tls.cert"),
        Some(&Value::List(vec![
            Value::from("a.pem"),
            Value::from("b.pem")
        ]))
    );

    let (cli, layer) = parse(&["--listen-tls-cert", "c.pem", "check", "--jobs", "3"]);
    assert_eq!(cli.tls.cert, [PathBuf::from("c.pem")]);
    let resolved =
        resolve(Ex::SETTINGS_REGISTRY, Layers::new().then(&layer).then(&env)).expect("resolves");
    assert_eq!(
        resolved.get_key("listen.tls.cert"),
        Some(&Value::List(vec![Value::from("c.pem")]))
    );
    assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(3)));
}

#[test]
fn the_spec_documents_the_settings_the_flags_declare() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains(r#"prop "listen.tls.cert""#), "{kdl}");
    assert!(kdl.contains(r#"prop "name""#), "{kdl}");
    let _: usage::Spec = kdl.parse().expect("usage-lib reads the whole spec");
}

/// A tool whose only settings are the ones its flags declare
#[derive(Cli, Debug)]
#[usage(bin = "bare")]
struct Bare {
    /// How many jobs to run at once
    #[usage(long, env = "BARE_JOBS", setting = "jobs")]
    jobs: Option<u16>,
}

#[test]
fn a_cli_with_no_config_type_still_documents_its_settings() {
    let kdl = Bare::to_kdl();
    assert!(kdl.contains(r#"prop "jobs""#), "{kdl}");
    let _: usage::Spec = kdl.parse().expect("usage-lib reads the whole spec");
}
