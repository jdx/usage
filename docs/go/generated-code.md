# Generated Code

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

`usage generate go` lowers a KDL spec into one Go file. The output is `gofmt`-clean and carries
the standard `// Code generated … DO NOT EDIT.` header.

```bash
usage generate go -f mycli.usage.kdl -o tables.go -p mycli
```

| Flag                   | Meaning                                                              |
| ---------------------- | -------------------------------------------------------------------- |
| `-f --file <FILE>`     | the KDL spec (`-` for stdin)                                         |
| `--spec <SPEC>`        | a raw spec string instead of a file                                  |
| `-o --out-file <PATH>` | output path (`-` for stdout)                                         |
| `-p --package <NAME>`  | package clause; defaults to the spec's `bin` made into an identifier |

## What the file exports

```go
const Version = "1.2.3"        // only when the spec declares a version

const (                        // one key per command, flag, and argument
	CmdRoot     uint64 = 1
	FlagVerbose uint64 = 2
	ArgFile     uint64 = 3
	CmdInstall  uint64 = 4
	// …
)

var Root *argv.Command         // the hot parse table
var Meta argv.Metadata         // validation metadata (required, choices, env, defaults, relations)
var HelpText argv.HelpTable    // help text per entry
var HelpMeta argv.HelpSpec     // root-level page furniture (name, bin, version, about)

type Cli struct { /* … */ }    // one struct for the root
type InstallCmd struct { /* … */ }  // and one per command

func Parse(args []string) (*Cli, error)
```

`Parse` takes the tokens after the program name. A spec that declares
`multicall` needs argv[0] rewritten first, because a symlink `ls -> busybox`
has no `ls` word in `os.Args[1:]`:

```go
args := argv.RewriteMulticall(os.Args[0], os.Args[1:], HelpMeta.Name, HelpMeta.Bin)
cli, err := Parse(args)
```

`RewriteMulticall` prepends argv[0]'s basename when it is an applet rather than
the dispatcher (`name` / `bin`). Path components and a trailing `.exe` are
stripped. A dispatcher invocation is a no-op.

The three tables are separate on purpose: reference only `Root` and the linker drops the
validation metadata and help text. On mise's spec that's the difference between a 2.60MB and a
2.82MB contribution to the binary. Dispatch on the key constants, never on `Name` strings — a
rename in the spec then fails to compile instead of silently misrouting.

## The structs

For the spec on the [intro page](/go/):

```go
// Cli is the whole command line.
type Cli struct {
	Verbose bool        // FlagVerbose
	Jobs    string      // FlagJobs
	File    string      // ArgFile
	Install *InstallCmd // CmdInstall
}

// InstallCmd is `install`.
type InstallCmd struct {
	Force bool   // FlagInstallForce
	Pkg   string // ArgInstallPkg
}
```

- The root struct is always `Cli`; a subcommand's is the Pascal-cased path plus `Cmd`
  (`config ls` → `ConfigLsCmd`).
- Subcommands are pointers, and at most one per level is non-nil — that's how you tell which
  path was taken.
- Field types: `count` flags → `int`; value-less flags → `bool`; `var` flags/args → `[]string`;
  everything else → `string`. There is no type inference from the spec — a spec says what a
  value is _called_, never what type it is. Convert with the
  [typed helpers](/go/binding#typed-values).
- A flag and a command sharing a name are disambiguated by kind: a `--shell` flag beside a
  `shell` command yields fields `Shell` and `ShellCmd`, not `Shell2`.

## What `Parse` enforces

`Parse` walks the events, fills the structs, then — for the commands the words actually
selected — applies fallbacks and checks:

1. values resolve **argv → env → default_if → default**, per entry
2. `required`, `choices`, `var_min`/`var_max` are checked
3. `conflicts`, `requires`, `required_if`, `required_if_eq`, and `required_unless` are checked across the selected commands

A value-less flag set from an env var goes through `argv.EnvTruth` (usage-lib's narrow
allow-list: `1`, `true`, `True`, `TRUE`); a `default` on one compares against the literal
`"true"`. `count` fields are never filled from env or defaults — a count is occurrences, and
only the command line has those. A `default_subcommand` routes in the parser, so the defaulted
command's struct is filled with no caller involvement.

Three things `Parse` deliberately does **not** do:

- **`overrides` is not applied.** If your spec uses it, call `argv.ApplyOverrides` yourself.
- **Help and version are not printed** — they come back as `*argv.Error` with `CodeHelp` /
  `CodeVersion` for you to render ([Help and errors](/go/help)).
- **No chain comes back with an error.** The renderers want the command chain; recover it with
  `argv.Walk(Root, args)`, which returns the chain even for lines that failed to parse.

## Using it against a real spec

From the tests over mise's actual 211-command spec:

```go
cli, err := mise.Parse([]string{"use", "-g", "node@20"})
// cli.Use != nil; cli.Use.Global == true; cli.Use.ToolVersion == []string{"node@20"}
// cli.Config == nil — a command nobody ran is nil

cli, _ = mise.Parse([]string{"tasks", "run", "build", "extra", "--", "--verbose"})
run := cli.Tasks.Run
// run.Task == "build"; run.Args == []string{"extra"}; run.ArgsLast == []string{"--verbose"}

_, err = mise.Parse([]string{"--log-level", "chatty"})
e := err.(*argv.Error) // e.Code == argv.CodeInvalidChoice
```
