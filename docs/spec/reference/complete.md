# `complete`

```sh
# use a custom completion command for all args named "plugin"
complete "plugin" run="mycli plugins list"
```

## Descriptions

If you set `descriptions=true`, you can provide descriptions for the completions:

```sh
complete "plugin" run="mycli plugins list" descriptions=true
```

Results will be split on ":" with the first part being the completion value and the second part being the description, e.g.:

```
user:User's full name
port:Port number
```

":" can be escaped with a backslash.
