//! An MCP server over stdio, serving a usage spec to an agent.
//!
//! The point is `effect`. An agent about to run `pitchfork logs --clear` can
//! ask what that does first and be told it deletes stored logs, without
//! running it and without the spec author writing prose about it.
//!
//! This is deliberately a local server rather than a hosted one: an agent doing
//! real work is in a project, in front of a CLI that is installed, and that is
//! a local question. It also means no hosting cost, no abuse surface, and it
//! works for private and internal CLIs that a public service could never see.
//!
//! Built on `rmcp`, the same as `mise mcp` and `fnox mcp`, so the three behave
//! alike and inherit protocol details — version negotiation, pagination,
//! cancellation, schema generation — rather than each reimplementing a subset.

use std::path::PathBuf;
use std::sync::Arc;

use miette::{bail, IntoDiagnostic, Result};
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters, ServerHandler},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use usage::{Spec, SpecArg, SpecCommand, SpecFlag};

use crate::cli::generate;

/// What the client is told before it sees any command. The three effect values
/// are the reason to point an agent at this, so they are spelled out here
/// rather than left to be inferred from a field name.
const INSTRUCTIONS: &str = "Describes a CLI from its usage spec. Every command, flag and \
argument may carry an `effect`: `read` only inspects state, `write` changes it, \
`destructive` removes something that is work to get back. The effect of an invocation is \
the highest of the command's and those of the flags and arguments given. A missing effect \
means unknown — treat it as needing confirmation, not as safe.";

/// Serve a usage spec over the Model Context Protocol
///
/// Reads JSON-RPC over stdin and writes responses to stdout, which is how MCP
/// clients launch a local server. Point one at `usage mcp -f mycli.usage.kdl`.
#[derive(Debug, clap::Args)]
#[clap(visible_alias = "mcp-server", verbatim_doc_comment)]
pub struct Mcp {
    // Unlike other subcommands this cannot be "-": stdin is the transport, so
    // reading the spec from it would consume the session.
    /// Usage spec file (not "-": stdin is the MCP transport)
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Raw string spec input
    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Mcp {
    pub fn run(&self) -> Result<()> {
        // `-f -` reads stdin to EOF, which is the transport this then wants to
        // serve on. Saying so beats a server that starts and instantly ends.
        if self.file.as_deref().is_some_and(|f| f.as_os_str() == "-") {
            bail!("`--file -` cannot be used with `mcp`: stdin is the MCP transport. Pass a path, or `--spec <text>`.");
        }
        let spec = generate::file_or_spec(&self.file, &self.spec)?;

        // A current-thread runtime: this server is one stdio conversation with
        // no concurrent work, so a thread pool would be cost without use.
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .into_diagnostic()?
            .block_on(async move {
                let service = SpecServer::new(spec)
                    .serve(rmcp::transport::io::stdio())
                    .await
                    .into_diagnostic()?;
                service.waiting().await.into_diagnostic()?;
                Ok(())
            })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListCommandsParams {
    /// Include commands hidden from help. They are still runnable.
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DescribeCommandParams {
    /// Command path without the binary, e.g. "logs" or "daemons remove".
    pub command: String,
}

#[derive(Clone)]
struct SpecServer {
    spec: Arc<Spec>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl SpecServer {
    fn new(spec: Spec) -> Self {
        Self {
            spec: Arc::new(spec),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Every command in the CLI, with its effect. Start here.")]
    async fn list_commands(
        &self,
        Parameters(ListCommandsParams { include_hidden }): Parameters<ListCommandsParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let commands = list_commands(&self.spec, include_hidden);
        Ok(json_result(
            json!({ "bin": self.spec.bin, "commands": commands }),
        ))
    }

    #[tool(
        description = "Full detail for one command: help, flags, arguments, and the effect of each. Use before running an unfamiliar command."
    )]
    async fn describe_command(
        &self,
        Parameters(DescribeCommandParams { command }): Parameters<DescribeCommandParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Hidden commands are described on request even though `list_commands`
        // omits them. An agent that names one already knows it exists; refusing
        // would only mean it runs the thing without knowing the effect. The
        // response says `"hidden": true` so the caller can weigh that.
        match find_chain(&self.spec, &command) {
            Some(chain) => Ok(json_result(describe(&self.spec, &chain))),
            // A command the caller invented is their mistake, not a protocol
            // failure, so it comes back as a tool error they can recover from.
            None => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "no such command: {command:?}. Call list_commands to see what exists."
            ))])),
        }
    }
}

