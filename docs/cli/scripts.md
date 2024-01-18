# Usage Scripts

Scripts can be used with the Usage CLI to display help, powerful arg parsing, and autocompletion in any language.
The Usage CLI can be used with "double-shebang" scripts which contain both the Usage definition and script in a
single file. Here is an example in bash:

```bash
#!/usr/bin/env usage
flag "-f,--force" help="Overwrite existing <file>"
flag "-u,--user <user>" help="User to run as"
arg "<file>" help="The file to write" default="file.txt"

#!/usr/bin/env bash
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
