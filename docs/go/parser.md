# The Parser

Generated `Parse` is the front door, but the event-level API underneath is public and stable —
use it when you need custom binding, a REPL, or completion positions.

```go
p := argv.New(mycli.Root, os.Args[1:])
for p.Next() {
	switch ev := p.Event(); ev.Kind {
	case argv.KindCommand:
		// ev.Command was selected
	case argv.KindFlag:
		// ev.Flag; ev.Value if ev.HasValue; ev.Negated for --no-* spellings
	case argv.KindArg:
		// ev.Value filled ev.Arg
	}
}
if err := p.Err(); err != nil {
	// a failure, or a help/version request
}
```

`New` returns the parser by value — it lives on the stack, and a parse performs **zero heap
allocations**, on success and failure paths alike (pinned by a `testing.AllocsPerRun` test).
An error is terminal: events already yielded are not a partial result; discard the attempt.

`Event.Value` aliases the argv strings — it's the raw OS bytes and is **not guaranteed to be
valid UTF-8**. Validate where you build your target type.

## Grammar

The behavior below is pinned vector-by-vector in the shared conformance corpus, so it's the same
grammar usage-lib and the Rust framework parse:

| Input                                 | Result                                                                      |
| ------------------------------------- | --------------------------------------------------------------------------- |
| `--jobs=8`, `--jobs 8`, `-j8`, `-j=8` | the flag with value `8`                                                     |
| `--jobs=`                             | empty string **is** a value (`HasValue` true)                               |
| `--jobs=a=b`                          | value `a=b` — only the first `=` splits                                     |
| `-fv`                                 | a short-flag bundle: `force`, then `verbose`                                |
| `-fj8`                                | a value-taking short ends the bundle: `force`, `jobs=8`                     |
| `--no-color`                          | the `color` flag with `Negated` set                                         |
| `--jobs -1`                           | negative numbers are the one detached dash-token accepted as a value        |
| `-- --force`                          | everything after `--` is positional                                         |
| `-`                                   | a bare `-` binds as a value where it was typed                              |
| `install` / `i`                       | command descent (aliases included)                                          |
| `--force install`                     | a subcommand's flags are not in scope above it                              |
| `other install`                       | only the descent position routes — `install` here is a value                |
| `--include a b`                       | a variadic flag collects values until a flag-like token                     |
| `--for`                               | no abbreviation inference — unknown flags fall through as values by default |

Unknown flags are governed per command by the spec's `unknown_flags`: the default `"value"` lets
the token fall through to the positionals (specs often wrap someone else's flags); `"error"`
rejects it — and rejects a bundle like `-fz` **whole**, with no partial `-f` event.

`--help`/`-h` arrive as ordinary flag events pointing at the package-level `argv.HelpShort` /
`argv.HelpLong` flags. The bare word `help` is a question rather than a command: it stops the
parse with `CodeHelp` and `Error.Cmd` set to the command asked about. `Error.Long` distinguishes
`--help` from `-h`.

Counting flags need nothing from the parser — each occurrence is its own event, and the caller
tallies (generated code does `field++`).

## Limits

- `argv.MaxDepth` is 16; deeper command trees fail with `CodeTooDeep`. (mise is 4 deep.)
- `Flag.Shorts` must be ASCII — a non-ASCII short can never match, and the rest of a bundle
  after a value-taking short would begin mid-character.

## Hand-written tables

Tables are plain data (`argv.Command`, `argv.Flag`, `argv.Arg`), so writing them by hand is
supported — but the generator is the intended path. If you do write them by hand: keys must be
dense starting from 1, since `Metadata` and `HelpTable` are indexed by key and `Lookup` returns
`nil` on drift rather than a neighbor.
