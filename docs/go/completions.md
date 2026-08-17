# Completions

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
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
	Cmd           *argv.Command   // the command in scope
	Chain         []*argv.Command // root → here
	FlagsPossible bool            // false past `--`
	AwaitingValue *argv.Flag      // cursor is inside this flag's value
	NextArg       *argv.Arg       // the positional that would bind next
	HelpTopic     bool            // after `help`: completing a topic, not a command to run
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
leaves the wiring to you today:

1. **Recognize the hidden subcommand** before normal parsing (check `args[0]`).
2. **Split the line** into completed words plus the partial word under the cursor.
3. Call `Walk` → `Candidates` → `RenderAnswer` and print the result.
4. **Generate the install scripts** from your spec with the Rust CLI:
   `usage g completion bash mycli --file mycli.usage.kdl` (and zsh/fish/…).

Also not carried into the generated tables yet: spec-level `complete` run-scripts (only
`choices` are known to `Candidates`) and `value_hint` (derive an `Answer.Files` request
yourself where you want path completion).
