use crate::kdl::{KdlEntry, KdlNode};
use serde::{Deserialize, Serialize};

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};
use crate::spec::is_false;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpecComplete {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub descriptions: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// A command whose own shell completion answers for this argument.
    ///
    /// Written as a command line (`"terraform"`, `"kubectl --context prod"`). Completing a
    /// word of the argument asks the user's shell what it would offer after that command
    /// line followed by the words the argument already holds, so a wrapper passes the
    /// wrapped tool's completion through unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<String>,
}

impl SpecComplete {
    /// A completer for the arg or flag named `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Shell command whose output supplies the completions.
    pub fn run(mut self, run: impl Into<String>) -> Self {
        self.run = Some(run.into());
        self
    }

    /// Command line whose own shell completion answers for this argument.
    pub fn delegate(mut self, delegate: impl Into<String>) -> Self {
        self.delegate = Some(delegate.into());
        self
    }

    /// Whether the completer emits descriptions alongside values.
    pub fn descriptions(mut self, descriptions: bool) -> Self {
        self.descriptions = descriptions;
        self
    }
}

impl SpecComplete {
    /// A top-level or command-level `complete "<name>" ...` node, which finds the arg or flag
    /// value it completes by name.
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        node.ensure_arg_len(1..=1)?;
        let name = node.arg(0)?.ensure_string()?.to_string().to_lowercase();
        let mut config = Self::parse_props(ctx, node)?;
        config.name = name;
        Ok(config)
    }

    /// A `complete run=...` node written inside the `arg` it completes. It takes no name: the
    /// enclosing arg is what is being completed, so the caller fills in [`SpecComplete::name`]
    /// once the arg's own name is settled.
    pub(crate) fn parse_inline(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        if let Some(name) = node.args().next() {
            bail_parse!(
                ctx,
                name.entry.span(),
                "a complete inside an arg completes that arg, so it takes no name"
            )
        }
        Self::parse_props(ctx, node)
    }

    fn parse_props(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        let mut config = Self::default();
        for (k, v) in node.props() {
            match k {
                "run" | "type" | "delegate"
                    if config.run.is_some()
                        || config.type_.is_some()
                        || config.delegate.is_some() =>
                {
                    bail_parse!(
                        ctx,
                        v.entry.span(),
                        "can set only one of run, type or delegate"
                    )
                }
                "run" => config.run = Some(v.ensure_string()?.to_string()),
                "descriptions" => config.descriptions = v.ensure_bool()?,
                "type" => config.type_ = Some(v.ensure_string()?.to_string()),
                "delegate" => {
                    let delegate = v.ensure_string()?;
                    // Split here rather than at completion time, so a typo in the quoting is
                    // reported where the spec is read instead of as a Tab that does nothing.
                    match crate::shell_words::split(&delegate) {
                        Ok(words) if !words.is_empty() => {}
                        Ok(_) => bail_parse!(ctx, v.entry.span(), "delegate names no command"),
                        Err(err) => {
                            bail_parse!(
                                ctx,
                                v.entry.span(),
                                "delegate is not a command line: {err}"
                            )
                        }
                    }
                    config.delegate = Some(delegate)
                }
                k => bail_parse!(ctx, v.entry.span(), "unsupported complete key {k}"),
            }
        }
        Ok(config)
    }

    /// The node written inside an `arg`: the named form's properties without the name, since
    /// the enclosing arg says what is being completed.
    pub(crate) fn to_inline_node(&self) -> KdlNode {
        let mut node = KdlNode::new("complete");
        self.push_props(&mut node);
        node
    }

    fn push_props(&self, node: &mut KdlNode) {
        if let Some(run) = &self.run {
            node.push(string_entry(Some("run"), run));
        }
        if let Some(type_) = &self.type_ {
            node.push(string_entry(Some("type"), type_));
        }
        if let Some(delegate) = &self.delegate {
            node.push(string_entry(Some("delegate"), delegate));
        }
        if self.descriptions {
            node.push(KdlEntry::new_prop("descriptions", true));
        }
    }
}

impl From<&SpecComplete> for KdlNode {
    fn from(complete: &SpecComplete) -> Self {
        let mut node = KdlNode::new("complete");
        node.push(string_entry(None, &complete.name));
        complete.push_props(&mut node);
        node
    }
}

#[cfg(test)]
mod tests {
    use crate::Spec;

    #[test]
    fn delegate_round_trips() {
        let spec: Spec = r#"
arg "<layer>"
arg "<command>" var=#true
complete "command" delegate="terraform -chdir=infra"
"#
        .parse()
        .unwrap();
        let complete = &spec.complete["command"];
        assert_eq!(complete.delegate.as_deref(), Some("terraform -chdir=infra"));
        assert!(complete.run.is_none() && complete.type_.is_none());

        let printed = spec.to_string();
        assert!(
            printed.contains(r#"complete command delegate="terraform -chdir=infra""#),
            "{printed}"
        );
        let reparsed: Spec = printed.parse().unwrap();
        assert_eq!(
            reparsed.complete["command"].delegate.as_deref(),
            Some("terraform -chdir=infra")
        );
    }

    #[test]
    fn delegate_excludes_run_and_type() {
        for spec in [
            r#"complete "c" delegate="git" run="ls""#,
            r#"complete "c" run="ls" delegate="git""#,
            r#"complete "c" type="file" delegate="git""#,
            r#"complete "c" delegate="git" type="file""#,
            r#"complete "c" run="ls" type="file""#,
        ] {
            let err = spec.parse::<Spec>().unwrap_err();
            assert!(
                format!("{err:?}").contains("can set only one of run, type or delegate"),
                "{spec}: {err:?}"
            );
        }
    }

    #[test]
    fn delegate_must_be_a_command_line() {
        for (spec, expected) in [
            (r#"complete "c" delegate="""#, "delegate names no command"),
            (
                r#"complete "c" delegate="git 'oops""#,
                "delegate is not a command line",
            ),
        ] {
            let err = spec.parse::<Spec>().unwrap_err();
            assert!(format!("{err:?}").contains(expected), "{spec}: {err:?}");
        }
    }
}
