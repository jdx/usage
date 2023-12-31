#  plugins install
## Usage
```
 plugins install [flags] [args]
```
## Args
- `[NEW_PLUGIN]`: The name of the plugin to install
e.g.: node, ruby
Can specify multiple plugins: `rtx plugins install node ruby python`
- `[GIT_URL]`: The git url of the plugin
## Flags
- `-f,--force`: Reinstall even if plugin exists
- `-a,--all`: Install all missing plugins
This will only install plugins that have matching shorthands.
i.e.: they don't need the full git repo url
- `-v,--verbose`: Show installation output
