# Completions

::: danger Not ready for testing
The Go framework is experimental and **not ready for any amount of testing yet** — do not build
against it. Much of what this page documents only
exists in open pull requests and may change before release. These docs are a draft published for
review, not an invitation to try it.
:::

The Go runtime answers the question every completion request boils down to — _what could go
where the cursor is?_ — from the same tables the parser runs on, so completions can never
disagree with the grammar.

## Position and candidates

```go
pos := argv.Walk(mycli.Root, wordsBeforeCursor)
candidates := argv.Candidates(pos, partialWord, mycli.HelpText, mycli.Meta)
```

`Walk` treats parse errors as _positions_, not failures — an unfinished command line is the
whole point. The `Position` tells you where the cursor stands:

```go
type Position struct {
	Cmd                 *argv.Command   // the command in scope
	Chain               []*argv.Command // root → here
	FlagsPossible       bool            // false past `--`
	SubcommandsPossible bool            // false once a positional has taken a word
	AwaitingValue       *argv.Flag      // cursor is inside this flag's value
	Collecting          *argv.Flag      // a variadic flag still claiming words
	NextArg             *argv.Arg       // the positional that would bind next
	NextArgValues       uint32          // values already bound to NextArg
	SeparatorSeen       bool            // a `--` has been typed
	HelpTopic           bool            // after `help`: completing a topic, not a command to run
}
```

`Candidates` answers by asking the parser's own scope rules, never by re-deriving them:

- subcommands and their visible aliases; hidden commands are never offered
- flags in scope — a global inside a subcommand yes, a non-global root flag no, and masking is
  per spelling, matching help and the parser
- negation spellings (`--no-color`) as first-class candidates
- a flag awaiting its value takes the position entirely: its `choices` and nothing else
- the pending positional's `choices` — unless it demands a `--` that hasn't been typed
- nothing flag-shaped past a `--`

Filtering by the partial word happens here, so every shell agrees on what matches. Each
candidate carries a `Describe` string for shells that display descriptions.

## Speaking each shell's dialect

```go
out := argv.RenderAnswer(argv.Answer{Candidates: candidates}, argv.Zsh)
```

`RenderAnswer` writes one line per candidate, in the format each shell reads: bash gets the
value alone; zsh gets display, description, and a quoted insert text (what it shows and what it
types differ); fish, nu, and PowerShell get value-tab-description. Values and descriptions are
sanitized so a candidate can never rearrange the protocol and make the shell insert something
nobody offered.

An `Answer` can also request the shell's native file or directory completion:

```go
argv.RenderAnswer(argv.Answer{Files: argv.AnyFile}, shell)  // or argv.Dirs
```

## Wiring it up

The protocol is the same one the Rust framework's generated shell scripts speak: the script
calls your binary back with

```
<bin> __complete_word__ --shell <shell> --line "<text before cursor>"
```

Unlike the Rust framework — which intercepts `__complete_word__` automatically — the Go side
asks for one call before normal parsing:

```go
if out, ok := argv.Respond(os.Args[1:], mycli.Root, mycli.HelpText, mycli.Meta); ok {
	fmt.Print(out)
	return
}
```

`Respond` recognizes the hidden subcommand, splits the line at the cursor, walks the tables,
and renders the answer in the calling shell's dialect. `false` means argv was an ordinary
invocation — the cue to parse it as one. The steps are exported individually when you want to
stand between them: `argv.ParseRequest` reads the request out of argv, `Request.Answer` works
out the candidates and whether paths belong, and `argv.Split` does the shell-aware line
splitting on its own.

The scripts that register your binary with each shell come from the same package:

```go
argv.Script("mycli", argv.Zsh) // and Bash, Fish, Nu, PowerShell
```

Each script's whole job is to hand the line to your binary and present what comes back the way
its shell presents things. There is no runtime dependency on the `usage` CLI and no spec file
involved — the binary was compiled with the tables.

One thing is not answered from the tables: spec-level `complete` run-scripts. A `run=` block
shells out, which this package will not do on a Tab. File, directory, executable, and command
completion need no script — `Answer` derives them from a `complete` block's declared type and
from value names like `<FILE>` and `<DIR>`, and asks the shell to complete the path itself.
