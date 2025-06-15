# `complete`

```sh
# use a custom completion command for all args named "plugin"
complete "plugin" run="mycli plugins list"
```

## Descriptions

If you set `descriptions=#true`, you can provide descriptions for the completions:

```sh
complete "plugin" run="mycli plugins list" descriptions=#true
```

Results will be split on ":" with the first part being the completion value and the second part
being the description, e.g.:

```
user:User's full name
port:Port number
```

":" can be escaped with a backslash.

## Templates

The run can be customized with [tera](https://keats.github.io/tera/) templates. The following values are available:

- `words`: A list of all words currently in the prompt. Individual words can be accessed `words[1]`
- `CURRENT`: The index of the word currently being typed, combine with `words` to get the current word e.g. `words[CURRENT]`.
- `PREV`: The index of the previous word in the prompt (CURRENT-1), combine with `words` to get the previous word e.g. `words[PREV]`.

Example of completing the second argument based on the first:

```sh
arg "<module>"
arg "<controller>"
complete "module" run="ls modules"
complete "controller" run="ls modules/{{words[PREV]}}/controllers"
```

Example of using multiple words (one, two, three) for the completions of the forth argument:

```sh
arg "<one>"
arg "<two>"
arg "<three>"
arg "<four>"
complete "four" run="echo {{ words | slice(start=-4) | join(sep='\"\n\"') }}"
```

Here we just use simple commands like `ls` and `echo` but these words could be passed to any command.
