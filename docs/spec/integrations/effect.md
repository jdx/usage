# Effect (TypeScript)

[`usage-spec-effect`](https://github.com/jdx/usage/tree/main/integrations/effect) converts an [Effect](https://effect.website) v4 `effect/cli` command tree into a usage spec. Effect keeps parsing and running the CLI; the spec drives completions, reference docs, and man pages.

It supports `effect` 4.x. The package is not published to npm yet; use it from a checkout of this repository.

## Quickstart

```ts
import { Argument, Command, Flag } from "effect/cli";
import { generate } from "usage-spec-effect";

const serve = Command.make("serve", {
  port: Flag.Int("port").pipe(Flag.withAlias("p"), Flag.withDefault(8080)),
  files: Argument.String("file").pipe(Argument.variadic({ min: 1 })),
}).pipe(Command.withDescription("Serve files"));

const app = Command.make("mycli").pipe(Command.withSubcommands([serve]));

if (process.argv.includes("--usage-spec")) {
  process.stdout.write(generate(app, { version: "1.0.0" }));
  process.exit(0);
}
```

This prints:

```kdl
name "mycli"
bin "mycli"
version "1.0.0"
cmd "serve" help="Serve files" {
    flag "-p --port" default="8080" {
        arg "<int>"
    }
    arg "<file>…" var=#true var_min=1
}
```

Check for `--usage-spec` before running the command, then pipe the output to `usage`:

```bash
mycli --usage-spec | usage generate completion bash mycli --file -
mycli --usage-spec | usage generate md --file -
```

## API

| Function                                | Description                                                                   |
| --------------------------------------- | ----------------------------------------------------------------------------- |
| `generate(command, options?)`           | Returns the usage spec as a KDL string                                        |
| `generateWithReport(command, options?)` | Returns `{ kdl, unsupported }`, listing what could not be represented exactly |

`options.name` overrides the spec `name` and `bin`; `options.version` sets `version`.

## Feature mapping

| Effect                                                     | Usage Spec                                                          |
| ---------------------------------------------------------- | ------------------------------------------------------------------- |
| Command name                                               | `name`, `bin`, `cmd`                                                |
| `Command.withShortDescription` / `withDescription`         | `about` / `help`, with `long_about` / `long_help` for longer text   |
| `Command.withAlias`                                        | `alias`                                                             |
| `Command.unlisted`                                         | `hide=#true`                                                        |
| `Command.withExamples`                                     | `example` nodes                                                     |
| `Command.withSharedFlags`                                  | `global=#true` flags                                                |
| `Flag.withAlias`                                           | one-character aliases become `-x`, longer ones `--alias`            |
| `Flag.Boolean`                                             | flag without an `arg` child                                         |
| Other flags                                                | flag with an `arg <type>` child, or `Flag.withMetavar`              |
| `Flag.optional`, `Flag.withDefault`                        | not required; `default="…"` for string, number and boolean defaults |
| `Flag.Literals`, `Argument.Literals`                       | `choices`                                                           |
| `Flag.atLeast` / `atMost` / `between`, `Argument.variadic` | `var=#true`, `var_min`, `var_max`                                   |
| `Argument.optional`                                        | `[name]` with `required=#false`                                     |
| `Flag.withHidden`                                          | `hide=#true`                                                        |

## Unsupported constructs

`generateWithReport` lists anything exported approximately instead of dropping it silently:

- `orElse` alternatives export only the first parameter.
- `FileParse`, `FileSchema`, `Redacted`, and `KeyValuePair` values export as plain strings.
- Defaults that are not a string, number, or boolean are omitted.
- Flags with an optional value (`Flag.withOptionalValue`) export as requiring a value.

## How it reads commands

Effect's public `Command` type exposes names, descriptions, aliases, examples, and subcommands, but not flags or arguments. The exporter reads those from the command's runtime `config`, the same data Effect uses for its own help and completions. If a future Effect release changes that shape, `generate` throws an error naming the command instead of emitting a partial spec.
