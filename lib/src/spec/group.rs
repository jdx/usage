use std::fmt::Display;

use kdl::{KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::Result;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::is_false;

/// A set of flags that relate to one another as a set.
///
/// Everything here could be written as pairwise [`conflicts`](crate::SpecFlag::conflicts)
/// — for three members, three declarations, and for six, fifteen — except for the part
/// that cannot: "one of these is required" is a statement about the set, and no rule
/// written on an individual flag says it.
///
/// The two properties are clap's, and are read the same way:
///
/// - `multiple` (default `false`) — whether more than one member may be given. The
///   default is what makes a bare group mutual exclusion.
/// - `required` (default `false`) — whether at least one member must be given.
///
/// So the default group is "at most one of these", `required` alone is "exactly one of
/// these", and `multiple` with `required` is "at least one of these".
///
/// Members are named the way every other relationship names a flag — `--long` or `-s` —
/// so a group refers to flags by how they are spelled rather than by an internal id.
#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecGroup {
    /// What this group is called. Used in messages, and it is how a reader tells two
    /// groups apart when a command has several.
    pub name: String,
    /// The flags in the group, as selectors.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
    /// Whether at least one member has to be given.
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
    /// Whether more than one member may be given.
    #[serde(skip_serializing_if = "is_false")]
    pub multiple: bool,
}

impl SpecGroup {
    /// A group named `name` holding `members`, exclusive and not required — the
    /// defaults clap uses.
    pub fn new(
        name: impl Into<String>,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            members: members.into_iter().map(Into::into).collect(),
            required: false,
            multiple: false,
        }
    }

    /// The same, but at least one member must be given.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// The same, but more than one member may be given.
    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut group = SpecGroup::default();
        // The name first, then the members: `group "input" "--file" "--url"`. One node
        // rather than a node and a child list, because a group is short by nature — a
        // set large enough to be unreadable on one line is a set nobody wants to be in.
        let mut args = node.args();
        let Some(name) = args.next() else {
            bail_parse!(ctx, node.span(), "a group needs a name");
        };
        group.name = name.ensure_string()?;
        for arg in args {
            group.members.push(arg.ensure_string()?);
        }
        for (k, v) in node.props() {
            match k {
                "required" => group.required = v.ensure_bool()?,
                "multiple" => group.multiple = v.ensure_bool()?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported group key {k}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "required" => group.required = child.arg(0)?.ensure_bool()?,
                "multiple" => group.multiple = child.arg(0)?.ensure_bool()?,
                // The child spelling for members, for a group whose selectors do not fit
                // comfortably on the node itself.
                "flag" => {
                    for arg in child.args() {
                        group.members.push(arg.ensure_string()?);
                    }
                }
                k => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported group value key {k}"
                ),
            }
        }
        if group.name.is_empty() {
            bail_parse!(ctx, node.span(), "a group needs a name");
        }
        // A group of one is a flag, and a group of none is nothing at all. Both are
        // almost certainly a mistake in the writing rather than an intention, and
        // neither can be enforced into meaning anything.
        if group.members.len() < 2 {
            bail_parse!(
                ctx,
                node.span(),
                "group {} needs at least two flags; a rule about one flag belongs on that flag",
                group.name
            );
        }
        Ok(group)
    }

    pub fn usage(&self) -> String {
        format!("group:{}", self.name)
    }
}

impl From<&SpecGroup> for KdlNode {
    fn from(group: &SpecGroup) -> KdlNode {
        let mut node = KdlNode::new("group");
        node.push(string_entry(None, &group.name));
        for member in &group.members {
            node.push(string_entry(None, member));
        }
        if group.required {
            node.push(KdlEntry::new_prop("required", true));
        }
        if group.multiple {
            node.push(KdlEntry::new_prop("multiple", true));
        }
        node
    }
}

impl Display for SpecGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.usage())
    }
}

#[cfg(test)]
mod tests {
    use crate::Spec;

    #[test]
    fn a_group_round_trips_through_kdl() {
        let spec: Spec = "flag \"--file <f>\"\nflag \"--url <u>\"\ngroup \"input\" \"--file\" \"--url\" required=#true\n"
            .parse()
            .unwrap();
        let group = &spec.cmd.groups[0];
        assert_eq!(group.name, "input");
        assert_eq!(
            group.members,
            vec!["--file".to_string(), "--url".to_string()]
        );
        assert!(group.required);
        assert!(!group.multiple);

        let reparsed: Spec = spec.to_string().parse().unwrap();
        let group = &reparsed.cmd.groups[0];
        assert_eq!(group.name, "input", "{spec}");
        assert_eq!(group.members.len(), 2, "{spec}");
        assert!(group.required, "{spec}");
    }

    #[test]
    fn a_group_of_fewer_than_two_flags_is_refused() {
        // A group of one is a rule about a flag, which belongs on the flag; a group of
        // none is nothing at all. Both are a slip in the writing rather than a shape
        // anyone means, and neither enforces anything, so they are refused where they
        // are written rather than silently doing nothing at run time.
        // The message is on the diagnostic's label rather than in `Display`, which
        // renders every parse failure as "Invalid usage config" — so it is read the way
        // the rest of the spec tests read one.
        let err = "flag \"--file <f>\"\ngroup \"input\" \"--file\"\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("at least two"), "{err:?}");

        let err = "group \"input\"\n".parse::<Spec>().unwrap_err();
        assert!(format!("{err:?}").contains("at least two"), "{err:?}");
    }

    #[test]
    fn a_group_comes_across_from_clap() {
        // Unlike `requires`, this one the bridge can read: `Command::get_groups` and
        // `ArgGroup::get_args` are public, so a clap CLI's groups reach the spec — and
        // every spec generated from a clap command was losing them before this.
        let cmd = clap::Command::new("ex")
            .arg(clap::Arg::new("file").long("file"))
            .arg(clap::Arg::new("url").long("url"))
            .group(
                clap::ArgGroup::new("input")
                    .args(["file", "url"])
                    .required(true),
            );
        let spec = Spec::from(&cmd);
        let group = spec
            .cmd
            .groups
            .iter()
            .find(|g| g.name == "input")
            .expect("the group should have come across");
        assert_eq!(
            group.members,
            vec!["--file".to_string(), "--url".to_string()]
        );
        assert!(group.required);
        assert!(!group.multiple);
    }

    #[test]
    fn a_clap_group_naming_a_positional_keeps_the_flags_it_can_name() {
        // The spec names group members the way it names every other relationship, by
        // flag, so a positional member has no spelling here. Dropping it silently would
        // leave a group that enforces less than clap does — but writing `--<name>` for
        // it would be a selector matching nothing, which reads as a rule that holds and
        // enforces even less. The flags it can name still make a group.
        let cmd = clap::Command::new("ex")
            .arg(clap::Arg::new("file").long("file"))
            .arg(clap::Arg::new("url").long("url"))
            .arg(clap::Arg::new("target"))
            .group(clap::ArgGroup::new("input").args(["file", "url", "target"]));
        let spec = Spec::from(&cmd);
        let group = spec.cmd.groups.iter().find(|g| g.name == "input").unwrap();
        assert_eq!(
            group.members,
            vec!["--file".to_string(), "--url".to_string()]
        );
    }
}
