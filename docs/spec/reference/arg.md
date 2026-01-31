# `arg`

```sh
arg "<file>"                             # positional arg, completed as a filename
arg "<dir>"                              # positional arg, completed as a directory
arg "[file]"                             # optional positional arg
arg "<file>" default="file.txt"          # default value for arg
arg "<file>" env="MY_FILE"               # arg can be backed by an env var
arg "<file>" parse="mycli parse-file {}" # parse arg value with external command

arg "[file]" var=#true # multiple args can be passed (e.g. mycli file1 file2 file3) (0 or more)
arg "<file>" var=#true # multiple args can be passed (e.g. mycli file1 file2 file3) (1 or more)
arg "<file>..."        # shorthand for var=#true (trailing ellipsis)
arg "<file>" var=#true var_min=3 # at least 3 args must be passed
arg "<file>" var=#true var_max=3 # up to 3 args can be passed
```

## Using Variadic Args in Bash

When using variadic arguments (`var=#true`), the values are passed as a shell-escaped
string via the `usage_<name>` environment variable. To properly handle arguments
containing spaces as a bash array, wrap the variable in parentheses:

```bash
# Given: usage_files="arg1 'arg with space' arg3"

# Convert to bash array:
eval "files=($usage_files)"

# Now use as array:
for f in "${files[@]}"; do
  echo "Processing: $f"
done

# Or pass to commands:
touch "${files[@]}"
```

This pattern ensures arguments with spaces are handled correctly as separate elements.

```kdl
arg "<shell>" {
  choices "bash" "zsh" "fish" # <shell> must be one of the choices
}

arg "<file>" long_help="longer help for --help (as oppoosed to -h)"

# double-dash behavior
arg "<file>" double_dash="required" # arg must be passed after a double dash (e.g. mycli -- file.txt)
arg "<file>" double_dash="optional" # arg may be passed after a double dash (e.g. mycli -- file.txt or mycli file.txt)
arg "<file>..." double_dash="automatic" # once arg is passed, behave as if a double dash was passed (e.g. mycli file.txt --filewithdash)
arg "<args>..." double_dash="preserve" # preserve double dashes as args (e.g. mycli arg1 -- arg2 -- arg3)
```
