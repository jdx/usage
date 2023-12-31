#  shell
## Usage
```
 shell [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to use
## Flags
- `-j,--jobs <JOBS>`: Number of jobs to run in parallel
[default: 4]
- `--raw`: Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
- `-u,--unset`: Removes a previously set version
