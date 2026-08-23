//! Audience and availability annotations remain descriptive metadata.

use std::ffi::OsStr;

use usage::Spec as LibSpec;
use usage_derive::{Args, Cli, Subcommands};

#[derive(Args)]
struct Doctor {
    /// Emit a machine-readable report
    #[usage(long, surface = "automation", available_if("json-feature"))]
    json: bool,

    /// Optional state directory
    #[usage(surface = "advanced", available_if("filesystem", "writable-state"))]
    state: Option<String>,
}

#[derive(Subcommands)]
enum Command {
    /// Inspect internal state
    #[usage(surface = "internal", available_if("debug-build"))]
    Doctor(Doctor),
}

/// A CLI with explicitly described contract surfaces.
#[derive(Cli)]
#[usage(
    bin = "surface-demo",
    surface = "public",
    available_if("supported-platform")
)]
struct SurfaceDemo {
    #[usage(subcommand)]
    command: Option<Command>,
}

#[test]
fn metadata_reaches_the_portable_spec() {
    let kdl = SurfaceDemo::to_kdl();
    let spec: LibSpec = kdl.parse().expect("valid generated spec");

    assert_eq!(spec.cmd.surface.as_deref(), Some("public"));
    assert_eq!(spec.cmd.available_if, ["supported-platform"]);

    let doctor = spec.cmd.subcommands.get("doctor").expect("doctor command");
    assert_eq!(doctor.surface.as_deref(), Some("internal"));
    assert_eq!(doctor.available_if, ["debug-build"]);

    let json = doctor
        .flags
        .iter()
        .find(|flag| flag.name == "json")
        .unwrap();
    assert_eq!(json.surface.as_deref(), Some("automation"));
    assert_eq!(json.available_if, ["json-feature"]);

    let state = doctor
        .args
        .iter()
        .find(|arg| arg.name.eq_ignore_ascii_case("state"))
        .unwrap();
    assert_eq!(state.surface.as_deref(), Some("advanced"));
    assert_eq!(state.available_if, ["filesystem", "writable-state"]);

    let round_trip: LibSpec = spec.to_string().parse().expect("round-tripped spec");
    assert_eq!(round_trip.cmd.surface, spec.cmd.surface);
    assert_eq!(round_trip.cmd.available_if, spec.cmd.available_if);
}

#[test]
fn annotations_do_not_change_parsing_or_help_visibility() {
    let argv = ["doctor", "--json", "state-dir"].map(OsStr::new);
    let parsed = SurfaceDemo::parse_from(&argv).expect("annotated declarations still parse");
    let Some(Command::Doctor(doctor)) = parsed.command else {
        panic!("expected doctor")
    };
    assert!(doctor.json);
    assert_eq!(doctor.state.as_deref(), Some("state-dir"));

    let help = usage_argv::help::render(SurfaceDemo::spec(), SurfaceDemo::command(), false)
        .expect("root help");
    assert!(help.contains("doctor"), "{help}");
}
