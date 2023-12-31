#  ls
## Usage
```
 ls [flags] [args]
```
## Args
- `[PLUGIN]...`: Only show tool versions from [PLUGIN]
## Flags
- `-c,--current`: Only show tool versions currently specified in a .tool-versions/.rtx.toml
- `-g,--global`: Only show tool versions currently specified in a the global .tool-versions/.rtx.toml
- `-i,--installed`: Only show tool versions that are installed Hides missing ones defined in .tool-versions/.rtx.toml but not yet installed
- `-J,--json`: Output in json format
- `-m,--missing`: Display missing tool versions
- `--prefix <PREFIX>`: Display versions matching this prefix
- `--no-header`: Don't display headers
