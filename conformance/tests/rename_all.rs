use std::ffi::OsStr;

use usage_derive::{Args, Cli, Subcommands};

fn argv<const N: usize>(tokens: [&str; N]) -> [&OsStr; N] {
    tokens.map(OsStr::new)
}

#[derive(Cli)]
#[usage(bin = "ex", rename_all = "camelCase", rename_all_env = "kebab-case")]
struct Renamed {
    #[usage(long, env)]
    api_token: Option<String>,
    #[usage(env, name = "service_credential", long)]
    credential: Option<String>,
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommands)]
#[usage(rename_all = "SCREAMING_SNAKE_CASE")]
enum Commands {
    ApiServer(ApiServer),
}

#[derive(Args)]
struct ApiServer {
    #[usage(long)]
    ready: bool,
}

#[test]
fn clap_container_casing_reaches_binding_and_the_spec() {
    let parsed = Renamed::parse_from(&argv(["--apiToken", "v", "API_SERVER", "--ready"]))
        .expect("renamed forms should parse");
    assert_eq!(parsed.api_token.as_deref(), Some("v"));
    assert!(parsed.credential.is_none());
    let Some(Commands::ApiServer(server)) = parsed.command else {
        panic!("API_SERVER should select the renamed command");
    };
    assert!(server.ready);

    let kdl = Renamed::to_kdl();
    assert!(kdl.contains("flag --apiToken"), "{kdl}");
    assert!(kdl.contains("arg <APITOKEN>"), "{kdl}");
    assert!(kdl.contains("env=api-token"), "{kdl}");
    assert!(
        kdl.contains("flag --service_credential") && kdl.contains("env=service-credential"),
        "{kdl}"
    );
    assert!(kdl.contains("cmd API_SERVER"), "{kdl}");
}
