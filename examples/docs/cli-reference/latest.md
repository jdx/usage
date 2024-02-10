# `mise latest`

Gets the latest available version for a plugin

###### Arg `<TOOL@VERSION>`

(required)Tool to get the latest version of

###### Arg `[ASDF_VERSION]`

The version prefix to use when querying the latest version same as the first argument after the "@" used for asdf compatibility

##### Flag `-i --installed`

Show latest installed instead of available versionExamples:
  $ mise latest node@20  # get the latest version of node 20
  20.0.0

  $ mise latest node     # get the latest stable version of node
  20.0.0
