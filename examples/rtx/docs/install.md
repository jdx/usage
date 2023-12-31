#  install
## Usage
```
 install [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to install e.g.: node@20
## Flags
- `-f,--force`: Force reinstall even if already installed
- `-j,--jobs <JOBS>`: Number of jobs to run in parallel
[default: 4]
- `--raw`: Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
- `-v,--verbose`: Show installation output
