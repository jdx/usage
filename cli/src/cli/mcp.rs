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
//! The protocol is JSON-RPC 2.0 over newline-delimited stdio, and the surface
//! a read-only server needs is three methods — `initialize`, `tools/list`,
//! `tools/call`. That is small enough to implement directly, which avoids
//! pulling an async runtime into a synchronous CLI.
//!
//! Spec: https://modelcontextprotocol.io/specification/2025-06-18

use std::io::{BufRead, Write};
use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};
use serde_json::{json, Value};
use usage::{Spec, SpecArg, SpecCommand, SpecFlag};

use crate::cli::generate;

/// The revision this implements. If a client asks for a different one it is
/// told this instead, which the spec allows and clients handle.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Serve a usage spec over the Model Context Protocol
///
/// Reads JSON-RPC over stdin and writes responses to stdout, which is how MCP
/// clients launch a local server. Point one at `usage mcp -f mycli.usage.kdl`.
#[derive(Debug, clap::Args)]
#[clap(visible_alias = "mcp-server", verbatim_doc_comment)]
pub struct Mcp {
    /// Usage spec file, or "-" to read the spec from stdin before serving
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Raw string spec input
    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,
}

impl Mcp {
    pub fn run(&self) -> Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        serve(&spec, stdin.lock(), &mut stdout)
    }
}

/// Read requests until the input closes, which is how a client shuts us down.
fn serve(spec: &Spec, input: impl BufRead, output: &mut impl Write) -> Result<()> {
    for line in input.lines() {
        let line = line.into_diagnostic()?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_line(spec, &line) {
            writeln!(output, "{response}").into_diagnostic()?;
            output.flush().into_diagnostic()?;
        }
    }
    Ok(())
}

/// Handle one message. Returns `None` for notifications, which take no reply.
fn handle_line(spec: &Spec, line: &str) -> Option<String> {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        // No id to reply against, so this is the one case that answers with a
        // null id, as JSON-RPC prescribes for an unparseable request.
        Err(e) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("parse error: {e}"),
            ))
        }
    };

    let method = req
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = req.get("id").cloned();

    // A notification has no id and must not be answered, `notifications/
    // initialized` being the one every client sends.
    let Some(id) = id else { return None };

    let response = match method {
        "initialize" => success(id, initialize_result()),
        "ping" => success(id, json!({})),
        "tools/list" => success(id, json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(spec, id, req.get("params")),
        _ => error_response(id, -32601, &format!("unknown method: {method}")),
    };
    Some(response)
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "usage",
            "title": "Usage CLI spec",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "Describes a CLI from its usage spec. Every command, \
    flag and argument may carry an `effect`: `read` only inspects state, `write` \
    changes it, `destructive` removes something that is work to get back. The \
    effect of an invocation is the highest of the command's and those of the \
    flags and arguments given. A missing effect means unknown — treat it as \
    needing confirmation, not as safe.",
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_commands",
            "title": "List commands",
            "description": "Every command in the CLI, with its effect. Start here.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "include_hidden": {
                        "type": "boolean",
                        "description": "Include commands hidden from help. They are still runnable.",
                    }
                },
            },
        },
        {
            "name": "describe_command",
            "title": "Describe a command",
            "description": "Full detail for one command: help, flags, arguments, \
    and the effect of each. Use before running an unfamiliar command.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command path without the binary, e.g. \"logs\" or \"daemons remove\".",
                    }
                },
                "required": ["command"],
            },
        },
    ])
}

fn call_tool(spec: &Spec, id: Value, params: Option<&Value>) -> String {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = params.and_then(|p| p.get("arguments"));

    match name {
        "list_commands" => {
            let include_hidden = args
                .and_then(|a| a.get("include_hidden"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let commands = list_commands(spec, include_hidden);
            success(
                id,
                tool_result(json!({ "bin": spec.bin, "commands": commands })),
            )
        }
        "describe_command" => {
            let path = args
                .and_then(|a| a.get("command"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            match find_command(spec, path) {
                Some(cmd) => success(id, tool_result(describe(spec, cmd))),
                // A command the caller made up is their error, not a protocol
                // failure, so it comes back as a tool error they can recover
                // from rather than killing the request.
                None => success(
                    id,
                    tool_error(&format!(
                        "no such command: {path:?}. Call list_commands to see what exists."
                    )),
                ),
            }
        }
        _ => error_response(id, -32602, &format!("unknown tool: {name}")),
    }
}

/// Flatten the tree. Hidden commands take their subtree with them, since a
/// visible child of a hidden parent is not reachable as a documented path.
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
fn find_command<'a>(spec: &'a Spec, path: &str) -> Option<&'a SpecCommand> {
    let mut cur = &spec.cmd;
    let mut found = None;
    for segment in path.split_whitespace() {
        let next = cur.find_subcommand(segment)?;
        found = Some(next);
        cur = next;
    }
    found
}

fn describe(spec: &Spec, cmd: &SpecCommand) -> Value {
    json!({
        "command": format!("{} {}", spec.bin, cmd.full_cmd.join(" ")).trim(),
        "usage": cmd.usage,
        "help": cmd.help,
        "long_help": cmd.help_long,
        "aliases": cmd.aliases,
        "hidden": cmd.hide,
        "effect": cmd.effect.map(|e| e.as_str()),
        "args": cmd.args.iter().map(describe_arg).collect::<Vec<_>>(),
        "flags": cmd.flags.iter().map(describe_flag).collect::<Vec<_>>(),
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
        "global": flag.global,
        "arg": flag.arg.as_ref().map(describe_arg),
    })
}

/// Structured data, plus the same JSON as text for clients that ignore
/// `structuredContent` — which the spec asks servers to do.
fn tool_result(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": value,
    })
}

