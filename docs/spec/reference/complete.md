# `complete`

```kdl
// use a custom completion command for all args named "plugin"
complete "plugin" run="mycli plugins list"
```

## `type` — completions usage supplies itself

Instead of a `run`, a completer can name something usage already knows how to complete. `run`
and `type` are alternatives; setting both is an error.

```kdl
complete "path" type="file"
complete "key" type="config_keys"
complete "value" type="config_values"
```

| type            | completes                                                            |
| --------------- | -------------------------------------------------------------------- |
| `file`, `path`  | files and directories, relative to the working directory             |
| `dir`           | directories only                                                     |
| `executable`    | executable paths                                                     |
| `command`       | commands known to the shell, including names found on `PATH`         |
| `command_args`  | a command for the first value, then ordinary argument paths          |
| `config_keys`   | the settings this spec's [`config`](./config.md) block declares      |
| `config_values` | the values accepted by the setting named earlier on the command line |

An arg or flag with no completer of its own falls back to `file` unless its declared
`choices` say otherwise, so `type="file"` is only worth writing to be explicit.

### Completing settings

`config_keys` and `config_values` are what a `config get`/`config set` pair wants, and they
need no `run`: the spec already says what the keys are and what each accepts.

```kdl
complete "key" type="config_keys"
complete "value" type="config_values"

cmd "config" {
    cmd "set" {
        arg "<key>"
        arg "<value>"
    }
}
```

`config_keys` offers every `prop` in the block, dotted keys and all, with its `help` as the
description. A `hide`d setting is left out; a `deprecated` one is still offered — a config
file in the wild names it, so it must be completable — with its description saying so.

`config_values` looks back along the command line for the last word that names a setting, so
it does not matter where the key sits relative to the cursor. It offers that setting's
`choices` with their own help, or `true` and `false` for a boolean. For anything else it stays
quiet and the file fallback applies, which is what a path-valued setting wants anyway.

Both are _closed_: where the spec enumerates the candidates, a prefix matching none of them
completes to nothing rather than falling back to filenames. A setting whose values the spec
does not enumerate keeps the fallback — including a union like `bool|path`, where `true` and
`false` are offered but a path is still valid, so a path prefix still completes.

Descriptions are reduced to their first line, since one candidate is one row of a menu.

## Descriptions

If you set `descriptions=#true`, you can provide descriptions for the completions:

```kdl
complete "plugin" run="mycli plugins list" descriptions=#true
```

Results will be split on ":" with the first part being the completion value and the second part
being the description, e.g.:

```
user:User's full name
port:Port number
```

":" can be escaped with a backslash.

## Templates

The run can be customized with [tera](https://keats.github.io/tera/) templates. The following values are available:

- `words`: A list of all words currently in the prompt. Individual words can be accessed `words[1]`
- `CURRENT`: The index of the word currently being typed, combine with `words` to get the current word e.g. `words[CURRENT]`.
- `PREV`: The index of the previous word in the prompt (CURRENT-1), combine with `words` to get the previous word e.g. `words[PREV]`.

Values interpolated into `run` are not escaped automatically. Pass every typed word that
becomes one shell argument through `shell_quote`; the filter emits one POSIX-shell-safe word,
including spaces, quotes, substitutions, and command separators as literal data:

```kdl
complete "package" run="mycli complete --query={{ words[CURRENT] | shell_quote }}"
```

Leaving off the filter is appropriate only when the interpolation is deliberately shell
syntax. `shell_quote` accepts strings; quote list members individually rather than quoting a
joined command line.

Example of completing the second argument based on the first:

```kdl
arg "<module>"
arg "<controller>"
complete "module" run="ls modules"
complete "controller" run="ls modules/{{ words[PREV] | shell_quote }}/controllers"
```

Example of using multiple words (one, two, three) for the completions of the forth argument:

```kdl
arg "<one>"
arg "<two>"
arg "<three>"
arg "<four>"
complete "four" run="echo {{ words | slice(start=-4) | join(sep='\"\n\"') }}"
```

Here we just use simple commands like `ls` and `echo` but these words could be passed to any command.

## Which shell runs `run`

`run` is executed with `sh -c`, so it is a POSIX shell command line: pipelines, `;` sequences
and shell builtins all work.

On Windows a POSIX shell is not guaranteed. usage still runs `sh -c` when `sh` is on `PATH`
(Git for Windows provides it), and falls back to `cmd /c` when it is not. `cmd` cannot run any
of the above — it only handles a plain command invocation — so a spec that targets Windows
should either keep `run` to a single command or state that it needs a POSIX shell.

The script is run with stdin closed and stderr inherited, and with `__USAGE` set to the usage
version so a script can tell it was invoked by usage.
