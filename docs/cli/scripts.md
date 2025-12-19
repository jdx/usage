# Usage Scripts

Scripts can be used with the Usage CLI to display help, powerful arg parsing, and autocompletion in
any language.
For this to work, we add comments to the script that describe the flags and arguments that the
script accepts.
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