fn tool_error(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

fn success(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"
name "pitchfork"
bin "pitchfork"
cmd "logs" effect="read" help="Displays logs" {
    alias "l"
    flag "-c --clear" effect="destructive" help="Delete logs"
    flag "-t --tail" help="Follow"
}
cmd "daemons" help="Manage daemons" {
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

    fn call(method: &str, params: Value) -> Value {
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let out = handle_line(&spec(), &req.to_string()).expect("a response");
        serde_json::from_str(&out).unwrap()
    }

    /// The payload a tool call returns, unwrapped from the envelope.
    fn structured(tool: &str, args: Value) -> Value {
        let res = call("tools/call", json!({"name": tool, "arguments": args}));
        res["result"]["structuredContent"].clone()
    }

    #[test]
    fn initialize_reports_the_protocol_and_tools() {
        let res = call("initialize", json!({"protocolVersion": PROTOCOL_VERSION}));
        assert_eq!(res["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert!(res["result"]["capabilities"]["tools"].is_object());
        assert_eq!(res["result"]["serverInfo"]["name"], "usage");
        // The instructions carry the meaning of `effect`, which is the whole
        // reason a client would point at this.
        let instructions = res["result"]["instructions"].as_str().unwrap();
        assert!(instructions.contains("destructive"), "{instructions}");
    }

    #[test]
    fn notifications_get_no_reply() {
        // Every client sends this straight after initialize; answering it
        // with an id-less response is a protocol violation.
        let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(handle_line(&spec(), &notification.to_string()).is_none());
    }

    #[test]
    fn unparseable_input_is_a_parse_error() {
        let out = handle_line(&spec(), "{not json").expect("a response");
        let res: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(res["error"]["code"], -32700);
        assert!(res["id"].is_null());
    }

    #[test]
    fn unknown_method_is_a_protocol_error() {
        let res = call("resources/list", json!({}));
        assert_eq!(res["error"]["code"], -32601);
    }

    #[test]
    fn tools_are_listed_with_schemas() {
        let res = call("tools/list", json!({}));
        let tools = res["result"]["tools"].as_array().unwrap();
        let names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["list_commands", "describe_command"]);
        assert_eq!(tools[1]["inputSchema"]["required"][0], "command");
    }

    #[test]
    fn list_commands_hides_hidden_subtrees_by_default() {
        let out = structured("list_commands", json!({}));
        let paths: Vec<_> = out["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["command"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(paths, ["logs", "daemons", "daemons remove", "start"]);
        // A visible child of a hidden parent is not a documented path.
        assert!(!paths.iter().any(|p| p.starts_with("internal")));
    }

    #[test]
    fn list_commands_can_include_hidden() {
        let out = structured("list_commands", json!({"include_hidden": true}));
        let paths: Vec<_> = out["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["command"].as_str().unwrap().to_string())
            .collect();
        assert!(paths.contains(&"internal".to_string()));
        assert!(paths.contains(&"internal child".to_string()));
    }

    #[test]
    fn list_commands_carries_the_effect() {
        let out = structured("list_commands", json!({}));
        let by_path = |p: &str| {
            out["commands"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["command"] == p)
                .cloned()
                .unwrap()
        };
        assert_eq!(by_path("logs")["effect"], "read");
        assert_eq!(by_path("daemons remove")["effect"], "destructive");
        // Unset stays null rather than defaulting to something reassuring.
        assert!(by_path("start")["effect"].is_null());
    }

    #[test]
    fn describe_command_reports_flag_effects() {
        let out = structured("describe_command", json!({"command": "logs"}));
        assert_eq!(out["effect"], "read");
        assert_eq!(out["help"], "Displays logs");
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
    fn describe_command_walks_nested_paths_and_aliases() {
        assert_eq!(
            structured("describe_command", json!({"command": "daemons remove"}))["effect"],
            "destructive"
        );
        // `l` is an alias for `logs`, and an agent may well have seen it.
        assert_eq!(
            structured("describe_command", json!({"command": "l"}))["help"],
            "Displays logs"
        );
    }

    #[test]
    fn describing_a_missing_command_is_a_tool_error_not_a_protocol_error() {
        let res = call(
            "tools/call",
            json!({"name": "describe_command", "arguments": {"command": "nope"}}),
        );
        assert!(res["error"].is_null(), "should not be a protocol error");
        assert_eq!(res["result"]["isError"], true);
        let text = res["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("list_commands"), "{text}");
    }

    #[test]
    fn results_carry_text_as_well_as_structured_content() {
        // The spec asks servers returning structured content to also send the
        // serialized JSON, for clients that only read `content`.
        let res = call(
            "tools/call",
            json!({"name": "list_commands", "arguments": {}}),
        );
        let text = res["result"]["content"][0]["text"].as_str().unwrap();
        let reparsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(reparsed, res["result"]["structuredContent"]);
    }

    #[test]
    fn serve_answers_a_full_session() {
        let input = format!(
            "{}\n{}\n{}\n",
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        );
        let mut out = Vec::new();
        serve(&spec(), input.as_bytes(), &mut out).unwrap();
        let lines: Vec<_> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(String::from)
            .collect();
        // Two requests, one notification, so two responses.
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(serde_json::from_str::<Value>(&lines[0]).unwrap()["id"], 1);
        assert_eq!(serde_json::from_str::<Value>(&lines[1]).unwrap()["id"], 2);
    }
}
