# Command outputs and exit codes

A command can declare what it writes to stdout, the JSON Schema for structured output, and
what its exit statuses mean:

```kdl
cmd "check" {
  flag "--format <FORMAT>" help="Output format"

  output "human" default=#true help="Human-readable report"
  output "json" framing="json" help="One report object" {
    schema #"""
    {
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "type": "object",
      "required": ["passed"],
      "properties": { "passed": { "type": "boolean" } }
    }
    """#
  }
  output "jsonl" framing="jsonl" help="One event per line"
  select "--format"

  exit_code 0 "all checks passed"
  exit_code 1 "a check failed"
}
```

`output`'s first argument is the value a user types. `framing` is the wire contract a
consumer reads:

| Framing | Meaning                                            |
| ------- | -------------------------------------------------- |
| `text`  | Unstructured text; the default                     |
| `json`  | One JSON document, read to end of stream           |
| `jsonl` | One JSON document per line, consumed incrementally |

The name and framing are deliberately separate. An output named `ndjson` can declare
`framing="jsonl"`, allowing consumers to treat equivalent formats alike without knowing each
CLI's spelling.

An output also accepts `help`, `default=#true`, and `hide=#true`. At most one effective output
may be the default. `schema` is carried verbatim, so it can use any JSON Schema draft and can
contain references that another tool resolves.

For a schema kept next to the spec, name its file instead of embedding it:

```kdl
output "json" framing="json" {
  schema file="report.schema.json"
}
```

A relative path is resolved from the KDL file containing the declaration, including when that
file is included by another spec. Usage loads the file into the same schema field consumers
already read and adds it to `Spec::sources`, so generated output is self-contained and build
scripts know to rerun when the JSON file changes. Inline schemas remain useful for specs parsed
from strings, where a relative file has no directory to resolve against.

## Selecting an output

For a value-taking flag, declare one `select` on the command:

```kdl
flag "--format <FORMAT>"
output "human" default=#true
output "json" framing="json"
select "--format"
```

Usage fills the flag's choices from the output names. Those choices reach validation,
completions, generated documentation, Fig, and generated SDK flag types.

For a boolean flag that selects one output, put `select` on that output:

```kdl
flag "--json"
output "text" default=#true
output "json" framing="json" select="--json"
```

## Inheritance

Top-level `output`, `select`, and `exit_code` declarations apply CLI-wide. A command can refine
an inherited output by redeclaring its name, or remove it with `hide=#true`. Exit codes fold by
number, so a command can refine `exit_code 1 "error"` without repeating the CLI-wide success and
interrupt statuses.

When commands share a global value selector, each command receives choices for its own effective
outputs. This prevents one command from advertising formats that only a sibling produces.

```kdl
flag "--format <FORMAT>" global=#true
exit_code 0 "success"
exit_code 130 "interrupted"

cmd "list" {
  output "text" default=#true
  output "json" framing="json"
  select "--format"
}
```

## Rust derives

Typed Rust declarations use the same vocabulary:

```rust
#[derive(usage::Args)]
#[usage(
    output("human", default, help = "Human-readable report"),
    output("json", framing = "json", schema_from = Report),
    exit_code(0, "all checks passed"),
    exit_code(1, "a check failed"),
)]
struct Check {
    #[usage(long, select)]
    format: Option<String>,
}
```

A schema can come from `schema = "…"`, `schema_from = Type` using `schemars`, or
`schema_fn = path` where the function returns a `String`.

Generated Markdown and manpages document outputs and exit statuses. `usage mcp` includes them in
`describe_command`, parsing a valid schema into structured JSON and preserving invalid schema text
as a string. Generated Python and TypeScript SDKs add parsed JSON methods and streaming JSONL
methods while leaving the existing raw `exec` method unchanged.
