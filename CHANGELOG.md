# Changelog

---
## [0.1.17](https://github.com/jdx/usage/compare/v0.1.16..v0.1.17) - 2024-03-17

### ⚙️ Miscellaneous Tasks

- ensure we publish the CLI by [@jdx](https://github.com/jdx) in [8b1f379](https://github.com/jdx/usage/commit/8b1f379ed94b5e85429846d0e3d1b0198a1449d1)
- bump release by [@jdx](https://github.com/jdx) in [3fa016a](https://github.com/jdx/usage/commit/3fa016a266753e9e5ebeb81eed61c74ced46e5cb)

---
## [0.1.16](https://github.com/jdx/usage/compare/v0.1.9..v0.1.16) - 2024-03-17

### 🐛 Bug Fixes

- **(completions)** add newline before error message by [@jdx](https://github.com/jdx) in [bbbafad](https://github.com/jdx/usage/commit/bbbafad126889ccc415e586b7601f7bb97c6f5a8)
- bug fix for release tagging by [@jdx](https://github.com/jdx) in [2c4832f](https://github.com/jdx/usage/commit/2c4832f7c7c67d8d5c477a11e56a49b487f574b8)

### 📦️ Dependency Updates

- update rust crate heck to v0.5.0 (#30) by [@renovate[bot]](https://github.com/renovate[bot]) in [#30](https://github.com/jdx/usage/pull/30)
- update dependency vitepress to v1.0.0-rc.45 (#28) by [@renovate[bot]](https://github.com/renovate[bot]) in [b4b8054](https://github.com/jdx/usage/commit/b4b8054d74d9df6826e2c44b051ec4823b646c0b)

### 🔍 Other Changes

- added author field by [@jdx](https://github.com/jdx) in [b0e815a](https://github.com/jdx/usage/commit/b0e815a72bf4bfad6659a909a058cd86b7f9d56d)
- snapshots by [@jdx](https://github.com/jdx) in [3f0f16c](https://github.com/jdx/usage/commit/3f0f16c9b4fc2ff346a97644e97878916c1fa630)
- added brew tap to gh actions by [@jdx](https://github.com/jdx) in [e79f386](https://github.com/jdx/usage/commit/e79f386ff75bea7d35f3c90f0060a94656169c51)

### 🚜 Refactor

- move usage-lib into its own dir by [@jdx](https://github.com/jdx) in [37e2379](https://github.com/jdx/usage/commit/37e2379122f123a85c4888e6efa1f62c631ac013)

### 🧪 Testing

- **(markdown-link-check)** ignore placeholder urls by [@jdx](https://github.com/jdx) in [6744453](https://github.com/jdx/usage/commit/67444538f25a11c09f842e20a5baa30fc3f41fae)
- **(markdown-link-check)** ignore placeholder urls by [@jdx](https://github.com/jdx) in [940dfb7](https://github.com/jdx/usage/commit/940dfb7cd5d1dbc8d2f1bab3029c1c4ba786f6ee)
- fix snapshots by [@jdx](https://github.com/jdx) in [0ea3d8b](https://github.com/jdx/usage/commit/0ea3d8b6ae7e3343c71c6d23b9e2b5d0f648a575)
- fix deprecation warnings by [@jdx](https://github.com/jdx) in [be8d6d5](https://github.com/jdx/usage/commit/be8d6d5b9090103d5596ff6a038ad63e538c1722)

### ⚙️ Miscellaneous Tasks

- **(release-plz)** autopublish tag/gh release by [@jdx](https://github.com/jdx) in [5f78550](https://github.com/jdx/usage/commit/5f7855048912adda5ebfa6cfd2375cf5e5ccb79b)
- **(release-plz)** remove old logic by [@jdx](https://github.com/jdx) in [9ac8a0e](https://github.com/jdx/usage/commit/9ac8a0e95ae51398633486365a45a447bd8664e5)
- **(release-plz)** prefix versions with "v" by [@jdx](https://github.com/jdx) in [964503c](https://github.com/jdx/usage/commit/964503c57d8960abec4d6655257c1b904e585eba)
- added git-cliff by [@jdx](https://github.com/jdx) in [6cca2bb](https://github.com/jdx/usage/commit/6cca2bbc77e459c45838e1957bc35eb42601a727)
- added release-please by [@jdx](https://github.com/jdx) in [e60127f](https://github.com/jdx/usage/commit/e60127f63a48a841b9aadfa04c9c4df045167dde)
- attempt to fix mega-linter by [@jdx](https://github.com/jdx) in [25a35e0](https://github.com/jdx/usage/commit/25a35e064c2ca29771d1c6b1ac5d2bea2b03b530)
- bootstrap release-please (#31) by [@jdx](https://github.com/jdx) in [b6a7584](https://github.com/jdx/usage/commit/b6a758421231e33582c9571aa3690936faa1e59b)
- release-plz by [@jdx](https://github.com/jdx) in [b7aa490](https://github.com/jdx/usage/commit/b7aa490d7b401d86ac11569aae824951ab4de27c)
- cargo update by [@jdx](https://github.com/jdx) in [0aa872c](https://github.com/jdx/usage/commit/0aa872ca68822d32d9fa8a5228525124ed076abb)
- remove markdown link checker since it keeps failing by [@jdx](https://github.com/jdx) in [0668a1f](https://github.com/jdx/usage/commit/0668a1f6dae63bd3ea916939ab0a4c9c58fd0c13)
- release (#39) by [@mise-en-dev](https://github.com/mise-en-dev) in [#39](https://github.com/jdx/usage/pull/39)
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

### New Contributors

* @mise-en-dev made their first contribution in [#39](https://github.com/jdx/usage/pull/39)

---
## [0.1.9](https://github.com/jdx/usage/compare/v0.1.8..v0.1.9) - 2024-02-13

### 🐛 Bug Fixes

- fix actionlint by [@jdx](https://github.com/jdx) in [725bcf9](https://github.com/jdx/usage/commit/725bcf96055aafc9f0a58e0c8affe2c0ac7f3ba9)

### 🔍 Other Changes

- improve error by [@jdx](https://github.com/jdx) in [4621457](https://github.com/jdx/usage/commit/4621457b6cccde7f01ba60afe6c33870201975be)

### ⚙️ Miscellaneous Tasks

- Release by [@jdx](https://github.com/jdx) in [1a10e64](https://github.com/jdx/usage/commit/1a10e641aa7803f6cc9fea98fea959e7e29b8430)

---
## [0.1.8](https://github.com/jdx/usage/compare/v0.1.7..v0.1.8) - 2024-02-10

### 🐛 Bug Fixes

- fix binstall by [@jdx](https://github.com/jdx) in [a3b4513](https://github.com/jdx/usage/commit/a3b45132dd4b9f6b4d7a1ae224de455f28de75dd)

### 📦️ Dependency Updates

- update stefanzweifel/git-auto-commit-action action to v5 (#25) by [@renovate[bot]](https://github.com/renovate[bot]) in [#25](https://github.com/jdx/usage/pull/25)

### ⚙️ Miscellaneous Tasks

- Release by [@jdx](https://github.com/jdx) in [258b63b](https://github.com/jdx/usage/commit/258b63b4bbfd4e20846d55ac40add4ac5d0ac28f)

---
## [0.1.7](https://github.com/jdx/usage/compare/v0.1.6..v0.1.7) - 2024-02-10

### 🐛 Bug Fixes

- fix apple urls for binstall by [@jdx](https://github.com/jdx) in [06261f0](https://github.com/jdx/usage/commit/06261f0174bc0a95f216a9b22f85b0955f8c4a26)

### ⚙️ Miscellaneous Tasks

- Release by [@jdx](https://github.com/jdx) in [2c07616](https://github.com/jdx/usage/commit/2c07616e0fda38fe589873c9c6674941b8ebd214)

---
## [0.1.6] - 2024-02-10

### 📦️ Dependency Updates

- update actions/checkout action to v4 (#23) by [@renovate[bot]](https://github.com/renovate[bot]) in [#23](https://github.com/jdx/usage/pull/23)

### 🔍 Other Changes

- add config for cargo-binstall by [@jdx](https://github.com/jdx) in [9711365](https://github.com/jdx/usage/commit/9711365fbfe1b39df03597af93caf9ca1b0e1b62)

### ⚙️ Miscellaneous Tasks

- Release by [@jdx](https://github.com/jdx) in [72c5834](https://github.com/jdx/usage/commit/72c58342396ff8f479043b7465526eb0fa735644)

<!-- generated by git-cliff -->