// `router = self.tool_router` uses the router built once in `new`. The macro's
// default is `Self::tool_router()`, which rebuilds it on every request.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for SpecServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            // Without this the server names itself after rmcp, since rmcp's
            // default reads the `CARGO_*` vars of its own crate.
            .with_server_info(Implementation::new("usage", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Structured data, plus the same JSON as text for clients that only read
/// `content` — which the spec asks servers returning structure to do.
fn json_result(value: Value) -> CallToolResult {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    CallToolResult::success(vec![ContentBlock::text(text)])
}

/// Flatten the tree. Hidden commands take their subtree with them, since a
/// visible child of a hidden parent is not a documented path.
fn list_commands(spec: &Spec, include_hidden: bool) -> Vec<Value> {
    fn walk(cmd: &SpecCommand, path: &mut Vec<String>, include_hidden: bool, out: &mut Vec<Value>) {
        for (name, sub) in &cmd.subcommands {
            if sub.hide && !include_hidden {
                continue;
            }
            path.push(name.clone());
            out.push(json!({
                "command": path.join(" "),
                "help": sub.help,
                "effect": sub.effect.map(|e| e.as_str()),
                "hidden": sub.hide,
            }));
            walk(sub, path, include_hidden, out);
            path.pop();
        }
    }
    let mut out = vec![];
    walk(&spec.cmd, &mut vec![], include_hidden, &mut out);
    out
}

/// Resolve a space-separated path, following aliases as a user would.
///
/// Returns the whole chain from the root down to the match, because the
/// ancestors are not decoration: their `global` flags are part of what the
/// command accepts.
fn find_chain<'a>(spec: &'a Spec, path: &str) -> Option<Vec<&'a SpecCommand>> {
    let mut segments = path.split_whitespace().peekable();

    // An agent that has seen the CLI in a shell will write the whole line, and
    // a leading binary name is not a subcommand. Only skipped when the root has
    // no subcommand by that name, so a CLI with a `usage usage` keeps working.
    if segments.peek() == Some(&spec.bin.as_str()) && spec.cmd.find_subcommand(&spec.bin).is_none()
    {
        segments.next();
    }

    let mut chain = vec![&spec.cmd];
    for segment in segments {
        chain.push(chain.last().unwrap().find_subcommand(segment)?);
    }
    // Just the root means the caller named no command, or named only the bin.
    (chain.len() > 1).then_some(chain)
}

/// Every flag the command accepts, including `global` ones from its ancestors.
///
/// Delegated to `usage::available_flags` rather than walked here. usage resolves
/// globals while parsing an invocation instead of copying them onto each
/// subcommand, so `cmd.flags` alone is short by exactly the options an agent
/// reaches for — and the merge rules are subtle enough (a non-global
/// re-declaration of a global's long name keeps the *global's* effect, not the
/// local one) that a second implementation would state the effect wrongly,
/// which is the one thing this server must not do.
fn flags_for(chain: &[&SpecCommand]) -> Vec<Value> {
    usage::available_flags(chain)
        .iter()
        .map(|f| describe_flag(f))
        .collect()
}

fn describe(spec: &Spec, chain: &[&SpecCommand]) -> Value {
    let cmd = chain.last().expect("chain is never empty");
    json!({
        // The path without the binary, which is what `list_commands` emits and
        // what this tool takes back. Prefixing the bin here made the two ends
        // disagree, so a path copied out of one response was rejected by the
        // other. `bin` is reported alongside, as `list_commands` does.
        "command": cmd.full_cmd.join(" "),
        "bin": spec.bin,
        "usage": cmd.usage,
        "help": cmd.help,
        "long_help": cmd.help_long,
        "aliases": cmd.aliases,
        "hidden": cmd.hide,
        "effect": cmd.effect.map(|e| e.as_str()),
        "args": cmd.args.iter().map(describe_arg).collect::<Vec<_>>(),
        "flags": flags_for(chain),
        "subcommands": cmd.subcommands.keys().collect::<Vec<_>>(),
    })
}

fn describe_arg(arg: &SpecArg) -> Value {
    json!({
        "name": arg.name,
        "required": arg.required,
        "variadic": arg.var,
        "help": arg.help,
        "effect": arg.effect.map(|e| e.as_str()),
        "choices": arg.choices.as_ref().map(|c| c.choices.clone()),
    })
}

