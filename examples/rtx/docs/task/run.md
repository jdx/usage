#  task run
## Usage
```
 task run [flags] [args]
```
## Args
- `[TASK]`: Task to run
Can specify multiple tasks by separating with `:::`
e.g.: rtx run task1 arg1 arg2 ::: task2 arg1 arg2
- `[ARGS]...`: Arguments to pass to the task. Use ":::" to separate tasks
## Flags
- `-C,--cd <CD>`: Change to this directory before executing the command
- `-n,--dry-run`: Don't actually run the task(s), just print them in order of execution
- `-f,--force`: Force the task to run even if outputs are up to date
- `-p,--prefix`: Print stdout/stderr by line, prefixed with the task's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `RTX_TASK_OUTPUT` env var
- `-i,--interleave`: Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `RTX_TASK_OUTPUT` env var
- `-t,--tool <TOOL@VERSION>`: Tool(s) to also add e.g.: node@20 python@3.10
- `-j,--jobs <JOBS>`: Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `RTX_JOBS` env var
- `-r,--raw`: Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `RTX_RAW` env var
