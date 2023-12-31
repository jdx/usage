#  exec
## Usage
```
 exec [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to start e.g.: node@20 python@3.10
- `[COMMAND]...`: Command string to execute (same as --command)
## Flags
- `-c,--command <C>`: Command string to execute
- `-C,--cd <CD>`: Change to this directory before executing the command
- `-j,--jobs <JOBS>`: Number of jobs to run in parallel
[default: 4]
- `--raw`: Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
