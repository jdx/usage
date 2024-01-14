# `mise activate`
#### Args

* `[SHELL_TYPE]` – Shell type to generate the script for

#### Flags

* `-s,--shell <SHELL>` – Shell type to generate the script for
* `--status` – Show "mise: <PLUGIN>@<VERSION>" message when changing directories
* `-q,--quiet` – Suppress non-error messages
Initializes mise in the current shell session

This should go into your shell's rc file.
Otherwise, it will only take effect in the current session.
(e.g. ~/.zshrc, ~/.bashrc)

This is only intended to be used in interactive sessions, not scripts.
mise is only capable of updating PATH when the prompt is displayed to the user.
For non-interactive use-cases, use shims instead.

Typically this can be added with something like the following:

    echo 'eval "$(mise activate)"' >> ~/.zshrc

However, this requires that "mise" is in your PATH. If it is not, you need to
specify the full path like this:

    echo 'eval "$(/path/to/mise activate)"' >> ~/.zshrc
Examples:
  $ eval "$(mise activate bash)"
  $ eval "$(mise activate zsh)"
  $ mise activate fish | source
  $ execx($(mise activate xonsh))
