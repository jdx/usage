# Changelog

## [2.13.0](https://github.com/jdx/usage/compare/v2.12.0..v2.13.0) - 2026-01-19

### 🚀 Features

- **(release)** add LLM-generated prose summary to release notes by [@jdx](https://github.com/jdx) in [#421](https://github.com/jdx/usage/pull/421)
- add LLM-generated release notes for GitHub releases by [@jdx](https://github.com/jdx) in [#423](https://github.com/jdx/usage/pull/423)
- add spec lint command by [@jdx](https://github.com/jdx) in [#430](https://github.com/jdx/usage/pull/430)

### 🐛 Bug Fixes

- replace unsafe path unwrap chains with proper error handling by [@jdx](https://github.com/jdx) in [#424](https://github.com/jdx/usage/pull/424)
- pass positional args through to executed scripts by [@jdx](https://github.com/jdx) in [#425](https://github.com/jdx/usage/pull/425)
- replace unimplemented!() with proper errors for unsupported shells by [@jdx](https://github.com/jdx) in [#432](https://github.com/jdx/usage/pull/432)
- update claude CLI model and add bypassPermissions by [@jdx](https://github.com/jdx) in [#435](https://github.com/jdx/usage/pull/435)

### 🚜 Refactor

- remove unused double-shebang support by [@jdx](https://github.com/jdx) in [#426](https://github.com/jdx/usage/pull/426)
- replace once_cell with std::sync::LazyLock by [@jdx](https://github.com/jdx) in [#428](https://github.com/jdx/usage/pull/428)
- improve code quality with safety and lint fixes by [@jdx](https://github.com/jdx) in [#427](https://github.com/jdx/usage/pull/427)

### ⚡ Performance

- use Arc for flag/arg keys in ParseOutput to reduce cloning by [@jdx](https://github.com/jdx) in [#422](https://github.com/jdx/usage/pull/422)

### 🔍 Other Changes

- update insta snapshots to newer format by [@jdx](https://github.com/jdx) in [#429](https://github.com/jdx/usage/pull/429)
- fix legacy inline snapshot format warnings by [@jdx](https://github.com/jdx) in [#433](https://github.com/jdx/usage/pull/433)
- replace TODO with doc comment for subcommand_lookup by [@jdx](https://github.com/jdx) in [#434](https://github.com/jdx/usage/pull/434)

### 📦️ Dependency Updates

- update actions/setup-node digest to 6044e13 by [@renovate[bot]](https://github.com/renovate[bot]) in [#419](https://github.com/jdx/usage/pull/419)
- replace dependency @tsconfig/node22 with @tsconfig/node24 by [@renovate[bot]](https://github.com/renovate[bot]) in [#418](https://github.com/jdx/usage/pull/418)

## [2.12.0](https://github.com/jdx/usage/compare/v2.11.0..v2.12.0) - 2026-01-14

### 🚀 Features

- Allowing preserving double dashes for variadic args by [@alcroito](https://github.com/alcroito) in [#417](https://github.com/jdx/usage/pull/417)

### 📦️ Dependency Updates

- replace dependency @tsconfig/node18 with @tsconfig/node20 by [@renovate[bot]](https://github.com/renovate[bot]) in [#411](https://github.com/jdx/usage/pull/411)
- update amannn/action-semantic-pull-request action to v6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#412](https://github.com/jdx/usage/pull/412)
- replace dependency @tsconfig/node20 with @tsconfig/node22 by [@renovate[bot]](https://github.com/renovate[bot]) in [#415](https://github.com/jdx/usage/pull/415)
- update rust crate clap to v4.5.54 by [@renovate[bot]](https://github.com/renovate[bot]) in [#416](https://github.com/jdx/usage/pull/416)

### New Contributors

- @alcroito made their first contribution in [#417](https://github.com/jdx/usage/pull/417)

## [2.11.0](https://github.com/jdx/usage/compare/v2.10.0..v2.11.0) - 2025-12-31

### 🚀 Features

- add default_subcommand and restart_token for naked task completions by [@jdx](https://github.com/jdx) in [#410](https://github.com/jdx/usage/pull/410)

### 🐛 Bug Fixes

- handle --help flag in exec command for non-shell scripts by [@jdx](https://github.com/jdx) in [#409](https://github.com/jdx/usage/pull/409)

### 🧪 Testing

- add non-shell script tests by [@muzimuzhi](https://github.com/muzimuzhi) in [#406](https://github.com/jdx/usage/pull/406)

### 📦️ Dependency Updates

- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#403](https://github.com/jdx/usage/pull/403)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#407](https://github.com/jdx/usage/pull/407)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#408](https://github.com/jdx/usage/pull/408)

## [2.10.0](https://github.com/jdx/usage/compare/v2.9.0..v2.10.0) - 2025-12-19

### 🚀 Features

- add variadic argument improvements and builder API by [@jdx](https://github.com/jdx) in [#401](https://github.com/jdx/usage/pull/401)

### 🐛 Bug Fixes

- unhide exec command and fix docs shebang for non-shell scripts by [@jdx](https://github.com/jdx) in [#402](https://github.com/jdx/usage/pull/402)

### 📦️ Dependency Updates

- update actions/checkout digest to 34e1148 by [@renovate[bot]](https://github.com/renovate[bot]) in [#389](https://github.com/jdx/usage/pull/389)
- update swatinem/rust-cache digest to 779680d by [@renovate[bot]](https://github.com/renovate[bot]) in [#390](https://github.com/jdx/usage/pull/390)
- update rust crate ctor to v0.6.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#392](https://github.com/jdx/usage/pull/392)
- update actions/checkout action to v6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#393](https://github.com/jdx/usage/pull/393)
- update actions/setup-node action to v6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#394](https://github.com/jdx/usage/pull/394)
- update actions/upload-pages-artifact action to v4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#395](https://github.com/jdx/usage/pull/395)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#396](https://github.com/jdx/usage/pull/396)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#397](https://github.com/jdx/usage/pull/397)
- update codecov/codecov-action digest to 671740a by [@renovate[bot]](https://github.com/renovate[bot]) in [#398](https://github.com/jdx/usage/pull/398)
- update rust crate shell-words to v1.1.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#399](https://github.com/jdx/usage/pull/399)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#400](https://github.com/jdx/usage/pull/400)

## [2.9.0](https://github.com/jdx/usage/compare/v2.8.0..v2.9.0) - 2025-12-03

### 🚀 Features

- Support `Vec<String>` for default values of variadic flags by [@iamkroot](https://github.com/iamkroot) in [#388](https://github.com/jdx/usage/pull/388)

### 🐛 Bug Fixes

- treat count flags as repeatable by [@frederikb](https://github.com/frederikb) in [#383](https://github.com/jdx/usage/pull/383)

### 📦️ Dependency Updates

- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#385](https://github.com/jdx/usage/pull/385)

### New Contributors

- @frederikb made their first contribution in [#383](https://github.com/jdx/usage/pull/383)

## [2.8.0](https://github.com/jdx/usage/compare/v2.7.0..v2.8.0) - 2025-11-12

### 🚀 Features

- add examples section to markdown and manpage generation by [@jdx](https://github.com/jdx) in [#380](https://github.com/jdx/usage/pull/380)
- add examples support to spec-level by [@jdx](https://github.com/jdx) in [#382](https://github.com/jdx/usage/pull/382)

### 🐛 Bug Fixes

- allow blank comment lines in #USAGE blocks by [@jdx](https://github.com/jdx) in [#381](https://github.com/jdx/usage/pull/381)

## [2.7.0](https://github.com/jdx/usage/compare/v2.6.0..v2.7.0) - 2025-11-11

### 🚀 Features

- support bracketed header syntax by [@okuuva](https://github.com/okuuva) in [#377](https://github.com/jdx/usage/pull/377)

### 📚 Documentation

- Fix VitePress build error in markdown documentation by [@jdx](https://github.com/jdx) in [#378](https://github.com/jdx/usage/pull/378)

### 🔍 Other Changes

- integrate clap-sort to enforce alphabetical subcommand ordering by [@jdx](https://github.com/jdx) in [#370](https://github.com/jdx/usage/pull/370)

### 📦️ Dependency Updates

- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#372](https://github.com/jdx/usage/pull/372)
- update rust crate clap-sort to v1.0.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#373](https://github.com/jdx/usage/pull/373)
- update rust crate ctor to v0.6.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#374](https://github.com/jdx/usage/pull/374)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#375](https://github.com/jdx/usage/pull/375)

### New Contributors

- @okuuva made their first contribution in [#377](https://github.com/jdx/usage/pull/377)

## [2.6.0](https://github.com/jdx/usage/compare/v2.5.2..v2.6.0) - 2025-11-02

### 🚀 Features

- implement inline help layout with automatic text wrapping by [@jdx](https://github.com/jdx) in [#365](https://github.com/jdx/usage/pull/365)
- add manpage generation support by [@jdx](https://github.com/jdx) in [#369](https://github.com/jdx/usage/pull/369)

### 🐛 Bug Fixes

- resolve clippy warnings in test files by [@jdx](https://github.com/jdx) in [#367](https://github.com/jdx/usage/pull/367)
- prevent empty help_rendered from causing inline-empty layout by [@jdx](https://github.com/jdx) in [#368](https://github.com/jdx/usage/pull/368)

## [2.5.2](https://github.com/jdx/usage/compare/v2.5.1..v2.5.2) - 2025-10-31

### 🐛 Bug Fixes

- avoid using exec crate on windows by [@gaojunran](https://github.com/gaojunran) in [#363](https://github.com/jdx/usage/pull/363)
- support boolean literals for flag defaults by [@jdx](https://github.com/jdx) in [#364](https://github.com/jdx/usage/pull/364)

### 📦️ Dependency Updates

- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#359](https://github.com/jdx/usage/pull/359)
- update rust crate clap to v4.5.51 by [@renovate[bot]](https://github.com/renovate[bot]) in [#361](https://github.com/jdx/usage/pull/361)
- update rust crate tera to v1.20.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#362](https://github.com/jdx/usage/pull/362)

### New Contributors

- @gaojunran made their first contribution in [#363](https://github.com/jdx/usage/pull/363)

## [2.5.1](https://github.com/jdx/usage/compare/v2.5.0..v2.5.1) - 2025-10-26

### 🐛 Bug Fixes

- pass global flags to mount commands during completion by [@jdx](https://github.com/jdx) in [#354](https://github.com/jdx/usage/pull/354)

### 🧪 Testing

- add comprehensive test for default="" behavior by [@jdx](https://github.com/jdx) in [#357](https://github.com/jdx/usage/pull/357)

### 🔍 Other Changes

- disable homebrew formula by [@jdx](https://github.com/jdx) in [#355](https://github.com/jdx/usage/pull/355)

## [2.5.0](https://github.com/jdx/usage/compare/v2.4.0..v2.5.0) - 2025-10-25

### 🚀 Features

- Print default values if specified by [@iamkroot](https://github.com/iamkroot) in [#350](https://github.com/jdx/usage/pull/350)

### 🐛 Bug Fixes

- add fallback for shell by [@MeanderingProgrammer](https://github.com/MeanderingProgrammer) in [#347](https://github.com/jdx/usage/pull/347)
- complete descriptions serialized as string instead of bool by [@iamkroot](https://github.com/iamkroot) in [#349](https://github.com/jdx/usage/pull/349)

### 🔍 Other Changes

- mise up by [@jdx](https://github.com/jdx) in [#353](https://github.com/jdx/usage/pull/353)

### 📦️ Dependency Updates

- update rust crate serde_with to v3.15.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#351](https://github.com/jdx/usage/pull/351)
- update rust crate ctor to 0.6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#352](https://github.com/jdx/usage/pull/352)

### New Contributors

- @iamkroot made their first contribution in [#350](https://github.com/jdx/usage/pull/350)

## [2.4.0](https://github.com/jdx/usage/compare/v2.3.2..v2.4.0) - 2025-10-21

### 🚀 Features

- add env attribute support for flags by [@jdx](https://github.com/jdx) in [#336](https://github.com/jdx/usage/pull/336)
- add env attribute support for args by [@jdx](https://github.com/jdx) in [#346](https://github.com/jdx/usage/pull/346)

### 🐛 Bug Fixes

- handle colons in zsh completions without description by [@MeanderingProgrammer](https://github.com/MeanderingProgrammer) in [#341](https://github.com/jdx/usage/pull/341)

### 📦️ Dependency Updates

- update pnpm/action-setup digest to 41ff726 by [@renovate[bot]](https://github.com/renovate[bot]) in [#339](https://github.com/jdx/usage/pull/339)
- update dependency semver to v7.7.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#340](https://github.com/jdx/usage/pull/340)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#342](https://github.com/jdx/usage/pull/342)
- update rust crate clap to v4.5.49 by [@renovate[bot]](https://github.com/renovate[bot]) in [#343](https://github.com/jdx/usage/pull/343)
- update rust crate regex to v1.12.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#344](https://github.com/jdx/usage/pull/344)
- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#345](https://github.com/jdx/usage/pull/345)

### New Contributors

- @MeanderingProgrammer made their first contribution in [#341](https://github.com/jdx/usage/pull/341)

## [2.3.2](https://github.com/jdx/usage/compare/v2.3.1..v2.3.2) - 2025-09-29

### 🐛 Bug Fixes

- **(zsh)** compdef ordering by [@jdx](https://github.com/jdx) in [#335](https://github.com/jdx/usage/pull/335)

### 📦️ Dependency Updates

- lock file maintenance by [@renovate[bot]](https://github.com/renovate[bot]) in [#332](https://github.com/jdx/usage/pull/332)

## [2.3.1](https://github.com/jdx/usage/compare/v2.3.0..v2.3.1) - 2025-09-28

### 🐛 Bug Fixes

- issues with very large specs by [@jdx](https://github.com/jdx) in [#330](https://github.com/jdx/usage/pull/330)

## [2.3.0](https://github.com/jdx/usage/compare/v2.2.2..v2.3.0) - 2025-09-28

### 🚀 Features

- add @generated comments to all generators by [@jdx](https://github.com/jdx) in [#310](https://github.com/jdx/usage/pull/310)

### 🐛 Bug Fixes

- **(brew)** bump formula after the release by [@muzimuzhi](https://github.com/muzimuzhi) in [#305](https://github.com/jdx/usage/pull/305)
- **(completions)** ignore aliases and functions named usage (2nd attempt) by [@risu729](https://github.com/risu729) in [#304](https://github.com/jdx/usage/pull/304)
- use temp files to avoid 'argument list too long' error in shell completions by [@jdx](https://github.com/jdx) in [#329](https://github.com/jdx/usage/pull/329)

### 🔍 Other Changes

- ignore renovate new bot name by [@risu729](https://github.com/risu729) in [#324](https://github.com/jdx/usage/pull/324)

### 📦️ Dependency Updates

- pin dependencies by [@renovate[bot]](https://github.com/renovate[bot]) in [#307](https://github.com/jdx/usage/pull/307)
- update rust crate serde_json to v1.0.141 by [@renovate[bot]](https://github.com/renovate[bot]) in [#309](https://github.com/jdx/usage/pull/309)
- update jdx/mise-action digest to 13abe50 by [@renovate[bot]](https://github.com/renovate[bot]) in [#308](https://github.com/jdx/usage/pull/308)
- update actions/checkout digest to 08eba0b by [@renovate[bot]](https://github.com/renovate[bot]) in [#316](https://github.com/jdx/usage/pull/316)
- update amannn/action-semantic-pull-request digest to e32d7e6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#317](https://github.com/jdx/usage/pull/317)
- update jdx/mise-action digest to c37c932 by [@renovate[bot]](https://github.com/renovate[bot]) in [#312](https://github.com/jdx/usage/pull/312)
- update apple-actions/import-codesign-certs digest to 95e84a1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#318](https://github.com/jdx/usage/pull/318)
- update dependency vitepress to v1.6.4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#319](https://github.com/jdx/usage/pull/319)
- update codecov/codecov-action digest to 5a10915 by [@renovate[bot]](https://github.com/renovate[bot]) in [#326](https://github.com/jdx/usage/pull/326)
- update rust crate ctor to 0.5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#327](https://github.com/jdx/usage/pull/327)

### New Contributors

- @muzimuzhi made their first contribution in [#305](https://github.com/jdx/usage/pull/305)

## [2.2.2](https://github.com/jdx/usage/compare/v2.2.1..v2.2.2) - 2025-07-16

### 📚 Documentation

- fix revert for git-cliff by [@jdx](https://github.com/jdx) in [#302](https://github.com/jdx/usage/pull/302)

### ◀️ Revert

- Revert "fix(completions): ignore aliases and functions named usage" by [@jdx](https://github.com/jdx) in [#301](https://github.com/jdx/usage/pull/301)

## [2.2.1](https://github.com/jdx/usage/compare/v2.2.0..v2.2.1) - 2025-07-16

### 🐛 Bug Fixes

- **(completions)** ignore aliases and functions named usage by [@risu729](https://github.com/risu729) in [#300](https://github.com/jdx/usage/pull/300)

### 🔍 Other Changes

- refactor gh release creation to avoid rate limit errors by [@jdx](https://github.com/jdx) in [#299](https://github.com/jdx/usage/pull/299)

### 📦️ Dependency Updates

- update dawidd6/action-homebrew-bump-formula action to v5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#294](https://github.com/jdx/usage/pull/294)

## [2.2.0](https://github.com/jdx/usage/compare/v2.1.1..v2.2.0) - 2025-07-11

### 🚀 Features

- Generalize bash command to support bash/zsh/fish by [@NorthIsUp](https://github.com/NorthIsUp) in [#280](https://github.com/jdx/usage/pull/280)

### 🐛 Bug Fixes

- update wrong name package manager by [@axemanofic](https://github.com/axemanofic) in [#287](https://github.com/jdx/usage/pull/287)
- fall back to listing files on unknown completions by [@jdx](https://github.com/jdx) in [#296](https://github.com/jdx/usage/pull/296)

### 📚 Documentation

- complete templates by [@syhol](https://github.com/syhol) in [#286](https://github.com/jdx/usage/pull/286)
- fix bad whitespace character by [@syhol](https://github.com/syhol) in [#288](https://github.com/jdx/usage/pull/288)

### 🔍 Other Changes

- add semantic-pr-lint by [@jdx](https://github.com/jdx) in [#281](https://github.com/jdx/usage/pull/281)
- clippy by [@jdx](https://github.com/jdx) in [f6d5e38](https://github.com/jdx/usage/commit/f6d5e381d902574ad2a9ebf8366bcdfa17098593)

### 📦️ Dependency Updates

- update pnpm/action-setup action to v4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#278](https://github.com/jdx/usage/pull/278)
- update dependency semver to v7.7.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#283](https://github.com/jdx/usage/pull/283)
- update autofix-ci/action action to v1.3.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#289](https://github.com/jdx/usage/pull/289)

### New Contributors

- @syhol made their first contribution in [#288](https://github.com/jdx/usage/pull/288)
- @axemanofic made their first contribution in [#287](https://github.com/jdx/usage/pull/287)
- @NorthIsUp made their first contribution in [#280](https://github.com/jdx/usage/pull/280)

## [2.1.1](https://github.com/jdx/usage/compare/v2.1.0..v2.1.1) - 2025-04-26

### 🔍 Other Changes

- dry run releases by [@jdx](https://github.com/jdx) in [67cd3d6](https://github.com/jdx/usage/commit/67cd3d615b60ea7c3a0f0e2d63e0932b99c7b62a)
- fix releases by [@jdx](https://github.com/jdx) in [#272](https://github.com/jdx/usage/pull/272)

## [2.1.0](https://github.com/jdx/usage/compare/v2.0.7..v2.1.0) - 2025-04-26

### 🚀 Features

- use ellipsis character by [@jdx](https://github.com/jdx) in [#269](https://github.com/jdx/usage/pull/269)

### 🔍 Other Changes

- upgrade ubuntu by [@jdx](https://github.com/jdx) in [3f71633](https://github.com/jdx/usage/commit/3f71633bd7be4c337e3584bed20d35c7355cb5e7)

### 📦️ Dependency Updates

- update apple-actions/import-codesign-certs action to v5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#262](https://github.com/jdx/usage/pull/262)

## [2.0.7](https://github.com/jdx/usage/compare/v2.0.6..v2.0.7) - 2025-03-24

### 🐛 Bug Fixes

- implement short flag chaining and update flag handling logic by [@aroemen](https://github.com/aroemen) in [#258](https://github.com/jdx/usage/pull/258)

### 🔍 Other Changes

- Fix some typos in completions.md by [@torarvid](https://github.com/torarvid) in [#253](https://github.com/jdx/usage/pull/253)
- updated deps by [@jdx](https://github.com/jdx) in [7a498e6](https://github.com/jdx/usage/commit/7a498e60e90420af8bec0e97ddbc9f69fdbcd8d5)

### 📦️ Dependency Updates

- update apple-actions/import-codesign-certs action to v4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#256](https://github.com/jdx/usage/pull/256)
- update dependency vitepress to v1.6.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#255](https://github.com/jdx/usage/pull/255)

### New Contributors

- @aroemen made their first contribution in [#258](https://github.com/jdx/usage/pull/258)
- @torarvid made their first contribution in [#253](https://github.com/jdx/usage/pull/253)

## [2.0.6](https://github.com/jdx/usage/compare/v2.0.5..v2.0.6) - 2025-03-18

### 🐛 Bug Fixes

- **(lib)** make ParseValue cloneable by [@risu729](https://github.com/risu729) in [#252](https://github.com/jdx/usage/pull/252)

### 📚 Documentation

- add arch instructions by [@jdx](https://github.com/jdx) in [b8f8387](https://github.com/jdx/usage/commit/b8f83872ae342c6a9e8ab82287cb545b58aebcfa)

### 🔍 Other Changes

- renovate skip autofix by [@jdx](https://github.com/jdx) in [#238](https://github.com/jdx/usage/pull/238)
- remove aur by [@jdx](https://github.com/jdx) in [2b711d8](https://github.com/jdx/usage/commit/2b711d8bdfd8c297b0e43ec4cb5289051bb1a144)
- added workflow_dispatch to release-plz by [@jdx](https://github.com/jdx) in [cef737c](https://github.com/jdx/usage/commit/cef737c6f42bf981d19b8b26c757bcfd83bc247e)

### New Contributors

- @risu729 made their first contribution in [#252](https://github.com/jdx/usage/pull/252)

## [2.0.5](https://github.com/jdx/usage/compare/v2.0.4..v2.0.5) - 2025-02-16

### 🐛 Bug Fixes

- 2 bugs with flags and var=#true by [@jdx](https://github.com/jdx) in [#235](https://github.com/jdx/usage/pull/235)

### 🔍 Other Changes

- added coverage workflow by [@jdx](https://github.com/jdx) in [#237](https://github.com/jdx/usage/pull/237)

## [2.0.4](https://github.com/jdx/usage/compare/v2.0.3..v2.0.4) - 2025-02-02

### 🔍 Other Changes

- bump clap_usage by [@jdx](https://github.com/jdx) in [d896d24](https://github.com/jdx/usage/commit/d896d24911892969b36e51e03465d442fa040652)
- use ubuntu-20.04 for publishing by [@jdx](https://github.com/jdx) in [d1d6989](https://github.com/jdx/usage/commit/d1d6989814ebbc24aafd59621ea3cb5d63fe8a2e)
- bump npm packages on releases by [@jdx](https://github.com/jdx) in [c24e25d](https://github.com/jdx/usage/commit/c24e25d92fd43e6e79703cdffcf2394d941cd109)
- added pnpm by [@jdx](https://github.com/jdx) in [4c517fb](https://github.com/jdx/usage/commit/4c517fbdff91849e2a9525432833f5e58d0d7385)

## [2.0.3](https://github.com/jdx/usage/compare/v2.0.0..v2.0.3) - 2025-01-10

### 🐛 Bug Fixes

- add v1-fallback for kdl by [@jdx](https://github.com/jdx) in [9516e15](https://github.com/jdx/usage/commit/9516e15d53c0769a1227ec4ab37e0622b4e7bead)

### ◀️ Revert

- Revert "fix: add v1-fallback for kdl" by [@jdx](https://github.com/jdx) in [ef98628](https://github.com/jdx/usage/commit/ef98628658cb3adcc3284aa341b70329743fa3da)
- Revert "chore: attempt to fix kdl v1-fallback" by [@jdx](https://github.com/jdx) in [c440c2a](https://github.com/jdx/usage/commit/c440c2a4fb843da0670b72f0b6c233602d7c9066)

### 🔍 Other Changes

- fix publish script by [@jdx](https://github.com/jdx) in [7c72bc3](https://github.com/jdx/usage/commit/7c72bc3450fe0e731331be18354244b5a19223d5)
- configure render:fig task by [@jdx](https://github.com/jdx) in [f744199](https://github.com/jdx/usage/commit/f744199b53de9272cf62ab6c760c9da1239fa626)
- fix fig syntax rendering by [@jdx](https://github.com/jdx) in [2b2d301](https://github.com/jdx/usage/commit/2b2d30104280c64854b78d829547a5d3fa8694df)
- attempt to fix kdl v1-fallback by [@jdx](https://github.com/jdx) in [8c0a2c6](https://github.com/jdx/usage/commit/8c0a2c698e51f382888dfa2bc170bb9035df1173)
- bump by [@jdx](https://github.com/jdx) in [bdc1dfb](https://github.com/jdx/usage/commit/bdc1dfb2c6f12466cad102f1a7b06f30b32ef05e)
- bump by [@jdx](https://github.com/jdx) in [6a468df](https://github.com/jdx/usage/commit/6a468df654ce2e7a9fad1de52a279be74268fbbf)

## [2.0.0](https://github.com/jdx/usage/compare/v1.7.4..v2.0.0) - 2025-01-10

### 🚀 Features

- **breaking** kdl 2.0 by [@jdx](https://github.com/jdx) in [#218](https://github.com/jdx/usage/pull/218)

### 🐛 Bug Fixes

- **(fish)** remove deprecated completion option by [@jdx](https://github.com/jdx) in [#217](https://github.com/jdx/usage/pull/217)
- make compatible with ancient bash by [@jdx](https://github.com/jdx) in [9e76a17](https://github.com/jdx/usage/commit/9e76a17e433fde50d15c3250aef693f378c17efc)

### 📚 Documentation

- add source_code_link_template example by [@jdx](https://github.com/jdx) in [cb1f7b4](https://github.com/jdx/usage/commit/cb1f7b4b0bacd66b0928b291d3e58fc3c93d18a3)

### 🔍 Other Changes

- Update LICENSE by [@jdx](https://github.com/jdx) in [e27b7d9](https://github.com/jdx/usage/commit/e27b7d9cbbbe096a844dde9ace93bfebd35e2e63)
- remove unused custom homebrew tap by [@jdx](https://github.com/jdx) in [d5d734f](https://github.com/jdx/usage/commit/d5d734f970605bb8090ac216887650516f1edd4c)
- upgraded itertools by [@jdx](https://github.com/jdx) in [b3cb03a](https://github.com/jdx/usage/commit/b3cb03a5319e22672ff1e87500b861f7af47b157)
- fix git-cliff config by [@jdx](https://github.com/jdx) in [9f293ae](https://github.com/jdx/usage/commit/9f293ae19541bb2c3ba927523e008e88e8fbdfab)

### 📦️ Dependency Updates

- update dependency @withfig/autocomplete to v2.690.2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#214](https://github.com/jdx/usage/pull/214)

## [1.7.4](https://github.com/jdx/usage/compare/v1.7.3..v1.7.4) - 2024-12-21

### 🔍 Other Changes

- expose spec.merge method by [@jdx](https://github.com/jdx) in [6de998c](https://github.com/jdx/usage/commit/6de998c00ec15b5bca70bbd46cb5700d9e620861)

## [1.7.3](https://github.com/jdx/usage/compare/v1.7.2..v1.7.3) - 2024-12-21

### 🔍 Other Changes

- Better fig generation to avoid linter from complaining by [@miguelmig](https://github.com/miguelmig) in [#208](https://github.com/jdx/usage/pull/208)

## [1.7.2](https://github.com/jdx/usage/compare/v1.7.1..v1.7.2) - 2024-12-18

### 🐛 Bug Fixes

- clean up double_dash rendering by [@jdx](https://github.com/jdx) in [eac7db8](https://github.com/jdx/usage/commit/eac7db8a68ded04f6c2260fe68a5bba2867a3a5d)

## [1.7.1](https://github.com/jdx/usage/compare/v1.7.0..v1.7.1) - 2024-12-18

### 🐛 Bug Fixes

- completions with descriptions splitting by [@jdx](https://github.com/jdx) in [5e72f3b](https://github.com/jdx/usage/commit/5e72f3bcda74b3b05a0b3362cfc7a39a15c53146)
- snake_case double_dash options by [@jdx](https://github.com/jdx) in [92d4dcc](https://github.com/jdx/usage/commit/92d4dccdfa922df5b030eaf6ed8197c9075ff1b2)

### 🧪 Testing

- added test case for completer with description by [@jdx](https://github.com/jdx) in [441bfa9](https://github.com/jdx/usage/commit/441bfa9b30c0026252202784f3aad1ce9bd7baf0)

## [1.7.0](https://github.com/jdx/usage/compare/v1.6.0..v1.7.0) - 2024-12-18

### 🚀 Features

- added double_dash option to args by [@jdx](https://github.com/jdx) in [#202](https://github.com/jdx/usage/pull/202)

### 🐛 Bug Fixes

- allow overriding `usage` in case of conflict by [@jdx](https://github.com/jdx) in [#198](https://github.com/jdx/usage/pull/198)
- join code fences if they are right next to each other by [@jdx](https://github.com/jdx) in [#200](https://github.com/jdx/usage/pull/200)
- default cmd help types by [@jdx](https://github.com/jdx) in [#203](https://github.com/jdx/usage/pull/203)
- make --include-bash-completion-lib work by [@jdx](https://github.com/jdx) in [c833bb4](https://github.com/jdx/usage/commit/c833bb4493d55dd23278ded7f3a1769e8aa448e5)

### 🔍 Other Changes

- disable cargo up in release-plz by [@jdx](https://github.com/jdx) in [16a36b7](https://github.com/jdx/usage/commit/16a36b78673b40a98a4701e30d5222f4a1b4bb95)
- pin kdl-rs by [@jdx](https://github.com/jdx) in [7feeb24](https://github.com/jdx/usage/commit/7feeb2403d8055232e3c7a828c8ffe56052d2063)

## [1.6.0](https://github.com/jdx/usage/compare/v1.5.3..v1.6.0) - 2024-12-14

### 🚀 Features

- feature for automatically adding code fences by [@jdx](https://github.com/jdx) in [#197](https://github.com/jdx/usage/pull/197)

### 🐛 Bug Fixes

- make bash_completion optional by [@jdx](https://github.com/jdx) in [6705de4](https://github.com/jdx/usage/commit/6705de473fbd2207be2f933c051a48188029b069)

## [1.5.3](https://github.com/jdx/usage/compare/v1.5.2..v1.5.3) - 2024-12-13

### 🐛 Bug Fixes

- bash completion escape by [@jdx](https://github.com/jdx) in [ce80f20](https://github.com/jdx/usage/commit/ce80f207b609f251515ba0889844cd694ed6f820)

### 🧪 Testing

- snapshots by [@jdx](https://github.com/jdx) in [d15bd90](https://github.com/jdx/usage/commit/d15bd90af4d67440219182c287959013ca56b8d3)

### 🔍 Other Changes

- add snapshots to pre-commit by [@jdx](https://github.com/jdx) in [9d19066](https://github.com/jdx/usage/commit/9d1906603bd0ea505928a4423b78bb4edd744b18)

## [1.5.2](https://github.com/jdx/usage/compare/v1.5.1..v1.5.2) - 2024-12-12

### 🐛 Bug Fixes

- remove debug @usage by [@jdx](https://github.com/jdx) in [8178c97](https://github.com/jdx/usage/commit/8178c97f3004bcacb4827083c5cb46fa23bff64e)

## [1.5.1](https://github.com/jdx/usage/compare/v1.5.0..v1.5.1) - 2024-12-12

### 🔍 Other Changes

- remove submodule by [@jdx](https://github.com/jdx) in [5922490](https://github.com/jdx/usage/commit/5922490244dd43f7e7852aa5be8eef3c549671de)

## [1.5.0](https://github.com/jdx/usage/compare/v1.4.2..v1.5.0) - 2024-12-12

### 🚀 Features

- descriptions in completions by [@jdx](https://github.com/jdx) in [ef73a40](https://github.com/jdx/usage/commit/ef73a40be990a611df13bb9f662fb5d1e1538651)

## [1.4.2](https://github.com/jdx/usage/compare/v1.4.1..v1.4.2) - 2024-12-12

### 🐛 Bug Fixes

- handle colons in bash completions by [@jdx](https://github.com/jdx) in [240ea41](https://github.com/jdx/usage/commit/240ea418e6bcadfacca70a14670cd10de1086cbe)
- handle colons in zsh completions by [@jdx](https://github.com/jdx) in [455b6f7](https://github.com/jdx/usage/commit/455b6f7435d07c6a9a2c20d82584da96c5ae5933)

### 🧪 Testing

- snapshots by [@jdx](https://github.com/jdx) in [4ab650f](https://github.com/jdx/usage/commit/4ab650f1e4b6bf35491f538f99d42a121702f173)

### 🔍 Other Changes

- add bash-completions to lib by [@jdx](https://github.com/jdx) in [8450ff7](https://github.com/jdx/usage/commit/8450ff7c15149d926a948c6f291b2d727bb607ce)
- submodules by [@jdx](https://github.com/jdx) in [83d68a9](https://github.com/jdx/usage/commit/83d68a9976e778e3e98744f850b07be82a42e49a)
- submodules by [@jdx](https://github.com/jdx) in [a4f5251](https://github.com/jdx/usage/commit/a4f52519c36972b962e4a9aaf973d72acdfdd100)
- ignore bash-completion in prettier by [@jdx](https://github.com/jdx) in [4b58310](https://github.com/jdx/usage/commit/4b5831095041916aa1e220549efaee425a1ab928)

## [1.4.1](https://github.com/jdx/usage/compare/v1.4.0..v1.4.1) - 2024-12-10

### 🐛 Bug Fixes

- bug when "about" is empty by [@jdx](https://github.com/jdx) in [1db423b](https://github.com/jdx/usage/commit/1db423b356510ab03023ed6348ca783b1a02a31e)
- join var=true args with shell_words::join by [@jdx](https://github.com/jdx) in [#190](https://github.com/jdx/usage/pull/190)

## [1.4.0](https://github.com/jdx/usage/compare/v1.3.5..v1.4.0) - 2024-12-09

### 🚀 Features

- `usage g json` by [@jdx](https://github.com/jdx) in [#184](https://github.com/jdx/usage/pull/184)

### 🐛 Bug Fixes

- bug with completing default args/flags by [@jdx](https://github.com/jdx) in [#185](https://github.com/jdx/usage/pull/185)
- added completes to string output by [@jdx](https://github.com/jdx) in [#186](https://github.com/jdx/usage/pull/186)
- added completes to string output by [@jdx](https://github.com/jdx) in [#187](https://github.com/jdx/usage/pull/187)
- added completes to cmds by [@jdx](https://github.com/jdx) in [f421d9e](https://github.com/jdx/usage/commit/f421d9e5b8a88eae70914ff0be44bee824dc0aa1)

### 📚 Documentation

- fix links by [@jdx](https://github.com/jdx) in [46be80a](https://github.com/jdx/usage/commit/46be80a48d2174167546ecbf3b3e3cf32487d4b8)
- fix links by [@jdx](https://github.com/jdx) in [8a4327b](https://github.com/jdx/usage/commit/8a4327bafc2c644867e0c01d3e4902a7e8ee20f4)

### 🧪 Testing

- set GITHUB_TOKEN by [@jdx](https://github.com/jdx) in [f43fa85](https://github.com/jdx/usage/commit/f43fa85280bf63211f4f6453e4bfc2e97e9d7c3b)

### 🔍 Other Changes

- Update markdown.md by [@jdx](https://github.com/jdx) in [a0f32d5](https://github.com/jdx/usage/commit/a0f32d5a664e21b4603402488606a52c320173a4)
- lint-fix by [@jdx](https://github.com/jdx) in [a825d43](https://github.com/jdx/usage/commit/a825d43ec2d73339655645224f76f153f7484548)
- fix release-plz by [@jdx](https://github.com/jdx) in [1586ede](https://github.com/jdx/usage/commit/1586ede484681162d2a85c41627835d9e95fcd89)
- fix release-plz by [@jdx](https://github.com/jdx) in [650f5fb](https://github.com/jdx/usage/commit/650f5fb980c1b73c28a4eb23f5a65a0eb47fd58e)

## [1.3.5](https://github.com/jdx/usage/compare/v1.3.4..v1.3.5) - 2024-12-09

### 🔍 Other Changes

- Update README.md by [@jdx](https://github.com/jdx) in [3fc2181](https://github.com/jdx/usage/commit/3fc218107b0f911169a58f2c8dba3fba7e6bcdc3)
- bump to miette-7 by [@jdx](https://github.com/jdx) in [#21](https://github.com/jdx/usage/pull/21)

## [1.3.4](https://github.com/jdx/usage/compare/v1.3.3..v1.3.4) - 2024-12-03

### 🔍 Other Changes

- added shellcheck for bash completion file by [@jdx](https://github.com/jdx) in [#176](https://github.com/jdx/usage/pull/176)
- skip autofix on renovate prs by [@jdx](https://github.com/jdx) in [ada6c92](https://github.com/jdx/usage/commit/ada6c92da40d54d3afcba9a6366213a22f215272)
- pin kdl below 4.7 by [@jdx](https://github.com/jdx) in [045c9cf](https://github.com/jdx/usage/commit/045c9cf7edc6b9764fd9a794afbbde5b21ddba76)

## [1.3.3](https://github.com/jdx/usage/compare/v1.3.2..v1.3.3) - 2024-11-22

### 🐛 Bug Fixes

- unset arg/flag required if default provided by [@jdx](https://github.com/jdx) in [#175](https://github.com/jdx/usage/pull/175)

### 🔍 Other Changes

- added shellcheck disable comment for bash completion by [@jdx](https://github.com/jdx) in [7e1da8f](https://github.com/jdx/usage/commit/7e1da8fabc78d94f752c59b09bb83e4b18ec0bfe)

## [1.3.2](https://github.com/jdx/usage/compare/v1.3.1..v1.3.2) - 2024-11-16

### 🐛 Bug Fixes

- space-separate multi-args by [@jdx](https://github.com/jdx) in [4054034](https://github.com/jdx/usage/commit/4054034bb12414fd179c17a105855e86544d497a)

## [1.3.1](https://github.com/jdx/usage/compare/v1.3.0..v1.3.1) - 2024-11-14

### 🐛 Bug Fixes

- **(fish)** cache usage spec in global by [@jdx](https://github.com/jdx) in [0b06c6c](https://github.com/jdx/usage/commit/0b06c6c5c4e7f30a97f5102faff302fa3e3c62e0)
- show full path for file completions by [@jdx](https://github.com/jdx) in [eb18a91](https://github.com/jdx/usage/commit/eb18a91bb0e2245d1946ab89cdb9316da54d76f8)

### 🔍 Other Changes

- Update index.md by [@jdx](https://github.com/jdx) in [1a113d9](https://github.com/jdx/usage/commit/1a113d96f8ff49e25c1d54f0cc023e6425ad44c4)

## [1.3.0](https://github.com/jdx/usage/compare/v1.2.0..v1.3.0) - 2024-11-10

### 🚀 Features

- min_usage_version by [@jdx](https://github.com/jdx) in [#166](https://github.com/jdx/usage/pull/166)

### 🐛 Bug Fixes

- **(fig)** better generate spec for fig mount commands by [@miguelmig](https://github.com/miguelmig) in [#165](https://github.com/jdx/usage/pull/165)
- completions for bins with dashes by [@jdx](https://github.com/jdx) in [adbb347](https://github.com/jdx/usage/commit/adbb3478b86a4eede4f9812c73fc547f13f00842)
- bash script with snake case escapes by [@jdx](https://github.com/jdx) in [4e5ba4a](https://github.com/jdx/usage/commit/4e5ba4a6fa9d3adfe04c27a24b489c15af94ef69)

### 📦️ Dependency Updates

- update dependency vitepress to v1.5.0 by [@renovate[bot]](https://github.com/renovate[bot]) in [#160](https://github.com/jdx/usage/pull/160)

## [1.2.0](https://github.com/jdx/usage/compare/v1.1.1..v1.2.0) - 2024-11-05

### 🚀 Features

- added cache-key to generated completions by [@jdx](https://github.com/jdx) in [#159](https://github.com/jdx/usage/pull/159)

### 🐛 Bug Fixes

- require --file or --usage-cmd on `usage g completion` by [@jdx](https://github.com/jdx) in [3cae2ae](https://github.com/jdx/usage/commit/3cae2ae4a1ad6a97358bb49d9d0f3e15c65feb40)

## [1.1.1](https://github.com/jdx/usage/compare/v1.0.1..v1.1.1) - 2024-11-04

### 🚀 Features

- added clap_usage by [@jdx](https://github.com/jdx) in [#150](https://github.com/jdx/usage/pull/150)
- added completions for usage-cli itself by [@jdx](https://github.com/jdx) in [#151](https://github.com/jdx/usage/pull/151)

### 🐛 Bug Fixes

- pass exit codes with `usage bash` and `usage exec` by [@jdx](https://github.com/jdx) in [#152](https://github.com/jdx/usage/pull/152)
- tweaks to fig completions by [@jdx](https://github.com/jdx) in [#153](https://github.com/jdx/usage/pull/153)
- Include the generator for mount run commands by [@miguelmig](https://github.com/miguelmig) in [#154](https://github.com/jdx/usage/pull/154)

### 📚 Documentation

- fix highlighting by [@jdx](https://github.com/jdx) in [c03b934](https://github.com/jdx/usage/commit/c03b9348d0472891eef45a4ca4db1786bdb3683e)

### 🔍 Other Changes

- generate markdown examples by [@jdx](https://github.com/jdx) in [5057650](https://github.com/jdx/usage/commit/50576504497435138d301d3f2ee18258c8c2e5c0)
- Add fig generate completion subcommand by [@miguelmig](https://github.com/miguelmig) in [#148](https://github.com/jdx/usage/pull/148)
- run render when creating release by [@jdx](https://github.com/jdx) in [7d723b7](https://github.com/jdx/usage/commit/7d723b7525cf54a58aaedfcbac7352334fd9c3bb)
- set clap_usage version by [@jdx](https://github.com/jdx) in [0a4909f](https://github.com/jdx/usage/commit/0a4909f39ac42b11bc4f9e0e23daf01466e12969)
- do not bump clap_usage on every release by [@jdx](https://github.com/jdx) in [2cac664](https://github.com/jdx/usage/commit/2cac6649ca29bfdad4cb9f3aee0573f6be587d1e)
- fix autolint action by [@jdx](https://github.com/jdx) in [#155](https://github.com/jdx/usage/pull/155)
- fix cli assets by [@jdx](https://github.com/jdx) in [ab8c6a0](https://github.com/jdx/usage/commit/ab8c6a0a14af1d4ec829660183ec58605afa33c7)

### 📦️ Dependency Updates

- update dependency vitepress to v1.4.5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#145](https://github.com/jdx/usage/pull/145)

### New Contributors

- @miguelmig made their first contribution in [#154](https://github.com/jdx/usage/pull/154)

## [1.0.1](https://github.com/jdx/usage/compare/v1.0.0..v1.0.1) - 2024-10-31

### 🐛 Bug Fixes

- allow calling `usage g completion -f` by [@jdx](https://github.com/jdx) in [#143](https://github.com/jdx/usage/pull/143)

### 📚 Documentation

- add bin name to `mise g completion` examples by [@jdx](https://github.com/jdx) in [8892b5b](https://github.com/jdx/usage/commit/8892b5b8c706ad4db46aa70753718436ec464fee)

## [1.0.0](https://github.com/jdx/usage/compare/v0.12.1..v1.0.0) - 2024-10-28

### 📚 Documentation

- document source_code_link_template by [@jdx](https://github.com/jdx) in [c408dad](https://github.com/jdx/usage/commit/c408dadeb3754c049a3db7aba882ba004e45aa9e)
- remove beta note by [@jdx](https://github.com/jdx) in [18045f6](https://github.com/jdx/usage/commit/18045f69f22579cee363ec03d65689b6f00f2d5e)

## [0.12.1](https://github.com/jdx/usage/compare/v0.12.0..v0.12.1) - 2024-10-27

### 🐛 Bug Fixes

- added backticks around source code link by [@jdx](https://github.com/jdx) in [53121fa](https://github.com/jdx/usage/commit/53121fabc8bcb3603474b0864a6f9add592bcabf)
- bug with missing source code template by [@jdx](https://github.com/jdx) in [3e3e303](https://github.com/jdx/usage/commit/3e3e30389a9c508b30f00c3751152ea51d2fc8fa)

## [0.12.0](https://github.com/jdx/usage/compare/v0.11.1..v0.12.0) - 2024-10-27

### 🚀 Features

- added source code links by [@jdx](https://github.com/jdx) in [6bc9c84](https://github.com/jdx/usage/commit/6bc9c84fc7a6efaf09e30af75925488f761834bd)

### 🐛 Bug Fixes

- use prettier-compatible md list syntax by [@jdx](https://github.com/jdx) in [2726bf2](https://github.com/jdx/usage/commit/2726bf22e7c4fabb48322b58813ff50bda698fe5)

## [0.11.1](https://github.com/jdx/usage/compare/v0.11.0..v0.11.1) - 2024-10-25

### 🐛 Bug Fixes

- fixed default arg/flags by [@jdx](https://github.com/jdx) in [#135](https://github.com/jdx/usage/pull/135)
- read choices from clap args by [@jdx](https://github.com/jdx) in [#136](https://github.com/jdx/usage/pull/136)

### 📦️ Dependency Updates

- update dawidd6/action-homebrew-bump-formula action to v4 by [@renovate[bot]](https://github.com/renovate[bot]) in [#131](https://github.com/jdx/usage/pull/131)
- update dependency vitepress to v1.4.1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#130](https://github.com/jdx/usage/pull/130)

## [0.11.0](https://github.com/jdx/usage/compare/v0.10.0..v0.11.0) - 2024-10-14

### 🚀 Features

- support single quotes in zsh descriptions by [@jasisk](https://github.com/jasisk) in [#128](https://github.com/jdx/usage/pull/128)
- render help in cli parsing by [@jdx](https://github.com/jdx) in [7c49fcb](https://github.com/jdx/usage/commit/7c49fcba4567da7ad8c7af9c4bb72a7c276a4a57)
- implemented more cli help for args/flags/subcommands by [@jdx](https://github.com/jdx) in [669f44e](https://github.com/jdx/usage/commit/669f44ea0459f997444c46ebfac1f42c00e210b4)

### 🐛 Bug Fixes

- bug with help and args by [@jdx](https://github.com/jdx) in [6c615f9](https://github.com/jdx/usage/commit/6c615f9f8b1c6798fcba3ed88890b2891505c6ec)
- allow building without docs feature by [@jdx](https://github.com/jdx) in [212f96c](https://github.com/jdx/usage/commit/212f96ccb118f393ed6d5141996e02ec3e3630d9)

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

### New Contributors

- @mise-en-dev made their first contribution in [#39](https://github.com/jdx/usage/pull/39)

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
