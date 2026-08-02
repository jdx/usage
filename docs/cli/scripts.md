# Usage Scripts

Scripts can be used with the Usage CLI to display help, powerful arg parsing, and autocompletion in
any language.
For this to work, we add comments to the script that describe the flags and arguments that the
script accepts.

::: tip Enabling autocompletion
Tab-completion for shebang scripts is opt-in: add
`source <(usage g completion-init bash)` to your `~/.bashrc` (one-time setup)
to enable `<Tab>` on every `usage`-shebang script on `$PATH`. See
[Generating Completion Scripts](./completions.md) for details and other shells.
:::
Here is an example in bash:

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

Assuming this script was located at `./mycli`, it could be used like this:

```bash
$ ./mycli --help
Usage: mycli [flags] [args]
...
$ ./mycli -f --user=alice output.txt
$ cat output.txt
Hello, alice
```

For languages that use `//` for comments, like JavaScript, you can use `//USAGE` comments:

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

Short flag chaining allows you to combine multiple single-character flags into a single argument.
This can make command-line usage more concise and easier to type.

For example, consider the following script:

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

Assuming this script was located at `./mycli`, it could be used like this:

```bash
$ ./mycli -abc
Option A is set
Option B is set
Option C is set
```

In this example, the `-a`, `-b`, and `-c` flags are combined into a single `-abc` argument, enabling all three options at once.

## Shell Escaping

### `var=#true`

When using `var=#true`, the value will be a single string (because that's all env vars can do)
delimited
by spaces. If an arg itself has a space, then it will have quotes around it. This logic is handled
by [`shell_words::join()`](https://docs.rs/shell-words/latest/shell_words/fn.join.html). For now,
this is not customizable behavior. It would be possible to
support [alternatives](https://github.com/jdx/usage/issues/189) though.

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
set USAGE_SHELL_BASH=C:\Program Files\Git\bin\bash.exe
```

```powershell
# PowerShell
$env:USAGE_SHELL_BASH = 'C:\Program Files\Git\bin\bash.exe'
```

Each shell subcommand reads the variable for the program it runs:

| Command            | Variable           |
| ------------------ | ------------------ |
| `usage bash`       | `USAGE_SHELL_BASH` |
| `usage zsh`        | `USAGE_SHELL_ZSH`  |
| `usage fish`       | `USAGE_SHELL_FISH` |
| `usage powershell` | `USAGE_SHELL_PWSH` |

`usage powershell` runs `pwsh`, so its variable is named for that — which also lets you point it
at `powershell.exe` on a machine that only has Windows PowerShell.

The value is a program: an absolute path, or a name to look up on `PATH`. It is not a command
line, so it takes no arguments and needs no quoting even where the path contains spaces. An
empty or whitespace-only value reads the same as an unset one. The variable is inherited by the
script, so a script
that invokes `usage` again gets the same shell.

`usage exec` needs none of this — it already names the interpreter, so a shebang can point
straight at one:

```bash
#!/usr/bin/env -S usage exec "C:/msys64/usr/bin/bash.exe"
```
