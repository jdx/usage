//! What a value is called in help, and which `usage` can read the spec.
//!
//! Both measured against clap 4 rather than remembered, because the whole point of matching it
//! is that an adopter's users read the same thing they read before:
//!
//! ```text
//! Usage: ex [OPTIONS] <TAG> [PREV_TAG]
//!
//! Arguments:
//!   <TAG>
//!   [PREV_TAG]
//!
//! Options:
//!       --type <TYPE>
//!       --max-tokens <MAX_TOKENS>
//!   -c, --config <CONFIG>
//! ```

use usage::Spec as LibSpec;
use usage_derive::Cli;

/// A tool whose values are named after their fields
#[derive(Cli)]
#[usage(bin = "ex", min_usage_version = "4.0")]
struct Ex {
    /// The tag to work on
    #[usage(arg)]
    tag: String,
    /// The one before it
    #[usage(arg)]
    prev_tag: Option<String>,
    /// What sort of thing
    #[usage(long = "type", short = 't')]
    type_: Option<String>,
    /// How many at most
    #[usage(long)]
    max_tokens: Option<u32>,
    /// Where the config is
    #[usage(long, short)]
    config: Option<String>,
    /// Named by hand, which still wins
    #[usage(long, value_name = "PATH")]
    out: Option<String>,
    /// A switch has no value to name
    #[usage(long)]
    quiet: bool,
}

#[test]
fn a_value_is_named_after_its_field_shouted() {
    let spec: LibSpec = Ex::to_kdl().parse().expect("valid spec");
    let value_of = |flag: &str| {
        spec.cmd
            .flags
            .iter()
            .find(|f| f.name == flag)
            .unwrap_or_else(|| panic!("{flag}"))
            .arg
            .as_ref()
            .map(|a| a.name.clone())
    };

    // Underscores kept, not the kebab the flag itself gets: one field written two ways, which
    // is what clap prints.
    assert_eq!(value_of("max-tokens").as_deref(), Some("MAX_TOKENS"));
    assert_eq!(value_of("config").as_deref(), Some("CONFIG"));

    // From the *form*, so a field called `type_` does not drag its ident in as `TYPE_`.
    assert_eq!(value_of("type").as_deref(), Some("TYPE"));

    // Said by hand, and that still wins — the default is only a default.
    assert_eq!(value_of("out").as_deref(), Some("PATH"));

    // A switch has no value, so there is no placeholder to invent for it.
    assert_eq!(value_of("quiet"), None);

    // A positional's name *is* its placeholder, and follows the same rule.
    assert_eq!(spec.cmd.args[0].name, "TAG");
    assert_eq!(spec.cmd.args[1].name, "PREV_TAG");
}

#[test]
fn the_help_line_reads_the_way_clap_writes_it() {
    // The rule exists for this: what a user sees. Asserted through the rendered line rather
    // than the tables, because a spec that says `MAX_TOKENS` and a help that prints
    // `max-tokens` would pass every assertion above.
    let line = usage_argv::help::usage_line(&["ex"], Ex::spec().root);
    assert!(line.contains("<TAG>"), "{line}");
    assert!(line.contains("[PREV_TAG]"), "{line}");

    let page = usage_argv::help::render(Ex::spec(), Ex::spec().root.cmd, false).expect("a page");
    assert!(page.contains("--max-tokens <MAX_TOKENS>"), "{page}");
    assert!(page.contains("--type <TYPE>"), "{page}");
    assert!(page.contains("--out <PATH>"), "{page}");
}

#[test]
fn a_spec_can_say_which_usage_can_read_it() {
    // Declared, not worked out: computing it would mean a table from every property to the
    // version that introduced it, kept in step by hand, and a table that rots silently claims
    // a spec is readable by a `usage` that chokes on it.
    let kdl = Ex::to_kdl();
    assert!(
        kdl.starts_with("min_usage_version \"4.0\"\n"),
        "it goes first, before anything an old `usage` would choke on: {kdl}"
    );

    // And it survives the trip out and back, which is the only thing it is for.
    let spec: LibSpec = kdl.parse().expect("valid spec");
    assert_eq!(spec.min_usage_version.as_deref(), Some("4.0"));
}

#[test]
fn the_fields_are_bound() {
    use std::ffi::OsStr;
    let argv = [
        "t",
        "p",
        "--type",
        "toml",
        "--max-tokens",
        "4",
        "-c",
        "c.toml",
        "--out",
        "o",
        "--quiet",
    ]
    .map(OsStr::new);
    let ex = Ex::parse_from(&argv).expect("should parse");
    assert_eq!(ex.tag, "t");
    assert_eq!(ex.prev_tag.as_deref(), Some("p"));
    assert_eq!(ex.type_.as_deref(), Some("toml"));
    assert_eq!(ex.max_tokens, Some(4));
    assert_eq!(ex.config.as_deref(), Some("c.toml"));
    assert_eq!(ex.out.as_deref(), Some("o"));
    assert!(ex.quiet);
}
