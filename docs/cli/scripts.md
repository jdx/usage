# Usage Scripts

A script can have `--help`, parsed arguments, and tab completion without a line of parsing code
in it. The spec lives in comments at the top of the file, and a `usage` shebang runs the script
through the parser first. Each flag and argument reaches the script as an environment variable
named `usage_<name>`.

::: tip Enabling autocompletion
Tab completion for shebang scripts is one line of setup: `source <(usage g completion-init bash)`
in `~/.bashrc` enables `<Tab>` on every `usage`-shebang script on `$PATH`. See
[Generating Completion Scripts](./completions.md#shebang-scripts) for zsh and fish.
:::

In bash:

```bash
#!/usr/bin/env -S usage bash
#USAGE flag "-f --force" help="Overwrite existing <file>"
#USAGE flag "-u --user <user>" help="User to run as"
#USAGE arg "<file>" help="The file to write" default="file.txt"

if [ "$usage_force" = "true" ]; then
  rm -f "$usage_file"
fi
if [ -n "$usage_user" ]; then
  echo "Hello, $usage_user" >> "$usage_file"
else
  echo "Hello, world" >> "$usage_file"
fi
```

With the script at `./mycli`:

```bash
$ ./mycli --help
Usage: mycli [flags] [args]
...
$ ./mycli -f --user=alice output.txt
$ cat output.txt
Hello, alice
```

A language without a dedicated command goes through `usage exec`, which names the interpreter to
run. The comment prefix follows the language, so JavaScript uses `//USAGE`:

```js
#!/usr/bin/env -S usage exec node
//USAGE flag "-f --force" help="Overwrite existing <file>"
//USAGE flag "-u --user <user>" help="User to run as"
//USAGE arg "<file>" help="The file to write" default="file.txt"

const fs = require("fs");

const { usage_user, usage_force, usage_file } = process.env;

if (usage_force === "true") {
  fs.rmSync(usage_file, { force: true });
}

const user = usage_user ?? "world";
fs.appendFileSync(usage_file, `Hello, ${user}\n`);
```

## Short Flag Chaining

Single-character flags can be bundled into one word, so `-abc` means `-a -b -c`:

```bash
#!/usr/bin/env -S usage bash
#USAGE flag "-a" help="Option A"
#USAGE flag "-b" help="Option B"
#USAGE flag "-c" help="Option C"

if [ "$usage_a" = "true" ]; then
  echo "Option A is set"
fi
if [ "$usage_b" = "true" ]; then
  echo "Option B is set"
fi
if [ "$usage_c" = "true" ]; then
  echo "Option C is set"
fi
```

```bash
$ ./mycli -abc
Option A is set
Option B is set
Option C is set
```

## Shell Escaping

### `var=#true`

An environment variable holds one string, so a flag or argument declared `var=#true` arrives as
its values joined with spaces. A value that itself contains a space is quoted, as
[`shell_words::join()`](https://docs.rs/shell-words/latest/shell_words/fn.join.html) quotes it,
so `eval set -- "$usage_files"` recovers the list in a POSIX shell. The joining is not
configurable yet; [issue 189](https://github.com/jdx/usage/issues/189) tracks alternatives.

## Windows

`usage bash ./mycli` runs whatever `bash` Windows resolves to, and the executable search order
there puts the system directory ahead of `PATH`. Installing WSL puts `bash.exe` in that
directory, so on such a machine `bash` is the WSL launcher no matter what else is installed —
and WSL cannot open a Windows path:

```console
$ usage bash C:/work/mycli
/bin/bash: C:/work/mycli: No such file or directory
```

Two ways out. Passing the script by a **relative path** works, because the launcher translates
the working directory. Or name the shell you actually meant:

```batch
:: Command Prompt
set USAGECLI_SHELL_BASH=C:\Program Files\Git\bin\bash.exe
```

```powershell
# PowerShell
$env:USAGECLI_SHELL_BASH = 'C:\Program Files\Git\bin\bash.exe'
```

Each shell subcommand reads the variable for the program it runs:

| Command            | Variable              |
| ------------------ | --------------------- |
| `usage bash`       | `USAGECLI_SHELL_BASH` |
| `usage zsh`        | `USAGECLI_SHELL_ZSH`  |
| `usage fish`       | `USAGECLI_SHELL_FISH` |
| `usage powershell` | `USAGECLI_SHELL_PWSH` |

`usage powershell` runs `pwsh`, so its variable is named for that — which also lets you point it
at `powershell.exe` on a machine that only has Windows PowerShell.

The value is a program: an absolute path, or a name to look up on `PATH`. It is not a command
line, so it takes no arguments and needs no quoting even where the path contains spaces. An
empty or whitespace-only value reads the same as an unset one. The variable is inherited by the
script, so a script
that invokes `usage` again gets the same shell.

### Why not `USAGE_SHELL_BASH`

That was the original spelling and is still read, so nothing that set it needs changing. The
`USAGECLI_` one exists because `USAGE_` is not usage's to take: a spec's values reach a script
as `usage_<arg>`, and Windows environment variable names are case-insensitive, so
`USAGE_SHELL_BASH` and a spec's own `shell_bash` argument are one variable there. The same
applies to `USAGECLI_DEBUG`, `USAGECLI_TRACE` and `USAGECLI_LOG`, whose old names collide with
the very ordinary argument names `debug`, `trace` and `log`.

It also matters under mise, which clears `usage_*` from a task's environment so its own parsed
arguments cannot leak in — comparing the first six characters, case-insensitively, which is why
no `USAGE_…` spelling escapes it. A `USAGE_SHELL_BASH` set for `mise run` never reaches the
task; a `USAGECLI_SHELL_BASH` does.

`usage exec` needs none of this — it already names the interpreter, so a shebang can point
straight at one:

```bash
#!/usr/bin/env -S usage exec "C:/msys64/usr/bin/bash.exe"
```
