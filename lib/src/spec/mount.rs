use std::fmt::Display;

use crate::kdl::{KdlEntry, KdlNode};
use serde::Serialize;

use crate::error::Result;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{string_entry, NodeHelper};

#[derive(Debug, Default, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecMount {
    pub run: String,
    /// Display-only arguments for this unresolved mount; never executes discovery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synopsis: Option<String>,
    /// Whether a discovered command may take precedence over
    /// [`Spec::default_subcommand`](crate::Spec::default_subcommand).
    ///
    /// Off by default, because resolving a mount runs a process: with a default
    /// subcommand declared, every word that is not a known command would otherwise
    /// pay for discovery before falling back — for a task runner, that is a
    /// subprocess per task invocation. Turn it on when a discovered command should
    /// win, and accept the cost.
    pub overrides_default: bool,
}

impl SpecMount {
    /// A mount that runs `run` to produce a spec for the subcommands here.
    pub fn new(run: impl Into<String>) -> Self {
        Self {
            run: run.into(),
            synopsis: None,
            overrides_default: false,
        }
    }

    /// The same, but a discovered command outranks the default subcommand.
    pub fn overriding_default(run: impl Into<String>) -> Self {
        Self {
            run: run.into(),
            synopsis: None,
            overrides_default: true,
        }
    }
}

impl SpecMount {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self> {
        let mut mount = SpecMount::default();
        for (k, v) in node.props() {
            match k {
                "run" => mount.run = v.ensure_string()?,
                "synopsis" => mount.synopsis = Some(v.ensure_string()?),
                "overrides_default" => mount.overrides_default = v.ensure_bool()?,
                k => bail_parse!(ctx, v.entry.span(), "unsupported mount key {k}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "run" => mount.run = child.arg(0)?.ensure_string()?,
                "synopsis" => mount.synopsis = Some(child.arg(0)?.ensure_string()?),
                "overrides_default" => mount.overrides_default = child.arg(0)?.ensure_bool()?,
                k => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported mount value key {k}"
                ),
            }
        }
        if mount.run.is_empty() {
            bail_parse!(ctx, node.span(), "mount run is required")
        }
        Ok(mount)
    }
    pub fn usage(&self) -> String {
        format!("mount:{}", self.run)
    }
}

impl From<&SpecMount> for KdlNode {
    fn from(mount: &SpecMount) -> KdlNode {
        let mut node = KdlNode::new("mount");
        node.push(string_entry(Some("run"), &mount.run));
        if let Some(synopsis) = &mount.synopsis {
            node.push(string_entry(Some("synopsis"), synopsis));
        }
        if mount.overrides_default {
            node.push(KdlEntry::new_prop("overrides_default", true));
        }
        node
    }
}

impl Display for SpecMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.usage())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unresolved_mount_synopsis_round_trips_and_renders_without_discovery() {
        let spec: crate::Spec = "bin ex\ncmd run {\n mount run=\"this-command-must-never-run\" synopsis=\"[TASK] [ARGS]…\"\n}\n".parse().unwrap();
        let run = &spec.cmd.subcommands["run"];
        assert_eq!(run.usage, "run [TASK] [ARGS]…");
        let again: crate::Spec = spec.to_string().parse().unwrap();
        assert_eq!(
            again.cmd.subcommands["run"].mounts[0].synopsis.as_deref(),
            Some("[TASK] [ARGS]…")
        );
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(
            json["cmd"]["subcommands"]["run"]["usage"],
            "run [TASK] [ARGS]…"
        );
        #[cfg(feature = "markdown")]
        assert!(crate::docs::markdown::MarkdownRenderer::new(spec.clone())
            .render_cmd(run)
            .unwrap()
            .contains("run [TASK] [ARGS]…"));
        #[cfg(feature = "cli-help")]
        assert!(crate::docs::cli::render_help(&spec, run, true).contains("run [TASK] [ARGS]…"));
    }

    #[test]
    fn subcommand_required_controls_custom_placeholder() {
        for (required, expected) in [(true, "<ACTION>"), (false, "[ACTION]")] {
            let source = format!(
                "bin ex\nsubcommand_required #{required}\nsubcommand_value_name ACTION\ncmd go\n"
            );
            let spec: crate::Spec = source.parse().unwrap();
            assert_eq!(spec.cmd.usage, expected);
        }
        let spec: crate::Spec = "bin ex\nmount run=\"never-run\"\n".parse().unwrap();
        assert_eq!(spec.cmd.usage, "");
    }
}
