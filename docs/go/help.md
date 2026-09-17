# Help and Errors

::: danger Development preview
The Go framework is not ready for adoption or testing. These pages describe work
in progress; APIs, generated code, and behavior may change. See the
[overview](/go/) for the current scope and limitations.
:::

## Rendering help and version requests

::: warning Generated-parser prerequisite
The generated-parser example below requires the synthetic help/version event
bridge in [#1437](https://github.com/jdx/usage/pull/1437). Until that change lands,
regenerating against `main` does not make built-in `-h`, `--help`, `-V`, and
`--version` return requests. Explicitly declared help/version action flags already
return requests and can use this renderer. This helper does not change parsing.
:::

`RenderRequest` handles `CodeHelp` and `CodeVersion` without application-side
command-tree searches or page selection:

```go
cli, err := Parse(args)
if err != nil {
    var request *argv.Error
    if errors.As(err, &request) {
        meta := HelpMeta
        meta.Version = buildVersion // optional runtime version; leaves tables unchanged
        if text, handled := argv.RenderRequest(request, meta, Root, args, HelpText); handled {
            _, writeErr := io.WriteString(stdout, text)
            return writeErr
        }
    }
    return err
}
// Dispatch cli.
```

Pass the same arguments used for parsing, excluding the program name. `HelpMeta.Bin`
sets the displayed binary name; command aliases retain their invoked spelling.
The helper follows parser events, so shared command nodes retain the correct
ancestors and implicit default commands use their declared names. It stops at
help/version even when more arguments follow.

Short, long, and recursive help use the existing page renderers. Versions are
returned with a trailing newline; long requests prefer `LongVersion` and fall
back to `Version`. Ordinary errors and nil return `("", false)`. The caller
retains control of output, write errors, and exit status.

## Help pages

Three renderers cover the usage line, the `-h` page, and the `--help` page:

```go
argv.UsageLine(path, cmd, HelpText)                    // "mise [FLAGS] [TASK] <SUBCOMMAND>"
argv.ShortHelp(HelpMeta, path, chain, HelpText)        // the -h page
argv.LongHelp(HelpMeta, path, chain, HelpText)         // the --help page
```

`path` is the command as invoked, binary first (`[]string{"mise", "config", "ls"}`); `chain` is
the `*argv.Command` chain from the root to the command (`argv.Walk` returns it, even for lines
that failed to parse). A rendered page:

```
List config files currently in use

Usage: mise config ls [FLAGS]

Flags:
  -J, --json               Output in JSON format
  -h, --help               Print help

Global flags:
  -C, --cd <DIR>           Change directory before running command
```

The output is not merely similar to the reference implementation's — all 211 of mise's usage
lines, `-h` pages, and `--help` pages are compared **byte for byte** against usage-lib's
rendering in CI. Layout details you get for free: sections in canonical order, commands listed by
name in one column per page with `[aliases: …]` after the summary for visible aliases,
`help_heading` groups (first-seen order, unheaded entries first), a 4-column short-flag gutter,
required entries in angle brackets, `[env: X]` and default annotations, and both pages wrapped at
a fixed 80 columns.

Both pages list a command's children identically: the name, then the summary — `help`, or the
first line of `long_help` when there is no `help`. A child's full `long_help` appears on the
child's own page, not repeated in every ancestor's list. A `default_subcommand` is marked
`(default)` in that list. `default_subcommand_help` appends that child's page after the parent
so the default command's flags are on the mount `--help`.

The short page appends `[choices]`, `[env: X]`, and (for arguments) `(default: …)` to the
description, wrapping with it; the long page gives each its own line, indented to the description
column so the section stays one grid, and prefers `long_help` over `help` for the command's own
description. Where a description is already a block under its usage — a `next_line_help` page, or
one whose text has breaks of its own — the annotations join it there. Examples declared on the
root are inherited by commands that declare none.

One rule is load-bearing: a page only advertises a flag spelling where that flag is the one that
would _bind_ it. Masking is per spelling — a subcommand redeclaring `--jobs` leaves an inherited
`-j` advertised if nothing claims it — and matches the parser exactly.

## Rendering failures

```go
msg := argv.Render(err, path, chain, HelpText)
```

The shape is clap's, which your users have seen before:

```
error: unknown flag `--wat`

Usage: ex run [-f --force]

For more information, try `--help`.
```

- The usage line names the command the user was **in**, not the program.
- `CodeHelp` and `CodeVersion` render as the empty string — print the page or version instead.
- Every error code renders something specific. `missing_flag_value` names the likeliest cause
  and the escape hatch in the flag's actual spelling: ``missing value for `--jobs` (a value
beginning with `-` has to be attached: `--jobs=-x`)``. `invalid_choice` appends
  `(expected one of: bash, zsh)`; the variadic codes pluralize correctly; `conflicting_flags`
  names both sides.
- Anything quoted back to the user — tokens, unexpected arguments, rejected values — has control
  characters escaped, so a hostile argv can't smuggle escape sequences to the terminal.

The error type itself is small enough to use directly:

```go
type Error struct {
	Code Code       // CodeUnknownFlag, CodeMissingRequiredFlag, CodeInvalidChoice, …
	// plus the specifics: Token, Name, Choices, Bound, Got, Value, Want, Cmd, Long, …
}
```

`Error()` (the `error` interface) is a bare one-liner; `Render` is the version for humans. The
`Code` names are stable strings shared with the conformance corpus (`unknown_flag`,
`invalid_choice`, `var_too_many`, …), so tests can assert on classes rather than message text.
