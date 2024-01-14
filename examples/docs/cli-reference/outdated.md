### `outdated`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to show outdated versions for
e.g.: node@20 python@3.10
If not specified, all tools in global and local configs will be shown

Shows outdated tool versions
Examples:
  $ mise outdated
  Plugin  Requested  Current  Latest
  python  3.11       3.11.0   3.11.1
  node    20         20.0.0   20.1.0

  $ mise outdated node
  Plugin  Requested  Current  Latest
  node    20         20.0.0   20.1.0
