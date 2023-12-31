#  global
## Usage
```
 global [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to add to .tool-versions
e.g.: node@20
If this is a single tool with no version, the current value of the global
.tool-versions will be displayed
## Flags
- `--pin`: Save exact version to `~/.tool-versions`
e.g.: `rtx global --pin node@20` will save `node 20.0.0` to ~/.tool-versions
- `--fuzzy`: Save fuzzy version to `~/.tool-versions`
e.g.: `rtx global --fuzzy node@20` will save `node 20` to ~/.tool-versions
this is the default behavior unless RTX_ASDF_COMPAT=1
- `--remove <PLUGIN>`: Remove the plugin(s) from ~/.tool-versions
- `--path`: Get the path of the global config file
