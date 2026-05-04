# Generating Type-Safe SDKs

Usage CLI can generate type-safe SDK client libraries from a Usage spec. The generated SDK is a
**subprocess wrapper** -- it invokes your CLI binary via `subprocess.run` / `child_process.spawn`,
not a native binding. It provides type definitions for arguments, flags,
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
const result = await deploy({ env: "prod", service: "api", replicas: 3 });
//                        ^ typed, choices-constrained, required-checked
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
const result = await cli.build.exec(
  { target: "release", output: "./dist" },
  { release: true }
);
if (result.ok) {
  console.log(result.stdout);
}
```

## Supported Languages

| Language   | Flag            | Output Files                                         |
| ---------- | --------------- | ---------------------------------------------------- |
| TypeScript | `-l typescript` | `types.ts`, `client.ts`, `runtime.ts`, `index.ts`    |
| Python     | `-l python`     | `types.py`, `client.py`, `runtime.py`, `__init__.py` |
| Rust       | Coming soon     |                                                      |

### TypeScript

```sh
usage generate sdk -l typescript -o ./sdk -f ./mycli.usage.kdl
```

Generates ES module files with full type annotations. The client uses `spawn` from
`node:child_process` under the hood and all `exec()` methods are async, returning
`Promise<CliResult>`.

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

_Rust SDK support is coming soon._

## How It Works

Each generated SDK consists of three parts:

1. **Types module** -- Type definitions for every command's args and flags. Choice constraints are
   rendered as union types (TypeScript) or `Literal` types (Python).
   Global flags are propagated to all subcommand flag types.

2. **Client module** -- A nested class/struct hierarchy mirroring the subcommand tree. Each node has
   an `exec()` method that constructs the CLI argument list and invokes the binary. Flag arguments
   are built via a helper method that handles value flags, boolean flags, count flags, negate flags,
   and repeatable flags.

3. **Runtime module** -- A small, static module containing `CliResult` (stdout, stderr, exit code)
   and `CliRunner` (the subprocess invocation logic). This module is identical across all SDKs
   generated from the same language target.
