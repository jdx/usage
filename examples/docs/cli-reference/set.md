# `mise set`

Manage environment variables

By default this command modifies ".mise.toml" in the current directory.

###### Arg `[ENV_VARS]...`

Environment variable(s) to set
e.g.: NODE_ENV=production

##### Flag `--file <FILE>`

The TOML file to update

Defaults to MISE_DEFAULT_CONFIG_FILENAME environment variable, or ".mise.toml".

##### Flag `-g --global`

Set the environment variable in the global config file

##### Flag `--remove... <ENV_VAR>`

Remove the environment variable from config file

Can be used multiple times.

Examples:

    $ mise set NODE_ENV=production

    $ mise set NODE_ENV
    production

    $ mise set
    key       value       source
    NODE_ENV  production  ~/.config/mise/config.toml
