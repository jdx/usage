# Generating Type-Safe SDKs

Usage CLI can generate type-safe SDK client libraries from a Usage spec. The generated SDK is a
**subprocess wrapper** -- it invokes your CLI binary via `subprocess.run` / `execFileSync` /
`std::process::Command`, not a native binding. It provides type definitions for arguments, flags,
and choices, along with a client that constructs the correct CLI argument list for you.

## When to Use This

### CLIs without language bindings

Popular tools like ffmpeg have hand-written bindings in many languages, but the vast majority of CLI
tools don't. For tools like `restic`, `rclone`, `pandoc`, `age`, or any internal CLI, the only
option has been to manually construct argument lists:

```python
# before: stringly-typed, no autocomplete, typos slip through
subprocess.run(["rclone", "copy", src, dst, "--progress", "--transfers", "4"])
```

With a generated SDK:

```python
# after: typed, autocomplete, mistakes caught at lint time
rclone.copy(src, dst, progress=True, transfers=4)
```

### Stay in sync with CLI versions

Hand-written bindings drift out of date when the CLI evolves. Generated SDKs solve this the same way
Protobuf/gRPC does -- the spec is the source of truth, the SDK is a derived artifact:

```sh
# in CI, when you cut a new release:
usage generate sdk -l python -o ./sdk/python/ -f ./mycli.usage.kdl
git commit -m "chore: regenerate sdk from v2.3.0 spec"
```

### Internal platform CLIs

This is the strongest use case. Companies typically have internal CLIs for deployment, config
management, database migrations, etc. Teams in different languages (Python scripts, TypeScript
services, Rust tools) all need to call these CLIs, and each team independently writes fragile
subprocess calls. With a Usage spec, you generate typed SDKs for all languages from a single source
of truth:

```ts
// auto-generated, always in sync with the CLI
import { deploy } from "@internal/platform-sdk";
await deploy({ env: "prod", service: "api", replicas: 3 });
//             ^ typed, choices-constrained, required-checked
```

## Quick Start

Given a spec file `mycli.usage.kdl`:

```sh
usage generate sdk -l typescript -o ./sdk -f ./mycli.usage.kdl
```

This generates a complete SDK in the `./sdk` directory, ready to use:

```ts
import { Mycli } from "./sdk";

const cli = new Mycli();
const result = cli.build.exec(
  { target: "release", output: "./dist" },
  { release: true }
);
if (result.ok) {
  console.log(result.stdout);
}
```

## Supported Languages

| Language   | Flag            | Output Files                                                                  |
| ---------- | --------------- | ----------------------------------------------------------------------------- |
| TypeScript | `-l typescript` | `types.ts`, `client.ts`, `runtime.ts`, `index.ts`                             |
| Python     | `-l python`     | `types.py`, `client.py`, `runtime.py`, `__init__.py`                          |
| Rust       | `-l rust`       | `src/types.rs`, `src/client.rs`, `src/runtime.rs`, `src/lib.rs`, `Cargo.toml` |

### TypeScript

```sh
usage generate sdk -l typescript -o ./sdk -f ./mycli.usage.kdl
```

Generates ES module files with full type annotations. The client uses `execFileSync` from
`node:child_process` under the hood.

```ts
import { Mycli, BuildArgs, BuildFlags } from "./sdk";

const cli = new Mycli();
const result = cli.build.exec(
  { target: "release", output: "./dist" } as BuildArgs,
  { release: true } as BuildFlags
);
```

### Python

```sh
usage generate sdk -l python -o ./sdk -f ./mycli.usage.kdl
```

Generates a Python package with `@dataclass` type definitions and type annotations. The client uses
`subprocess.run` under the hood.

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

### Rust

```sh
usage generate sdk -l rust -o ./sdk -f ./mycli.usage.kdl
```

Generates a zero-dependency Rust crate with idiomatic types: enums for choices, structs for
args/flags, and `Result<CliResult, CliError>` return types. The client uses
`std::process::Command` under the hood.

```rust
use mycli_sdk::{Mycli, BuildArgs, BuildFlags, TargetChoice};

let cli = Mycli::new("mycli");
let result = cli.build.exec(
    BuildArgs { target: TargetChoice::Release, output: "./dist".into() },
    Some(&BuildFlags { release: Some(true), ..Default::default() }),
)?;
if result.ok() {
    println!("{}", result.stdout);
}
```

## CLI Options

