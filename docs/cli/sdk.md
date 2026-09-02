# Generating Type-Safe SDKs

A CLI without a client library is called the same way everywhere: a hand-built list of strings
handed to `subprocess.run` or `child_process.spawn`, where a misspelled flag is found at runtime
by whoever runs it. `usage generate sdk` derives the client from the spec instead. The result is
a **subprocess wrapper**, not a native binding: typed definitions for every command's arguments,
flags, and choices, and a client that builds the argument list and invokes the binary.

```python
# before: stringly typed, no autocomplete, typos found at runtime
subprocess.run(["rclone", "copy", src, dst, "--progress", "--transfers", "4"])

# after: typed, autocompleted, mistakes caught by the type checker
rclone.copy(src, dst, progress=True, transfers=4)
```

## When it fits

**CLIs without bindings.** A few tools such as ffmpeg have hand-written bindings in many
languages. Most do not: `restic`, `rclone`, `pandoc`, `age`, and every internal CLI are called by
assembling strings. A spec for the tool is enough to generate the binding.

**Staying in sync.** Hand-written bindings drift as the CLI changes. A generated SDK is a derived
artifact, the way Protobuf stubs are, so regenerating it on each release is the whole of the
maintenance:

```sh
usage generate sdk -l python -o ./sdk/python/ -f ./mycli.usage.kdl
git commit -m "chore: regenerate sdk from v2.3.0 spec"
```

**Internal platform CLIs.** This is the strongest case. A company's deploy, config, and migration
tools are called from Python scripts, TypeScript services, and Rust tools alike, and each team
writes its own fragile subprocess calls. One spec generates a typed client for every language at
once:

```ts
// generated, and regenerated with the CLI
import { deploy } from "@internal/platform-sdk";
const result = await deploy({ env: "prod", service: "api", replicas: 3 });
//                        ^ typed, choices constrained, required fields checked
```

## Quick start

Given a spec file `mycli.usage.kdl`:

```sh
usage generate sdk -l typescript -o ./sdk -f ./mycli.usage.kdl
```

The `./sdk` directory is a complete package, ready to import:

```ts
import { Mycli } from "./sdk";

const cli = new Mycli();
const result = await cli.build.exec(
  { target: "release", output: "./dist" },
  { release: true }
);
if (result.ok) {
  console.log(result.stdout);
}
```

## Supported languages

| Language   | Flag            | Output files                                         |
| ---------- | --------------- | ---------------------------------------------------- |
| TypeScript | `-l typescript` | `types.ts`, `client.ts`, `runtime.ts`, `index.ts`    |
| Python     | `-l python`     | `types.py`, `client.py`, `runtime.py`, `__init__.py` |
| Rust       | planned         |                                                      |

### TypeScript

```sh
usage generate sdk -l typescript -o ./sdk -f ./mycli.usage.kdl
```

ES modules with full type annotations. The client spawns the binary with `node:child_process`,
so every `exec()` is async and returns a `Promise<CliResult>`.

```ts
import { Mycli, BuildArgs, BuildFlags } from "./sdk";

const cli = new Mycli();
const result = await cli.build.exec(
  { target: "release", output: "./dist" } as BuildArgs,
  { release: true } as BuildFlags
);
```

### Python

```sh
usage generate sdk -l python -o ./sdk -f ./mycli.usage.kdl
```

A package of `@dataclass` types with annotations throughout. The client runs the binary with
`subprocess.run`.

```python
from sdk import Mycli, BuildArgs, BuildFlags

cli = Mycli()
result = cli.build.exec(
    BuildArgs(target="release", output="./dist"),
    BuildFlags(release=True)
)
if result.ok:
    print(result.stdout)
```

## How it works

Each SDK is three modules:

1. **Types.** A definition for every command's args and flags. `choices` become union types in
   TypeScript and `Literal` types in Python, and global flags are repeated on every subcommand's
   flag type so that they can be passed where they are used.

2. **Client.** A nested class hierarchy mirroring the subcommand tree. Each node has an `exec()`
   that assembles the argument list and runs the binary; one helper handles value flags, boolean
   flags, count flags, negated flags, and repeatable flags.

3. **Runtime.** A small static module holding `CliResult` (stdout, stderr, exit code) and
   `CliRunner`, the subprocess call. It is identical across every SDK generated for the same
   language.

## Structured and streaming outputs

When a command [declares structured outputs](/spec/reference/output), the SDK keeps `exec()` for
raw text and adds a method per wire format. A `json` output produces `execJson()` in TypeScript
and `exec_json()` in Python; a `jsonl` output produces an async iterable or an iterator that
parses one document at a time. The result still carries stderr and the exit code, because a
documented nonzero status may accompany valid structured output.

Declared JSON Schemas are exported as string constants, and declared exit statuses become a
table and a literal-union type in the types module.
