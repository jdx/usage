//! Environment variables under a CLI's own prefix that nothing declares.
//!
//! Two implementations answer "which variables does this CLI read": `usage_argv::env`, over the
//! compiled tables a derive emits, and `usage::SpecUnknownEnv`, over a parsed spec. They are held
//! to one answer here by running both on the same CLI — the derive's tables, and the KDL that
//! derive emits.
//!
//! Its own test binary, like `post_binding_env`: the derive reads the process environment, so a
//! test that sets a variable races every other test in the binary that parses a CLI.

use std::collections::HashMap;
use std::ffi::OsStr;

use usage_argv::Error;
use usage_derive::{Args, Cli, Config, Subcommands};

/// The settings, whose variables count as known too.
#[derive(Config)]
#[expect(dead_code, reason = "only the registry this derives is read here")]
struct Settings {
    /// How many jobs to run at once
    #[usage(env = "EXU_JOBS", deprecated_env = "EXU_JOBS_OLD")]
    jobs: Option<u32>,
}

/// Network options, flattened into a subcommand.
#[derive(Args)]
struct Net {
    /// Where to listen
    #[usage(long, env = "EXU_PORT", env_fallback = "EXU_LISTEN_PORT")]
    port: Option<u16>,
}

#[derive(Args)]
struct Serve {
    #[usage(flatten)]
    net: Net,
    /// Serve over TLS
    #[usage(long, env = "EXU_TLS", deprecated_env = "EXU_SSL")]
    tls: bool,
}

#[derive(Args)]
struct Publish {
    /// Where to publish
    #[usage(env = "EXU_TARGET")]
    target: Option<String>,
}

#[derive(Subcommands)]
enum Commands {
    /// Serve it
    Serve(Serve),
    /// Publish it
    Publish(Publish),
}

/// A CLI that owns every variable under its prefix
#[derive(Cli)]
#[usage(
    bin = "exu",
    config = Settings,
    unknown_env(prefix = "EXU_", allow("EXU_LIBRARY", "EXU_CODEC"))
)]
struct Exu {
    /// Say more
    #[usage(long, env = "EXU_VERBOSE", setting = "verbose")]
    verbose: bool,
    #[usage(subcommand)]
    command: Commands,
}

/// Every name the CLI reads, by kind: current, fallback, deprecated, flattened, a sibling
/// subcommand's, a settings property's, and the allow list.
const KNOWN: &[&str] = &[
    "EXU_VERBOSE",
    "EXU_PORT",
    "EXU_LISTEN_PORT",
    "EXU_TLS",
    "EXU_SSL",
    "EXU_TARGET",
    "EXU_JOBS",
    "EXU_JOBS_OLD",
    "EXU_LIBRARY",
    "EXU_CODEC",
];

fn lib_unknown(names: &[&str]) -> Vec<String> {
    let spec: usage::Spec = Exu::to_kdl()
        .parse()
        .expect("usage-lib reads the emitted spec");
    let decl = spec
        .unknown_env
        .as_ref()
        .expect("the emitted spec carries unknown_env");
    decl.unknown(&spec, names.iter().map(OsStr::new))
}

fn argv_unknown(names: &[&str]) -> Vec<String> {
    let mut known = vec!["EXU_LIBRARY", "EXU_CODEC"];
    known.extend(
        Settings::SETTINGS_PROPS
            .iter()
            .flat_map(|p| p.envs.iter().chain(p.deprecated_envs.iter()))
            .copied(),
    );
    usage_argv::env::unknown(
        Exu::spec().root,
        "EXU_",
        &known,
        names.iter().map(OsStr::new),
    )
}

#[test]
fn both_implementations_know_the_same_names() {
    let mut env = KNOWN.to_vec();
    env.extend(["PATH", "EXUBERANT", "EX_PORT"]);
    if cfg!(windows) {
        // Windows variable names are case-insensitive, so this is `EXU_PORT`.
        env.push("EXU_Port");
    }
    assert_eq!(argv_unknown(&env), Vec::<String>::new());
    assert_eq!(lib_unknown(&env), Vec::<String>::new());

    let typos = ["EXU_PROT", "EXU_TSL", "EXU_PORT", "EXU_PROT"];
    let want = ["EXU_PROT", "EXU_TSL"];
    assert_eq!(argv_unknown(&typos), want);
    assert_eq!(lib_unknown(&typos), want);
}

#[test]
fn the_emitted_spec_round_trips() {
    let kdl = Exu::to_kdl();
    assert!(
        kdl.contains("unknown_env prefix=\"EXU_\" {\n    allow \"EXU_LIBRARY\" \"EXU_CODEC\"\n}\n"),
        "{kdl}"
    );
    let spec: usage::Spec = kdl.parse().unwrap();
    let decl = spec.unknown_env.as_ref().unwrap();
    assert_eq!(decl.prefix, "EXU_");
    assert_eq!(decl.allow, ["EXU_LIBRARY", "EXU_CODEC"]);
}

#[test]
fn a_parse_refuses_every_unknown_variable_at_once() {
    // One test, in one process, so the order of these is the order they read.
    let argv = [OsStr::new("publish")];

    // A variable for `serve` is not a typo while running `publish`, and a deprecated one is
    // not unknown either.
    unsafe {
        std::env::set_var("EXU_PORT", "8080");
        std::env::set_var("EXU_SSL", "1");
        std::env::set_var("EXU_LIBRARY", "x");
        std::env::set_var("EXU_JOBS_OLD", "2");
    }
    Exu::parse_from(&argv).expect("every variable is declared");

    // The deprecated name still warns as one, on the command that reads it.
    let serve = [OsStr::new("serve")];
    let mut warnings = Vec::new();
    Exu::parse_from_with_warnings(&serve, &mut warnings).expect("every variable is declared");
    assert!(
        warnings
            .iter()
            .any(|w| w.kind == usage_argv::warn::WarningKind::DeprecatedEnv && w.name == "EXU_SSL"),
        "{warnings:?}"
    );

    unsafe {
        std::env::set_var("EXU_PROT", "8080");
        std::env::set_var("EXU_TSL", "1");
    }
    let err = Exu::parse_from(&argv).err().expect("two typos");
    let Error::UnknownEnv(unknown) = &err else {
        panic!("expected UnknownEnv, got {err:?}");
    };
    assert_eq!(unknown.prefix, "EXU_");
    assert_eq!(unknown.names, ["EXU_PROT", "EXU_TSL"]);
    assert_eq!(
        Exu::render_failure(&argv, &err),
        "error: unknown environment variables under 'EXU_':\n  EXU_PROT\n  EXU_TSL\n\nFor more information, try '--help'.\n"
    );

    // Help is still help: the user asking how to fix it gets the page.
    let help = [OsStr::new("--help")];
    assert!(matches!(Exu::parse_from(&help), Err(Error::Help { .. })));

    // usage-lib says the same about the same environment.
    let spec: usage::Spec = Exu::to_kdl().parse().unwrap();
    // `vars` would panic on a variable the test process inherited that is not UTF-8.
    let env: HashMap<String, String> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let err = usage::Parser::new(&spec)
        .with_env(env)
        .parse(&["exu".to_string(), "publish".to_string()])
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Unknown environment variables under EXU_: EXU_PROT, EXU_TSL"
    );
}
