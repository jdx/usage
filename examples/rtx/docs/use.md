#  use
## Usage
```
 use [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to add to config file
e.g.: node@20
If no version is specified, it will default to @latest
## Flags
- `-f,--force`: Force reinstall even if already installed
- `--fuzzy`: Save fuzzy version to config file
e.g.: `rtx use --fuzzy node@20` will save 20 as the version
this is the default behavior unless RTX_ASDF_COMPAT=1
- `-g,--global`: Use the global config file (~/.config/rtx/config.toml) instead of the local one
- `-e,--env <ENV>`: Modify an environment-specific config file like .rtx.<env>.toml
- `-j,--jobs <JOBS>`: Number of jobs to run in parallel
[default: 4]
- `--raw`: Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
- `--remove <TOOL>`: Remove the tool(s) from config file
- `-p,--path <PATH>`: Specify a path to a config file or directory If a directory is specified, it will look for .rtx.toml (default) or .tool-versions
- `--pin`: Save exact version to config file
e.g.: `rtx use --pin node@20` will save 20.0.0 as the version
