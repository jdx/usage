# Changelog

## [0.11.0](https://github.com/jdx/usage/compare/v0.10.0..v0.11.0) - 2024-10-14

### 🚀 Features

- support single quotes in zsh descriptions by [@jasisk](https://github.com/jasisk) in [#128](https://github.com/jdx/usage/pull/128)
- render help in cli parsing by [@jdx](https://github.com/jdx) in [7c49fcb](https://github.com/jdx/usage/commit/7c49fcba4567da7ad8c7af9c4bb72a7c276a4a57)
- implemented more cli help for args/flags/subcommands by [@jdx](https://github.com/jdx) in [669f44e](https://github.com/jdx/usage/commit/669f44ea0459f997444c46ebfac1f42c00e210b4)

### 🐛 Bug Fixes

- bug with help and args by [@jdx](https://github.com/jdx) in [6c615f9](https://github.com/jdx/usage/commit/6c615f9f8b1c6798fcba3ed88890b2891505c6ec)

### 🔍 Other Changes

- use dashes in CHANGELOG by [@jdx](https://github.com/jdx) in [c458d8c](https://github.com/jdx/usage/commit/c458d8c8a4c810271ac2474fcb9412651edc8c86)
- remove dbg by [@jdx](https://github.com/jdx) in [cb6042c](https://github.com/jdx/usage/commit/cb6042cfcfec8b93b162361f5045eb94054316b8)

### New Contributors

- @jasisk made their first contribution in [#128](https://github.com/jdx/usage/pull/128)

## [0.10.0](https://github.com/jdx/usage/compare/v0.9.0..v0.10.0) - 2024-10-12

### 🚀 Features

- basic `--help` support by [@jdx](https://github.com/jdx) in [394df50](https://github.com/jdx/usage/commit/394df50623de7d497de47975267a4b7ec9377e70)

### 🔍 Other Changes

- debug output by [@jdx](https://github.com/jdx) in [53a4fe4](https://github.com/jdx/usage/commit/53a4fe4c155115e15dfe066844d83aa66c9bab83)

## [0.9.0](https://github.com/jdx/usage/compare/v0.8.4..v0.9.0) - 2024-10-12

### 🚀 Features

- put aliases in backticks by [@jdx](https://github.com/jdx) in [36b527f](https://github.com/jdx/usage/commit/36b527f8aaa9c64aadfb7dce06243625b28e091e)

### 🐛 Bug Fixes

- make `usage -v` work by [@jdx](https://github.com/jdx) in [caabb0f](https://github.com/jdx/usage/commit/caabb0f92f744bd1bcd0e1321c27649861b8ccea)
- remove quotes in zsh descriptions by [@jdx](https://github.com/jdx) in [dba5fd8](https://github.com/jdx/usage/commit/dba5fd8ec4f08938ff6fc127f3542ef48deb8ca2)

### 🔍 Other Changes

- use correct url for aur checksum by [@jdx](https://github.com/jdx) in [36d577e](https://github.com/jdx/usage/commit/36d577eca41c290d47d03ad74783870eca806788)

### 📦️ Dependency Updates

- update rust crate once_cell to v1.20.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#123](https://github.com/jdx/usage/pull/123)
- update rust crate regex to v1.11.0 by [@renovate[bot]](https://github.com/renovate[bot]) in [#124](https://github.com/jdx/usage/pull/124)
- update rust crate clap to v4.5.19 by [@renovate[bot]](https://github.com/renovate[bot]) in [#125](https://github.com/jdx/usage/pull/125)
- update rust crate once_cell to v1.20.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#126](https://github.com/jdx/usage/pull/126)

## [0.8.4](https://github.com/jdx/usage/compare/v0.8.3..v0.8.4) - 2024-09-29

### 🐛 Bug Fixes

- capitalize ARGS/FLAGS in md docs by [@jdx](https://github.com/jdx) in [3a314d5](https://github.com/jdx/usage/commit/3a314d5bcb7a1552a4cf2e833bd81b35a7e9e514)
- move usage out of header by [@jdx](https://github.com/jdx) in [9a43a72](https://github.com/jdx/usage/commit/9a43a72ae26606cc9c03ee718627c1a6636d77f2)

### 🔍 Other Changes

- fix aur by [@jdx](https://github.com/jdx) in [56a0cf7](https://github.com/jdx/usage/commit/56a0cf7250890dd7147e41d69f3942150fdbd5d5)

## [0.8.3](https://github.com/jdx/usage/compare/v0.8.2..v0.8.3) - 2024-09-28

### 🐛 Bug Fixes

- minor whitespace bug in md output by [@jdx](https://github.com/jdx) in [dcced73](https://github.com/jdx/usage/commit/dcced7300a3abfd2cde2eee2879d27fa30b50694)
- added aliases to command info by [@jdx](https://github.com/jdx) in [ac745d6](https://github.com/jdx/usage/commit/ac745d66215566500faa684b93192392bf307521)
- tweak usage output by [@jdx](https://github.com/jdx) in [c488b76](https://github.com/jdx/usage/commit/c488b76249c6ab6eb022cc022567faed82332074)
- make html_encode optional by [@jdx](https://github.com/jdx) in [cc629ee](https://github.com/jdx/usage/commit/cc629ee36acbbd2fe9a4e69c4b3216334f356739)

### 🔍 Other Changes

- always remove aur repo by [@jdx](https://github.com/jdx) in [368ae97](https://github.com/jdx/usage/commit/368ae97a73ecb82fb5855fdc8610dc7e2dd17084)

## [0.8.2](https://github.com/jdx/usage/compare/v0.8.1..v0.8.2) - 2024-09-28

### 🐛 Bug Fixes

- whitespace in md generation by [@jdx](https://github.com/jdx) in [3cb7769](https://github.com/jdx/usage/commit/3cb776920cd9bd18693cdc0e547b98b0efd25aca)
- escape html in md by [@jdx](https://github.com/jdx) in [a691143](https://github.com/jdx/usage/commit/a6911436156c15246c69ea66e62e2745e419b813)
- more work on html encoding md by [@jdx](https://github.com/jdx) in [b5cb342](https://github.com/jdx/usage/commit/b5cb342fa79ac70bd2723c026f3184021e5ae3ac)

## [0.8.1](https://github.com/jdx/usage/compare/v0.8.0..v0.8.1) - 2024-09-28

### 🐛 Bug Fixes

- handle bug with usage-bin aur script by [@jdx](https://github.com/jdx) in [6e4b7a7](https://github.com/jdx/usage/commit/6e4b7a79be85d5b02285718625f6302bef75cb75)
- improving md generation by [@jdx](https://github.com/jdx) in [#117](https://github.com/jdx/usage/pull/117)

### 🔍 Other Changes

- enable brew publish by [@jdx](https://github.com/jdx) in [d8cd84a](https://github.com/jdx/usage/commit/d8cd84afbf4ae21386fda4b5a01d0adeaf7839a9)

## [0.8.0](https://github.com/jdx/usage/compare/v0.7.4..v0.8.0) - 2024-09-27

### 🚀 Features

- basic support for markdown generation in lib by [@jdx](https://github.com/jdx) in [de004c8](https://github.com/jdx/usage/commit/de004c87890bda993288503fe49e02b342c72487)

### 🔍 Other Changes

- enable aur publishing by [@jdx](https://github.com/jdx) in [0049e95](https://github.com/jdx/usage/commit/0049e950001bf8a9dfb350d5e675c474f6958d18)

## [0.7.4](https://github.com/jdx/usage/compare/v0.7.3..v0.7.4) - 2024-09-27

### 🔍 Other Changes

- fix aur publishing by [@jdx](https://github.com/jdx) in [28752c3](https://github.com/jdx/usage/commit/28752c35f310bb78e45ab67c11b905e8af28b6c4)

## [0.7.3](https://github.com/jdx/usage/compare/v0.7.2..v0.7.3) - 2024-09-27

### 🔍 Other Changes

- fix aur publishing by [@jdx](https://github.com/jdx) in [9e21529](https://github.com/jdx/usage/commit/9e21529ba1e4ed3f1ae4c69a480cf801ff311c1a)

## [0.7.2](https://github.com/jdx/usage/compare/v0.7.1..v0.7.2) - 2024-09-27

### 🔍 Other Changes

- set GITHUB_TOKEN by [@jdx](https://github.com/jdx) in [fc7d06f](https://github.com/jdx/usage/commit/fc7d06ff15ca7b72d421fd3706c22b9e632b2224)
- fix codesign config by [@jdx](https://github.com/jdx) in [cf0b731](https://github.com/jdx/usage/commit/cf0b7311806d60b9d1e79c671958205156818311)

## [0.7.1](https://github.com/jdx/usage/compare/v0.7.0..v0.7.1) - 2024-09-27

### 🐛 Bug Fixes

- fail parsing if required args/flags not found by [@jdx](https://github.com/jdx) in [409145a](https://github.com/jdx/usage/commit/409145ae5db937bffa121e63f00f8f827c49b294)

### 🔍 Other Changes

- publish aur releases by [@jdx](https://github.com/jdx) in [#109](https://github.com/jdx/usage/pull/109)
- move tasks dir by [@jdx](https://github.com/jdx) in [8cb8cc3](https://github.com/jdx/usage/commit/8cb8cc348dbb04f3c41f3ca22c518f82dfa27830)
- install cargo-binstall before installing mise by [@jdx](https://github.com/jdx) in [6240460](https://github.com/jdx/usage/commit/62404602e602a1c7d578b5764703f0820c45299e)

## [0.7.0](https://github.com/jdx/usage/compare/v0.6.0..v0.7.0) - 2024-09-27

### 🚀 Features

- implemented choices for args/flags by [@jdx](https://github.com/jdx) in [#107](https://github.com/jdx/usage/pull/107)

### 🔍 Other Changes

- clean up pub exports by [@jdx](https://github.com/jdx) in [9996ab8](https://github.com/jdx/usage/commit/9996ab8ca041d27a0754096fe7b04ebd3958431b)

## [0.6.0](https://github.com/jdx/usage/compare/v0.5.1..v0.6.0) - 2024-09-26

### 🚀 Features

- negate by [@jdx](https://github.com/jdx) in [5d1b817](https://github.com/jdx/usage/commit/5d1b817d143227a03651502b7671c9b2853c92eb)
- negate by [@jdx](https://github.com/jdx) in [16f754d](https://github.com/jdx/usage/commit/16f754d1925c561198291b304cbf80c9ab2a4dee)
- mount by [@jdx](https://github.com/jdx) in [99530f4](https://github.com/jdx/usage/commit/99530f4682140e2b64f2625d844b840925e3d6ae)

### 🐛 Bug Fixes

- remove debug statements by [@jdx](https://github.com/jdx) in [664b592](https://github.com/jdx/usage/commit/664b592f4d8f7b96f24d3bb2ca2803df36fda512)
- export SpecMount by [@jdx](https://github.com/jdx) in [b44c4f1](https://github.com/jdx/usage/commit/b44c4f15c77dee10e59c136b52f52a844f4ee655)

### 🔍 Other Changes

- migrate away from deprecated git-cliff syntax by [@jdx](https://github.com/jdx) in [3062df9](https://github.com/jdx/usage/commit/3062df94a9ad7af3a2e57ba5e5e35d299daa6718)

## [0.5.1](https://github.com/jdx/usage/compare/v0.5.0..v0.5.1) - 2024-09-25

### 🐛 Bug Fixes

- bail instead of panic on CLI parse error by [@jdx](https://github.com/jdx) in [b935cca](https://github.com/jdx/usage/commit/b935ccae9a442378c71182293cd24380fdadf744)

## [0.5.0](https://github.com/jdx/usage/compare/v0.4.0..v0.5.0) - 2024-09-25

### 🚀 Features

- added .as_env() to CLI parser by [@jdx](https://github.com/jdx) in [b1f6617](https://github.com/jdx/usage/commit/b1f66179b70a4bcdc6792add24a7b62e1afdd81d)
- added Spec::parse_script fn by [@jdx](https://github.com/jdx) in [124a705](https://github.com/jdx/usage/commit/124a7050c6b1b5bb502049204556b74b6e8a4b71)

## [0.4.0](https://github.com/jdx/usage/compare/v0.3.1..v0.4.0) - 2024-09-25

### 🚀 Features

- add comment syntax for file scripts by [@jdx](https://github.com/jdx) in [ee75493](https://github.com/jdx/usage/commit/ee7549303a0cf63c5da8257287be21d0af85ce86)

### 🐛 Bug Fixes

- tweak comment syntax by [@jdx](https://github.com/jdx) in [dfff6e2](https://github.com/jdx/usage/commit/dfff6e2daaafb47200a32d4654482beabbe2f343)

### 📚 Documentation

- update flag syntax by [@jdx](https://github.com/jdx) in [a67de2e](https://github.com/jdx/usage/commit/a67de2e6e855b24d340d559ded9e1464f95c2894)

### 📦️ Dependency Updates

- update rust crate serde to v1.0.210 by [@renovate[bot]](https://github.com/renovate[bot]) in [#102](https://github.com/jdx/usage/pull/102)
- update rust crate clap to v4.5.18 by [@renovate[bot]](https://github.com/renovate[bot]) in [#101](https://github.com/jdx/usage/pull/101)

## [0.3.1](https://github.com/jdx/usage/compare/v0.3.0..v0.3.1) - 2024-08-28

### 🐛 Bug Fixes

- **(brew)** use official homebrew formula by [@jdx](https://github.com/jdx) in [#54](https://github.com/jdx/usage/pull/54)
- make shebang scripts work with comments by [@jdx](https://github.com/jdx) in [9eb2a64](https://github.com/jdx/usage/commit/9eb2a64ff0e3c463f53fe0c283bbb932e5b3dd77)

### 📦️ Dependency Updates

- update dependency vitepress to v1.2.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#72](https://github.com/jdx/usage/pull/72)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#73](https://github.com/jdx/usage/pull/73)
- update rust crate tera to v1.20.0 by [@renovate[bot]](https://github.com/renovate[bot]) in [#75](https://github.com/jdx/usage/pull/75)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#76](https://github.com/jdx/usage/pull/76)
- update dependency vitepress to v1.2.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#77](https://github.com/jdx/usage/pull/77)
- update rust crate clap to v4.5.6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#78](https://github.com/jdx/usage/pull/78)
- update rust crate clap to v4.5.7 by [@renovate[bot]](https://github.com/renovate[bot]) in [#79](https://github.com/jdx/usage/pull/79)
- update rust crate regex to v1.10.5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#80](https://github.com/jdx/usage/pull/80)
- update rust crate log to v0.4.22 by [@renovate[bot]](https://github.com/renovate[bot]) in [#82](https://github.com/jdx/usage/pull/82)
- update rust crate clap to v4.5.8 by [@renovate[bot]](https://github.com/renovate[bot]) in [#81](https://github.com/jdx/usage/pull/81)
- update rust crate serde to v1.0.204 by [@renovate[bot]](https://github.com/renovate[bot]) in [#85](https://github.com/jdx/usage/pull/85)
- update rust crate clap to v4.5.9 by [@renovate[bot]](https://github.com/renovate[bot]) in [#84](https://github.com/jdx/usage/pull/84)
- update rust crate strum to v0.26.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#86](https://github.com/jdx/usage/pull/86)
- update rust crate thiserror to v1.0.63 by [@renovate[bot]](https://github.com/renovate[bot]) in [#87](https://github.com/jdx/usage/pull/87)
- update dependency vitepress to v1.3.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#88](https://github.com/jdx/usage/pull/88)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#89](https://github.com/jdx/usage/pull/89)
- update rust crate predicates to v3.1.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#91](https://github.com/jdx/usage/pull/91)
- update rust crate assert_cmd to v2.0.15 by [@renovate[bot]](https://github.com/renovate[bot]) in [#90](https://github.com/jdx/usage/pull/90)
- update rust crate env_logger to v0.11.5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#93](https://github.com/jdx/usage/pull/93)
- update rust crate clap to v4.5.13 by [@renovate[bot]](https://github.com/renovate[bot]) in [#92](https://github.com/jdx/usage/pull/92)
- update rust crate assert_cmd to v2.0.16 by [@renovate[bot]](https://github.com/renovate[bot]) in [#94](https://github.com/jdx/usage/pull/94)
- update dependency vitepress to v1.3.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#95](https://github.com/jdx/usage/pull/95)
- update dependency vitepress to v1.3.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#96](https://github.com/jdx/usage/pull/96)
- update rust crate clap to v4.5.16 by [@renovate[bot]](https://github.com/renovate[bot]) in [#97](https://github.com/jdx/usage/pull/97)
- update dependency vitepress to v1.3.4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#98](https://github.com/jdx/usage/pull/98)
- update rust crate regex to v1.10.6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#99](https://github.com/jdx/usage/pull/99)

## [0.3.0](https://github.com/jdx/usage/compare/v0.2.1..v0.3.0) - 2024-05-26

### 🚀 Features

- complete descriptions by [@jdx](https://github.com/jdx) in [a8afca7](https://github.com/jdx/usage/commit/a8afca7d6ad773431acfde8280e9dfb2884ef4e0)

## [0.2.1](https://github.com/jdx/usage/compare/v0.2.0..v0.2.1) - 2024-05-25

### 🔍 Other Changes

- updated deps by [@jdx](https://github.com/jdx) in [a457da9](https://github.com/jdx/usage/commit/a457da9ccec4890d63f3ab8e2215e51e64fd2425)

### 📦️ Dependency Updates

- update rust crate xx to v1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#64](https://github.com/jdx/usage/pull/64)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#65](https://github.com/jdx/usage/pull/65)
- update rust crate serde to v1.0.202 by [@renovate[bot]](https://github.com/renovate[bot]) in [#68](https://github.com/jdx/usage/pull/68)
- update rust crate thiserror to v1.0.61 by [@renovate[bot]](https://github.com/renovate[bot]) in [#69](https://github.com/jdx/usage/pull/69)

## [0.2.0](https://github.com/jdx/usage/compare/v0.1.18..v0.2.0) - 2024-05-12

### 🚀 Features

- **(exec)** added `usage exec` command by [@jdx](https://github.com/jdx) in [#51](https://github.com/jdx/usage/pull/51)

### 🐛 Bug Fixes

- rust beta warning by [@jdx](https://github.com/jdx) in [8ba775e](https://github.com/jdx/usage/commit/8ba775e02daef37193fa0f43d59f4a4ad3081056)

### 🚜 Refactor

- created reusuable CLI parse function by [@jdx](https://github.com/jdx) in [8bc895a](https://github.com/jdx/usage/commit/8bc895a02ba6c7df32d47d0847b5b1985a2dbfdb)

### 📚 Documentation

- set GA by [@jdx](https://github.com/jdx) in [1a786c3](https://github.com/jdx/usage/commit/1a786c354a6e3f147453d8e6f38fb3916d21f889)
- update cliff.toml by [@jdx](https://github.com/jdx) in [df5f579](https://github.com/jdx/usage/commit/df5f579deac8d6f0fa2b0d2a492847950e338c94)

### 🔍 Other Changes

- **(aur)** added aur packaging by [@jdx](https://github.com/jdx) in [e00aff9](https://github.com/jdx/usage/commit/e00aff9739bf4c2286124cdb4724bd09f3b39a21)
- **(aur)** added aur packaging by [@jdx](https://github.com/jdx) in [e285fe9](https://github.com/jdx/usage/commit/e285fe9dcf6eabd684bb20607d64b8ebca29f663)
- **(release-plz)** fixed script by [@jdx](https://github.com/jdx) in [e4b2223](https://github.com/jdx/usage/commit/e4b2223da399ca30fa33917cf4088bb52ee7e49a)
- bump xx by [@jdx](https://github.com/jdx) in [c1bb0bb](https://github.com/jdx/usage/commit/c1bb0bb1c7600cf1ccb788c2d17651f6e93adf01)
- removed mega-linter by [@jdx](https://github.com/jdx) in [1aaa11f](https://github.com/jdx/usage/commit/1aaa11f49f9a5cd04419c2aebfb71b824f3c5ad1)
- fixing mise-action by [@jdx](https://github.com/jdx) in [c6a47fa](https://github.com/jdx/usage/commit/c6a47fa88cbd94de0fa0db2592a266b48c4c04ce)
- remove invalid config by [@jdx](https://github.com/jdx) in [eec7f7d](https://github.com/jdx/usage/commit/eec7f7d2324151bc809c45e514040dc353d544cc)
- better release PR title by [@jdx](https://github.com/jdx) in [849febb](https://github.com/jdx/usage/commit/849febbf6fc73fff6da6b3df15b9e31dad91580f)

### 📦️ Dependency Updates

- update dependency vitepress to v1.1.0 by [@renovate[bot]](https://github.com/renovate[bot]) in [#55](https://github.com/jdx/usage/pull/55)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#56](https://github.com/jdx/usage/pull/56)
- update dependency vitepress to v1.1.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#57](https://github.com/jdx/usage/pull/57)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#58](https://github.com/jdx/usage/pull/58)
- update rust crate xx to 0.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#59](https://github.com/jdx/usage/pull/59)
- update dependency vitepress to v1.1.4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#60](https://github.com/jdx/usage/pull/60)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#61](https://github.com/jdx/usage/pull/61)

## [0.1.18](https://github.com/jdx/usage/compare/v0.1.17..v0.1.18) - 2024-04-08

### 📚 Documentation

- **(changelog)** ran git-cliff by [@jdx](https://github.com/jdx) in [e2b6df1](https://github.com/jdx/usage/commit/e2b6df1b7fdb0318fa0eed709396cd202abd296b)
- improve CHANGELOG by [@jdx](https://github.com/jdx) in [#43](https://github.com/jdx/usage/pull/43)

### 🔍 Other Changes

- **(release-plz)** add all cargo files by [@jdx](https://github.com/jdx) in [6bc237d](https://github.com/jdx/usage/commit/6bc237d1babee025a0b4737781a6a742d93b7f4a)
- switch to dtolnay/rust-toolchain by [@jdx](https://github.com/jdx) in [d96d2a3](https://github.com/jdx/usage/commit/d96d2a37ff801d10868db265f26c10cf42181a11)

### 📦️ Dependency Updates

- update dependency vitepress to v1.0.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#42](https://github.com/jdx/usage/pull/42)
- update actions/configure-pages action to v5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#44](https://github.com/jdx/usage/pull/44)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#45](https://github.com/jdx/usage/pull/45)
- update dependency vitepress to v1.0.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#46](https://github.com/jdx/usage/pull/46)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#47](https://github.com/jdx/usage/pull/47)

## [0.1.17](https://github.com/jdx/usage/compare/v0.1.16..v0.1.17) - 2024-03-17

### 🔍 Other Changes

- ensure we publish the CLI by [@jdx](https://github.com/jdx) in [8b1f379](https://github.com/jdx/usage/commit/8b1f379ed94b5e85429846d0e3d1b0198a1449d1)
- bump release by [@jdx](https://github.com/jdx) in [3fa016a](https://github.com/jdx/usage/commit/3fa016a266753e9e5ebeb81eed61c74ced46e5cb)

## [0.1.16](https://github.com/jdx/usage/compare/v0.1.9..v0.1.16) - 2024-03-17

### 🐛 Bug Fixes

- **(completions)** add newline before error message by [@jdx](https://github.com/jdx) in [bbbafad](https://github.com/jdx/usage/commit/bbbafad126889ccc415e586b7601f7bb97c6f5a8)
- bug fix for release tagging by [@jdx](https://github.com/jdx) in [2c4832f](https://github.com/jdx/usage/commit/2c4832f7c7c67d8d5c477a11e56a49b487f574b8)

### 🚜 Refactor

- move usage-lib into its own dir by [@jdx](https://github.com/jdx) in [37e2379](https://github.com/jdx/usage/commit/37e2379122f123a85c4888e6efa1f62c631ac013)

### 🧪 Testing

- **(markdown-link-check)** ignore placeholder urls by [@jdx](https://github.com/jdx) in [6744453](https://github.com/jdx/usage/commit/67444538f25a11c09f842e20a5baa30fc3f41fae)
- **(markdown-link-check)** ignore placeholder urls by [@jdx](https://github.com/jdx) in [940dfb7](https://github.com/jdx/usage/commit/940dfb7cd5d1dbc8d2f1bab3029c1c4ba786f6ee)
- fix snapshots by [@jdx](https://github.com/jdx) in [0ea3d8b](https://github.com/jdx/usage/commit/0ea3d8b6ae7e3343c71c6d23b9e2b5d0f648a575)
- fix deprecation warnings by [@jdx](https://github.com/jdx) in [be8d6d5](https://github.com/jdx/usage/commit/be8d6d5b9090103d5596ff6a038ad63e538c1722)

### 🔍 Other Changes

- **(release-plz)** autopublish tag/gh release by [@jdx](https://github.com/jdx) in [5f78550](https://github.com/jdx/usage/commit/5f7855048912adda5ebfa6cfd2375cf5e5ccb79b)
- **(release-plz)** remove old logic by [@jdx](https://github.com/jdx) in [9ac8a0e](https://github.com/jdx/usage/commit/9ac8a0e95ae51398633486365a45a447bd8664e5)
- **(release-plz)** prefix versions with "v" by [@jdx](https://github.com/jdx) in [964503c](https://github.com/jdx/usage/commit/964503c57d8960abec4d6655257c1b904e585eba)
- added author field by [@jdx](https://github.com/jdx) in [b0e815a](https://github.com/jdx/usage/commit/b0e815a72bf4bfad6659a909a058cd86b7f9d56d)
- snapshots by [@jdx](https://github.com/jdx) in [3f0f16c](https://github.com/jdx/usage/commit/3f0f16c9b4fc2ff346a97644e97878916c1fa630)
- added brew tap to gh actions by [@jdx](https://github.com/jdx) in [e79f386](https://github.com/jdx/usage/commit/e79f386ff75bea7d35f3c90f0060a94656169c51)
- added git-cliff by [@jdx](https://github.com/jdx) in [6cca2bb](https://github.com/jdx/usage/commit/6cca2bbc77e459c45838e1957bc35eb42601a727)
- added release-please by [@jdx](https://github.com/jdx) in [e60127f](https://github.com/jdx/usage/commit/e60127f63a48a841b9aadfa04c9c4df045167dde)
- attempt to fix mega-linter by [@jdx](https://github.com/jdx) in [25a35e0](https://github.com/jdx/usage/commit/25a35e064c2ca29771d1c6b1ac5d2bea2b03b530)
- bootstrap release-please by [@jdx](https://github.com/jdx) in [b6a7584](https://github.com/jdx/usage/commit/b6a758421231e33582c9571aa3690936faa1e59b)
- release-plz by [@jdx](https://github.com/jdx) in [b7aa490](https://github.com/jdx/usage/commit/b7aa490d7b401d86ac11569aae824951ab4de27c)
- cargo update by [@jdx](https://github.com/jdx) in [0aa872c](https://github.com/jdx/usage/commit/0aa872ca68822d32d9fa8a5228525124ed076abb)
- remove markdown link checker since it keeps failing by [@jdx](https://github.com/jdx) in [0668a1f](https://github.com/jdx/usage/commit/0668a1f6dae63bd3ea916939ab0a4c9c58fd0c13)
- fixing cargo metadata by [@jdx](https://github.com/jdx) in [64f19d7](https://github.com/jdx/usage/commit/64f19d7d40de0f897ccd22c07cd72e74b98b435f)
- use custom release-plz logic by [@jdx](https://github.com/jdx) in [bf4c151](https://github.com/jdx/usage/commit/bf4c151205d0560eefbf7a64cefd2524c57813db)
- bump version to try another release by [@jdx](https://github.com/jdx) in [badf251](https://github.com/jdx/usage/commit/badf251feb7fe86d763e4458261060b81f85fe7e)
- set metadata for usage-lib dependency by [@jdx](https://github.com/jdx) in [7e3538a](https://github.com/jdx/usage/commit/7e3538a304372c8d010386e22d39c02c9319d297)
- added git-cliff dependency by [@jdx](https://github.com/jdx) in [afd74d0](https://github.com/jdx/usage/commit/afd74d020d86fd77fe9b0696ae63863237297009)
- bump version to try another release by [@jdx](https://github.com/jdx) in [032f686](https://github.com/jdx/usage/commit/032f6860f569874e8ca2928f7db367191a8e69b3)
- bump release by [@jdx](https://github.com/jdx) in [4f3e3ea](https://github.com/jdx/usage/commit/4f3e3ea284968006e677402bd78afd3c592698b4)
- release on tags by [@jdx](https://github.com/jdx) in [6fd60be](https://github.com/jdx/usage/commit/6fd60be73ed06d62520fd2d39f175857243ec6e7)
- bump release by [@jdx](https://github.com/jdx) in [58be1c4](https://github.com/jdx/usage/commit/58be1c40f45fa86d1d8c6c6e58cbec85451c0d40)
- bump release by [@jdx](https://github.com/jdx) in [cd92e36](https://github.com/jdx/usage/commit/cd92e366ee60d9ea2cc6b43f9dadc7f27c0dd63e)

### 📦️ Dependency Updates

- update rust crate heck to v0.5.0 by [@renovate[bot]](https://github.com/renovate[bot]) in [#30](https://github.com/jdx/usage/pull/30)
- update dependency vitepress to v1.0.0-rc.45 by [@renovate[bot]](https://github.com/renovate[bot]) in [b4b8054](https://github.com/jdx/usage/commit/b4b8054d74d9df6826e2c44b051ec4823b646c0b)

## [0.1.9](https://github.com/jdx/usage/compare/v0.1.8..v0.1.9) - 2024-02-13

### 🐛 Bug Fixes

- fix actionlint by [@jdx](https://github.com/jdx) in [725bcf9](https://github.com/jdx/usage/commit/725bcf96055aafc9f0a58e0c8affe2c0ac7f3ba9)

### 🔍 Other Changes

- improve error by [@jdx](https://github.com/jdx) in [4621457](https://github.com/jdx/usage/commit/4621457b6cccde7f01ba60afe6c33870201975be)

## [0.1.8](https://github.com/jdx/usage/compare/v0.1.7..v0.1.8) - 2024-02-10

### 🐛 Bug Fixes

- fix binstall by [@jdx](https://github.com/jdx) in [a3b4513](https://github.com/jdx/usage/commit/a3b45132dd4b9f6b4d7a1ae224de455f28de75dd)

### 📦️ Dependency Updates

- update stefanzweifel/git-auto-commit-action action to v5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#25](https://github.com/jdx/usage/pull/25)

## [0.1.7](https://github.com/jdx/usage/compare/v0.1.6..v0.1.7) - 2024-02-10

### 🐛 Bug Fixes

- fix apple urls for binstall by [@jdx](https://github.com/jdx) in [06261f0](https://github.com/jdx/usage/commit/06261f0174bc0a95f216a9b22f85b0955f8c4a26)

## [0.1.6] - 2024-02-10

### 🔍 Other Changes

- add config for cargo-binstall by [@jdx](https://github.com/jdx) in [9711365](https://github.com/jdx/usage/commit/9711365fbfe1b39df03597af93caf9ca1b0e1b62)

### 📦️ Dependency Updates

- update actions/checkout action to v4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#23](https://github.com/jdx/usage/pull/23)

<!-- generated by git-cliff -->
