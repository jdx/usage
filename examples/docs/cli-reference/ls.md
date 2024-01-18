# `mise ls`
* Aliases: `list`
#### Args

* `[PLUGIN]...` – Only show tool versions from [PLUGIN]

#### Flags

* `-p,--plugin <PLUGIN_FLAG>` – 
* `-c,--current` – Only show tool versions currently specified in a .tool-versions/.mise.toml
* `-g,--global` – Only show tool versions currently specified in a the global .tool-versions/.mise.toml
* `-i,--installed` – Only show tool versions that are installed Hides missing ones defined in .tool-versions/.mise.toml but not yet installed
* `--parseable` – Output in an easily parseable format
* `-J,--json` – Output in json format
* `-m,--missing` – Display missing tool versions
* `--prefix <PREFIX>` – Display versions matching this prefix
* `--no-header` – Don't display headers
List installed and/or currently selected tool versions
Examples:
  $ mise ls
  node    20.0.0 ~/src/myapp/.tool-versions latest
  python  3.11.0 ~/.tool-versions           3.10
  python  3.10.0

  $ mise ls --current
  node    20.0.0 ~/src/myapp/.tool-versions 20
  python  3.11.0 ~/.tool-versions           3.11.0

  $ mise ls --json
  {
    "node": [
      {
        "version": "20.0.0",
        "install_path": "/Users/jdx/.mise/installs/node/20.0.0",
        "source": {
          "type": ".mise.toml",
          "path": "/Users/jdx/.mise.toml"
        }
      }
    ],
    "python": [...]
  }
