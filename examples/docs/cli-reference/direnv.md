# `mise direnv`

Output direnv function to use mise inside direnv

See https://mise.rtx.dev/direnv.html for more information

Because this generates the legacy files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when legacy file versions change.