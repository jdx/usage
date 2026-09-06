# Generate SDKs

`usage generate sdk` creates typed TypeScript and Python clients for a CLI. Each
client builds an argument list and runs the executable as a subprocess. Required
arguments and declared choices appear in the generated types, so editors and type
checkers can catch mistakes before a command runs.

The CLI binary must be installed wherever the client runs. The generated client
does not need the `usage` executable at runtime.

## Start with a spec

Save this as `mycli.usage.kdl`. It describes the interface your executable must
implement; generating a client does not implement the `build` command itself.

```kdl
bin "mycli"
cmd "build" help="Build a target" {
    arg "<target>" help="Build target" {
        choices "debug" "release"
    }
    flag "--output <dir>" help="Output directory"
    flag "--release" help="Enable optimizations"
}
```

## TypeScript

Generate the source files into your Node.js project:

```sh
usage generate sdk --language typescript --output ./sdk --file ./mycli.usage.kdl
```

The output contains `types.ts`, `client.ts`, `runtime.ts`, and `index.ts`. Include
these files in your project's TypeScript build; the generator does not create
package metadata or install dependencies. The runtime uses `node:child_process`
and needs Node.js type declarations when type-checking. Install `@types/node`
as a development dependency and include `"node"` in your TypeScript
`compilerOptions.types`.

```ts
import { Mycli } from "./sdk";

async function build() {
  const cli = new Mycli("mycli");
  const result = await cli.build.exec(
    { target: "release" },
    { output: "./dist", release: true }
  );
  if (result.ok) {
    console.log(result.stdout);
  } else {
    console.error(result.stderr);
    process.exitCode = result.exitCode;
  }
}

build();
```

The first object holds positional arguments; the second holds flags. `target` is
required and accepts `"debug"` or `"release"`. Each `exec()` returns a promise.
Pass an absolute binary path to `Mycli` when the executable is not on `PATH`.

## Python

Generate a package next to the Python code that imports it:

```sh
usage generate sdk --language python --output ./sdk --file ./mycli.usage.kdl
```

The output contains `types.py`, `client.py`, `runtime.py`, and `__init__.py`. The
runtime uses the Python standard library and runs commands synchronously.

```python
from sdk import Mycli, BuildArgs, BuildFlags

cli = Mycli(bin_path="mycli")
result = cli.build.exec(
    BuildArgs(target="release"),
    BuildFlags(output="./dist", release=True),
)
if result.ok:
    print(result.stdout)
else:
    print(result.stderr)
    raise SystemExit(result.exit_code)
```

Generated dataclasses carry type annotations, including `Literal` choices. Use a
Python type checker to validate calls; annotations alone do not enforce types at
runtime.

## Results and errors

A completed process returns stdout, stderr, and its exit code, including when the
exit code is nonzero. Check `result.ok` or the exit code before consuming output.
Failures to start the executable are reported as errors instead of successful
results.

## Keep clients in sync

Regenerate the SDK when the CLI's interface changes. Keep the spec, generated
sources, and binary version together in your release process. Generated source
files are overwritten, so put custom wrappers in separate files.

TypeScript and Python are the supported targets. Both mirror the subcommand tree,
include global flags on descendant commands, and represent choices in their types.
See the [command reference](/cli/reference/generate/sdk) for generation options.

## Structured and streaming outputs

When a command [declares structured outputs](/spec/reference/output), the SDK keeps `exec()` for
raw text and adds a method per wire format. A `json` output produces `execJson()` in TypeScript
and `exec_json()` in Python; a `jsonl` output produces an async iterable or an iterator that
parses one document at a time. The result still carries stderr and the exit code, because a
documented nonzero status may accompany valid structured output.

Declared JSON Schemas are exported as string constants, and declared exit statuses become a
table and a literal-union type in the types module.
