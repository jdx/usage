#![allow(dead_code)]

use usage_argv::help;
use usage_derive::{Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "guide")]
struct Guide {
    /// Configuration file
    #[usage(long, help_heading = "Configuration")]
    config: Option<String>,

    /// Runtime profile
    #[usage(arg, help_heading = "Configuration")]
    profile: Option<String>,

    /// Internal switch
    #[usage(long, hide, help_heading = "Internals")]
    internal: bool,

    #[usage(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommands)]
enum Command {
    /// Clear cached data
    #[usage(help_heading = "Maintenance")]
    Clean,
    /// Print current state
    Status,
}

#[test]
fn visible_help_groups_and_standard_sections_are_addressable() {
    let topics = help::topics(Guide::spec(), Guide::command(), true).expect("the root command");
    let listed: Vec<_> = topics
        .iter()
        .map(|topic| (topic.id.as_str(), topic.title.as_str()))
        .collect();
    assert!(listed.contains(&("commands", "Commands")), "{listed:?}");
    assert!(
        listed.contains(&("maintenance", "Maintenance")),
        "{listed:?}"
    );
    assert!(
        listed.contains(&("configuration", "Configuration")),
        "{listed:?}"
    );
    assert!(!listed.iter().any(|(_, title)| *title == "Internals"));
}

#[test]
fn one_topic_combines_argument_and_flag_groups_with_the_same_heading() {
    let topic = help::render_topic(Guide::spec(), Guide::command(), "configuration", true)
        .expect("the declared heading");
    assert!(topic.starts_with("Configuration:\n"), "{topic}");
    assert!(topic.contains("--config <CONFIG>"), "{topic}");
    assert!(topic.contains("[PROFILE]"), "{topic}");
    assert!(!topic.contains("--internal"), "{topic}");
    assert!(!topic.contains("clean"), "{topic}");
    assert_eq!(topic.matches("Configuration:").count(), 1, "{topic}");

    let by_title = help::render_topic(Guide::spec(), Guide::command(), "Configuration", true);
    assert_eq!(by_title.as_deref(), Some(topic.as_str()));
}

#[test]
fn a_topic_that_is_not_on_the_page_is_absent() {
    assert!(help::render_topic(Guide::spec(), Guide::command(), "internals", false).is_none());
}
