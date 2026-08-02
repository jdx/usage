# `complete`

```kdl
// use a custom completion command for all args named "plugin"
complete "plugin" run="mycli plugins list"
```

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

Example of completing the second argument based on the first:

```kdl
arg "<module>"
arg "<controller>"
complete "module" run="ls modules"
complete "controller" run="ls modules/{{words[PREV]}}/controllers"
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
