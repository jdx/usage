# `mise sync python`

#### Flags

* `--pyenv` – Get tool versions from pyenv
Symlinks all tool versions from an external tool into mise

For example, use this to import all pyenv installs into mise
Examples:
  $ pyenv install 3.11.0
  $ mise sync python --pyenv
  $ mise use -g python@3.11.0 - uses pyenv-provided python