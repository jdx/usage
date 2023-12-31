#  upgrade
## Usage
```
 upgrade [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to upgrade
e.g.: node@20 python@3.10
If not specified, all current tools will be upgraded
## Flags
- `-n,--dry-run`: Just print what would be done, don't actually do it
- `-j,--jobs <JOBS>`: Number of jobs to run in parallel
[default: 4]
- `-i,--interactive`: Display multiselect menu to choose which tools to upgrade
- `--raw`: Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
