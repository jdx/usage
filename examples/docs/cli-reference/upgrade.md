# `mise upgrade`

###### Aliases: `up`

Upgrades outdated tool versions

###### Arg `[TOOL@VERSION]...`

Tool(s) to upgrade
e.g.: node@20 python@3.10
If not specified, all current tools will be upgraded

##### Flag `-n --dry-run`

Just print what would be done, don't actually do it

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

##### Flag `-i --interactive`

Display multiselect menu to choose which tools to upgrade

##### Flag `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
