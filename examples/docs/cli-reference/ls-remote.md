# `mise ls-remote`

List runtime versions available for install

note that the results are cached for 24 hours
run `mise cache clean` to clear the cache and get fresh results

###### Arg `[TOOL@VERSION]`

Plugin to get versions for

###### Arg `[PREFIX]`

The version prefix to use when querying the latest version
same as the first argument after the "@"

##### Flag `--all`

Show all installed plugins and versionsExamples:
  $ mise ls-remote node
  18.0.0
  20.0.0

  $ mise ls-remote node@20
  20.0.0
  20.1.0

  $ mise ls-remote node 20
  20.0.0
  20.1.0