```
usage generate sdk [OPTIONS]

Options:
  -l, --language <LANGUAGE>       Target language: typescript, python, rust
  -o, --output <OUTPUT>           Output directory for generated SDK files
  -p, --package-name <NAME>       Override the package/module name (defaults to spec bin name)
  -f, --file <FILE>               A usage spec taken in as a file
      --spec <SPEC>               Raw string spec input
```

## Feature Support

The following table shows which Usage spec features are supported by each language target:

| Feature            | Spec Syntax                         | TypeScript | Python | Rust |
| ------------------ | ----------------------------------- | :--------: | :----: | :--: |
| Positional args    | `arg "name"`                        |     ✅     |   ✅   |  ✅  |
| Required args      | `arg "name" required=#true`         |     ✅     |   ✅   |  ✅  |
| Optional args      | `arg "[name]"`                      |     ✅     |   ✅   |  ✅  |
| Variadic args      | `arg "name" var=#true`              |     ✅     |   ✅   |  ✅  |
| Arg choices        | `arg "name" { choices "a" "b" }`    |     ✅     |   ✅   |  ✅  |
| Arg defaults       | `arg "name" default="value"`        |     ✅     |   ✅   |  ✅  |
| Arg help text      | `arg "name" help="..."`             |     ✅     |   ✅   |  ✅  |
| Arg env var        | `arg "name" env="VAR"`              |     ✅     |   ✅   |  ✅  |
| Double dash        | `arg "name" double_dash="required"` |     ✅     |   ✅   |  ✅  |
| Boolean flags      | `flag "--flag"`                     |     ✅     |   ✅   |  ✅  |
| Value flags        | `flag "--flag <value>"`             |     ✅     |   ✅   |  ✅  |
| Short flags        | `flag "-f --flag"`                  |     ✅     |   ✅   |  ✅  |
| Flag choices       | `flag "--flag" { choices "a" "b" }` |     ✅     |   ✅   |  ✅  |
| Flag defaults      | `flag "--flag" default="val"`       |     ✅     |   ✅   |  ✅  |
| Flag help text     | `flag "--flag" help="..."`          |     ✅     |   ✅   |  ✅  |
| Flag env var       | `flag "--flag" env="VAR"`           |     ✅     |   ✅   |  ✅  |
| Count flags        | `flag "-v" count=#true`             |     ✅     |   ✅   |  ✅  |
| Negate flags       | `flag "--flag" negate="--no-flag"`  |     ✅     |   ✅   |  ✅  |
| Repeatable flags   | `flag "--flag" var=#true`           |     ✅     |   ✅   |  ✅  |
| Required flags     | `flag "--flag" required=#true`      |     ✅     |   ✅   |  ✅  |
| Global flags       | `flag "--flag" global=#true`        |     ✅     |   ✅   |  ✅  |
| Deprecated flags   | `flag "--flag" deprecated="msg"`    |     ✅     |   ✅   |  ✅  |
| Hidden args/flags  | `hide=#true`                        |     ✅     |   ✅   |  ✅  |
| Subcommands        | `cmd "name" { ... }`                |     ✅     |   ✅   |  ✅  |
| Nested subcommands | `cmd "a" { cmd "b" { ... } }`       |     ✅     |   ✅   |  ✅  |
| Subcommand aliases | `alias "name"`                      |     ✅     |   ✅   |  ✅  |
| Hyphenated names   | `cmd "add-remote"`                  |     ✅     |   ✅   |  ✅  |
| Spec metadata      | `version`, `about`, `author`        |     ✅     |   ✅   |  ✅  |
| Config             | `config "key" { ... }`              |     ✅     |   ✅   |  ✅  |

## How It Works

Each generated SDK consists of three parts:

1. **Types module** -- Type definitions for every command's args and flags. Choice constraints are
   rendered as union types (TypeScript), `Literal` types (Python), or enums with `Display` (Rust).
   Global flags are propagated to all subcommand flag types.

2. **Client module** -- A nested class/struct hierarchy mirroring the subcommand tree. Each node has
   an `exec()` method that constructs the CLI argument list and invokes the binary. Flag arguments
   are built via a helper method that handles value flags, boolean flags, count flags, negate flags,
   and repeatable flags.

3. **Runtime module** -- A small, static module containing `CliResult` (stdout, stderr, exit code)
   and `CliRunner` (the subprocess invocation logic). This module is identical across all SDKs
   generated from the same language target.
