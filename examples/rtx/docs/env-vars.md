#  env-vars
## Usage
```
 env-vars [flags] [args]
```
## Args
- `[ENV_VARS]...`: Environment variable(s) to set
e.g.: NODE_ENV=production
## Flags
### --file <FILE>
The TOML file to update

Defaults to RTX_DEFAULT_CONFIG_FILENAME environment variable, or ".rtx.toml".
### --remove <ENV_VAR>
Remove the environment variable from config file

Can be used multiple times.
