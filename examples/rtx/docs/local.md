#  local
## Usage
```
 local [flags] [args]
```
## Args
- `[TOOL@VERSION]...`: Tool(s) to add to .tool-versions/.rtx.toml
e.g.: node@20
if this is a single tool with no version,
the current value of .tool-versions/.rtx.toml will be displayed
## Flags
- `-p,--parent`: Recurse up to find a .tool-versions file rather than using the current directory only
by default this command will only set the tool in the current directory ("$PWD/.tool-versions")
- `--pin`: Save exact version to `.tool-versions`
e.g.: `rtx local --pin node@20` will save `node 20.0.0` to .tool-versions
- `--fuzzy`: Save fuzzy version to `.tool-versions` e.g.: `rtx local --fuzzy node@20` will save `node 20` to .tool-versions This is the default behavior unless RTX_ASDF_COMPAT=1
- `--remove <PLUGIN>`: Remove the plugin(s) from .tool-versions
- `--path`: Get the path of the config file