fn describe_flag(flag: &SpecFlag) -> Value {
    json!({
        "name": flag.name,
        "short": flag.short.iter().map(|c| format!("-{c}")).collect::<Vec<_>>(),
        "long": flag.long.iter().map(|l| format!("--{l}")).collect::<Vec<_>>(),
        "help": flag.help,
        "effect": flag.effect.map(|e| e.as_str()),
        "hidden": flag.hide,
        // Accepted by every subcommand, so it may appear anywhere in the line.
        // There is no separate "inherited" flag on the output: a subcommand
        // that re-declares a global is describing that same global, not a
        // local flag that shadows it.
        "global": flag.global,
        "arg": flag.arg.as_ref().map(describe_arg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"
name "pitchfork"
bin "pitchfork"
flag "-v --verbose" global=#true help="Verbose logging"
flag "-y --yes" global=#true effect="write" help="Skip confirmation"
flag "--not-global" help="Root only"
cmd "logs" effect="read" help="Displays logs" {
    alias "l"
    flag "-c --clear" effect="destructive" help="Delete logs"
    flag "-t --tail" help="Follow"
}
cmd "daemons" help="Manage daemons" {
    flag "-y --yes" help="Shadows the global one"
    cmd "remove" effect="destructive" help="Remove a daemon"
}
cmd "internal" hide=#true {
    cmd "child"
}
cmd "start" help="Runs a daemon"
"#;

    fn spec() -> Spec {
        SPEC.parse().unwrap()
    }

    fn commands(include_hidden: bool) -> Vec<Value> {
        list_commands(&spec(), include_hidden)
    }

    fn paths(include_hidden: bool) -> Vec<String> {
        commands(include_hidden)
            .iter()
            .map(|c| c["command"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn hidden_subtrees_are_excluded_by_default() {
        // A visible child of a hidden parent is not a documented path.
        assert_eq!(paths(false), ["logs", "daemons", "daemons remove", "start"]);
    }

    #[test]
    fn hidden_can_be_included() {
        let all = paths(true);
        assert!(all.contains(&"internal".to_string()));
        assert!(all.contains(&"internal child".to_string()));
    }

    #[test]
    fn commands_carry_their_effect() {
        let all = commands(false);
        let by_path = |p: &str| {
            all.iter()
                .find(|c| c["command"] == p)
                .cloned()
                .unwrap_or_else(|| panic!("no {p}"))
        };
        assert_eq!(by_path("logs")["effect"], "read");
        assert_eq!(by_path("daemons remove")["effect"], "destructive");
        // Unset stays null rather than defaulting to something reassuring.
        assert!(by_path("start")["effect"].is_null());
    }

    fn described(spec: &Spec, path: &str) -> Value {
        describe(spec, &find_chain(spec, path).unwrap())
    }

    fn flag<'a>(out: &'a Value, name: &str) -> &'a Value {
        out["flags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("no flag {name}"))
    }

    #[test]
    fn describe_reports_flag_effects() {
        let spec = spec();
        let out = described(&spec, "logs");
        assert_eq!(out["effect"], "read");
        assert_eq!(out["command"], "logs");
        assert_eq!(out["bin"], "pitchfork");
        assert_eq!(out["aliases"][0], "l");

        let flags = out["flags"].as_array().unwrap();
        let clear = flags.iter().find(|f| f["name"] == "clear").unwrap();
        assert_eq!(clear["effect"], "destructive");
        assert_eq!(clear["long"][0], "--clear");
        assert_eq!(clear["short"][0], "-c");

        // A flag with no effect must not inherit the command's.
        let tail = flags.iter().find(|f| f["name"] == "tail").unwrap();
        assert!(tail["effect"].is_null());
    }

    #[test]
    fn nested_paths_and_aliases_resolve() {
        let spec = spec();
        assert_eq!(
            described(&spec, "daemons remove")["help"],
            "Remove a daemon"
        );
        // `l` is an alias for `logs`, and an agent may well have seen it.
        assert_eq!(described(&spec, "l")["help"], "Displays logs");
        assert!(find_chain(&spec, "nope").is_none());
        assert!(find_chain(&spec, "logs nope").is_none());
        // The root on its own is not a command anyone can be told about.
        assert!(find_chain(&spec, "").is_none());
    }

    #[test]
    fn inherited_global_flags_are_included() {
        // usage resolves globals when parsing an invocation instead of copying
        // them onto each subcommand, so reporting only `cmd.flags` would tell
        // an agent that `--yes` does not exist on a command that accepts it.
        let spec = spec();
        let out = described(&spec, "daemons remove");

        assert_eq!(flag(&out, "verbose")["global"], true);
        // Inherited flags carry their effect, which is the whole point.
        assert_eq!(flag(&out, "yes")["effect"], "write");

        // A root flag that is not global is not accepted down here.
        assert!(!out["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "not-global"));
    }

    #[test]
    fn a_re_declared_global_keeps_the_globals_effect() {
        // `daemons` re-declares `-y --yes` without `global` or an effect. That
        // is the same logical flag, not a local one shadowing it: the parser
        // keeps the global's declaration and unions in the extra aliases. A
        // hand-rolled "nearest wins" would report `effect: null` here, telling
        // an agent that `--yes` is free of consequence when it is not.
        let spec = spec();
        let out = described(&spec, "daemons");
        let yes: Vec<_> = out["flags"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["name"] == "yes")
            .collect();
        assert_eq!(yes.len(), 1, "{yes:?}");
        assert_eq!(yes[0]["effect"], "write");
        assert_eq!(yes[0]["global"], true);
    }

    #[test]
    fn a_local_only_flag_is_reported_as_local() {
        let spec = spec();
        let out = described(&spec, "logs");
        assert_eq!(flag(&out, "tail")["global"], false);
    }

    #[test]
    fn a_path_from_one_tool_is_accepted_by_the_other() {
        // `list_commands` emits paths without the binary. Every one of them
        // must resolve here, or an agent doing the obvious thing — read the
        // list, describe an entry — gets a tool error on a command that exists.
        let spec = spec();
        for row in list_commands(&spec, true) {
            let path = row["command"].as_str().unwrap();
            let out = described(&spec, path);
            assert_eq!(out["command"], path, "round trip failed for {path:?}");
        }
    }

    #[test]
    fn a_leading_binary_name_is_tolerated() {
        // An agent that has seen the CLI in a shell writes the whole line.
        let spec = spec();
        assert_eq!(
            described(&spec, "pitchfork daemons remove")["command"],
            "daemons remove"
        );
        // The bin alone names no command.
        assert!(find_chain(&spec, "pitchfork").is_none());
    }

    #[test]
    fn a_subcommand_sharing_the_binarys_name_still_resolves() {
        // The skip must not eat a real command. `usage usage` is not
        // hypothetical — a CLI that describes itself is exactly this shape.
        let spec: Spec = "bin \"usage\"\ncmd \"usage\" help=\"self\""
            .parse()
            .unwrap();
        assert_eq!(described(&spec, "usage")["help"], "self");
    }

    #[test]
    fn a_hidden_command_can_still_be_described() {
        // `list_commands` omits it, but an agent that names one already knows
        // it exists. Refusing would only mean running it without the effect.
        let spec = spec();
        let out = described(&spec, "internal child");
        assert_eq!(out["command"], "internal child");
    }

    #[test]
    fn the_instructions_explain_what_effect_means() {
        // The client sees these before any command, so they carry the meaning
        // of the field that makes this server worth pointing at.
        for value in ["read", "write", "destructive"] {
            assert!(INSTRUCTIONS.contains(value), "missing {value}");
        }
        assert!(INSTRUCTIONS.contains("confirmation"));
    }

    #[tokio::test]
    async fn tools_are_registered_with_schemas() {
        let server = SpecServer::new(spec());
        let names: Vec<_> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(names.contains(&"list_commands".to_string()), "{names:?}");
        assert!(names.contains(&"describe_command".to_string()), "{names:?}");
    }

    #[tokio::test]
    async fn describing_a_missing_command_is_a_tool_error() {
        let server = SpecServer::new(spec());
        let res = server
            .describe_command(Parameters(DescribeCommandParams {
                command: "nope".into(),
            }))
            .await
            .unwrap();
        assert_eq!(res.is_error, Some(true));
    }

    #[test]
    fn server_info_declares_tools_and_instructions() {
        let info = SpecServer::new(spec()).get_info();
        assert!(info.capabilities.tools.is_some());
        assert_eq!(info.server_info.name, "usage");
        assert!(info.instructions.is_some());
    }
}
