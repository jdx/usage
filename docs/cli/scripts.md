# Usage Scripts

Scripts can be used with the Usage CLI to display help, powerful arg parsing, and autocompletion in any language.
For this to work, we add comments to the script that describe the flags and arguments that the script accepts.
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

```sh-session
$ ./mycli --help
Usage: mycli [flags] [args]
...
$ ./mycli -f --user=alice output.txt
$ cat output.txt
Hello, alice
```

For languages that use `//` for comments, like JavaScript, you can use `//USAGE` comments:

```js
#!/usr/bin/env -S usage node
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
