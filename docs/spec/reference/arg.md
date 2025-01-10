# `arg`

```sh
arg "<file>"                             # positional arg, completed as a filename
arg "<dir>"                              # positional arg, completed as a directory
arg "[file]"                             # optional positional arg
arg "<file>" default="file.txt"          # default value for arg
arg "<file>" parse="mycli parse-file {}" # parse arg value with external command

arg "[file]" var=#true # multiple args can be passed (e.g. mycli file1 file2 file3) (0 or more)
arg "<file>" var=#true # multiple args can be passed (e.g. mycli file1 file2 file3) (1 or more)
arg "<file>" var=#true var_min=3 # at least 3 args must be passed
arg "<file>" var=#true var_max=3 # up to 3 args can be passed

arg "<shell>" {
  choices "bash" "zsh" "fish" # <shell> must be one of the choices
}

arg "<file>" long_help="longer help for --help (as oppoosed to -h)"

# double-dash behavior
arg "<file>" double_dash="required" # arg must be passed after a double dash (e.g. mycli -- file.txt)
arg "<file>" double_dash="optional" # arg may be passed after a double dash (e.g. mycli -- file.txt or mycli file.txt)
arg "<file>..." double_dash="automatic" # once arg is passed, behave as if a double dash was passed (e.g. mycli file.txt --filewithdash)
```
