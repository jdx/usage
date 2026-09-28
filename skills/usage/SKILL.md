---
name: usage
description: Write and validate Usage KDL CLI specifications, add argument parsing to scripts, and generate shell completions, Markdown, and man pages with the usage CLI.
---

# Usage

Use the project's existing source of CLI metadata. For a hand-written CLI or
script, edit its `.usage.kdl` file or `#USAGE` comments. If a spec is generated
from Rust derives, clap, or Cobra, change those declarations and regenerate it
instead of editing the generated KDL. Check `usage --version` and the relevant
subcommand's `--help` for the installed version's options.

## Write and validate a spec

```kdl
name "greet"
bin "greet"
about "Greet someone by name"
flag "-v --verbose" help="Print extra detail"
flag "--style <style>" default="friendly" help="Greeting style" {
    choices "friendly" "formal"
}
arg "<name>" help="Person to greet"
```

Use `<name>` for a required argument, `[name]` for an optional one, and `...`
for a variadic argument. Put required positionals before optional ones and a
variadic positional last. A flag with no value is boolean; `--style <style>`
takes a value. KDL v2 booleans are `#true` and `#false`. Give commands, flags,
and arguments useful help; it feeds both docs and completions.

```sh
usage lint greet.usage.kdl --warnings-as-errors
usage explain --file greet.usage.kdl --format json -- greet --style formal Ada
```

`lint` catches semantic mistakes beyond KDL syntax. `explain` shows argument
binding, defaults, environment fallbacks, and errors without running the target
command. Inspect its report: it exits successfully even when the explained
argv is invalid. Put `--` before the program name so a later separator belongs
to the explained command. When providing `explain --env KEY=VALUE`, those
entries replace the entire environment for the explanation.

## Parse a script's arguments

```bash
#!/usr/bin/env -S usage bash
#USAGE flag "-u --uppercase" help="Print the greeting in uppercase"
#USAGE arg "<name>" help="Person to greet"

greeting="Hello, $usage_name"
if [ "$usage_uppercase" = "true" ]; then
  printf '%s\n' "$greeting" | tr '[:lower:]' '[:upper:]'
else
  printf '%s\n' "$greeting"
fi
```

Usage exports parsed values as `usage_<name>` environment variables. Boolean
values are strings such as `true`; quote values when passing them to the shell.
`usage_cmd` contains the canonical subcommand path for dispatch. For other
interpreters, use a shebang such as `#!/usr/bin/env -S usage exec node` and the
language's comment prefix (`//USAGE` for JavaScript). A sibling
`.<script>.usage.kdl` takes precedence over embedded comments.

## Generate output

```sh
usage generate completion bash greet --file greet.usage.kdl > greet.bash
usage generate markdown --file greet.usage.kdl --out-file greet.md
usage generate manpage --file greet.usage.kdl --out-file greet.1
```

Completion scripts call `usage complete-word` at runtime, so `usage` must stay
on PATH. For a CLI that prints its spec, use `--usage-cmd 'greet --usage-spec'`
instead of `--file` only if that command is actually supported. Dynamic
completion commands in a spec also execute when the user completes a value.

Preview generated scripts before installing them. `generate completion
--install` writes the shell's completion file but does not edit shell startup
files; follow any setup instructions it prints. Preserve existing startup-file
and completion configuration.

Use the [spec reference](https://usage.jdx.dev/spec/reference/),
[script guide](https://usage.jdx.dev/cli/scripts), and
[CLI reference](https://usage.jdx.dev/cli/reference/) for advanced declarations
and generation options.
