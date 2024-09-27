# Changelog

## [0.7.0](https://github.com/jdx/usage/compare/v0.6.0..v0.7.0) - 2024-09-26

### 🚀 Features

- implemented choices for args/flags by jdx in [3db63bd](https://github.com/jdx/usage/commit/3db63bd540d3fe796136218bdf6862f27a678767)

### 🔍 Other Changes

- clean up pub exports by Jeff Dickey in [9996ab8](https://github.com/jdx/usage/commit/9996ab8ca041d27a0754096fe7b04ebd3958431b)

## [0.6.0](https://github.com/jdx/usage/compare/v0.5.1..v0.6.0) - 2024-09-26

### 🚀 Features

- negate by Jeff Dickey in [5d1b817](https://github.com/jdx/usage/commit/5d1b817d143227a03651502b7671c9b2853c92eb)
- negate by Jeff Dickey in [16f754d](https://github.com/jdx/usage/commit/16f754d1925c561198291b304cbf80c9ab2a4dee)
- mount by Jeff Dickey in [99530f4](https://github.com/jdx/usage/commit/99530f4682140e2b64f2625d844b840925e3d6ae)

### 🐛 Bug Fixes

- remove debug statements by Jeff Dickey in [664b592](https://github.com/jdx/usage/commit/664b592f4d8f7b96f24d3bb2ca2803df36fda512)
- export SpecMount by Jeff Dickey in [b44c4f1](https://github.com/jdx/usage/commit/b44c4f15c77dee10e59c136b52f52a844f4ee655)

### 🔍 Other Changes

- migrate away from deprecated git-cliff syntax by Jeff Dickey in [3062df9](https://github.com/jdx/usage/commit/3062df94a9ad7af3a2e57ba5e5e35d299daa6718)

## [0.5.1](https://github.com/jdx/usage/compare/v0.5.0..v0.5.1) - 2024-09-25

### 🐛 Bug Fixes

- bail instead of panic on CLI parse error by Jeff Dickey in [b935cca](https://github.com/jdx/usage/commit/b935ccae9a442378c71182293cd24380fdadf744)

## [0.5.0](https://github.com/jdx/usage/compare/v0.4.0..v0.5.0) - 2024-09-25

### 🚀 Features

- added .as_env() to CLI parser by Jeff Dickey in [b1f6617](https://github.com/jdx/usage/commit/b1f66179b70a4bcdc6792add24a7b62e1afdd81d)
- added Spec::parse_script fn by Jeff Dickey in [124a705](https://github.com/jdx/usage/commit/124a7050c6b1b5bb502049204556b74b6e8a4b71)

## [0.4.0](https://github.com/jdx/usage/compare/v0.3.1..v0.4.0) - 2024-09-25

### 🚀 Features

- add comment syntax for file scripts by Jeff Dickey in [ee75493](https://github.com/jdx/usage/commit/ee7549303a0cf63c5da8257287be21d0af85ce86)

### 🐛 Bug Fixes

- tweak comment syntax by Jeff Dickey in [dfff6e2](https://github.com/jdx/usage/commit/dfff6e2daaafb47200a32d4654482beabbe2f343)

### 📚 Documentation

- update flag syntax by Jeff Dickey in [a67de2e](https://github.com/jdx/usage/commit/a67de2e6e855b24d340d559ded9e1464f95c2894)

### 📦️ Dependency Updates

- update rust crate serde to v1.0.210 by renovate[bot] in [866ed15](https://github.com/jdx/usage/commit/866ed15fc8b746c5b4a1bbfd3a61994b64dc0246)
- update rust crate clap to v4.5.18 by renovate[bot] in [161ba00](https://github.com/jdx/usage/commit/161ba0053ee58ac883cbe19b8f18c3790ec5d00a)

## [0.3.1](https://github.com/jdx/usage/compare/v0.3.0..v0.3.1) - 2024-08-28

### 🐛 Bug Fixes

- **(brew)** use official homebrew formula by jdx in [42269c9](https://github.com/jdx/usage/commit/42269c93ee608d734eb43ec8f306d4249f801c2a)
- make shebang scripts work with comments by Jeff Dickey in [9eb2a64](https://github.com/jdx/usage/commit/9eb2a64ff0e3c463f53fe0c283bbb932e5b3dd77)

### 📦️ Dependency Updates

- update dependency vitepress to v1.2.2 by renovate[bot] in [f74c0e1](https://github.com/jdx/usage/commit/f74c0e172181ea5364bbfcab7d4fbb484ef4f053)
- lock file maintenance by renovate[bot] in [4872230](https://github.com/jdx/usage/commit/48722300312e6e0edb1f03458ecbf13c96841976)
- update rust crate tera to v1.20.0 by renovate[bot] in [6fa06a4](https://github.com/jdx/usage/commit/6fa06a42f0bb94537b122856187760d59e2e5d07)
- lock file maintenance by renovate[bot] in [bbf39f3](https://github.com/jdx/usage/commit/bbf39f36964666848913cf88260852c93cff7b0e)
- update dependency vitepress to v1.2.3 by renovate[bot] in [b018c70](https://github.com/jdx/usage/commit/b018c704557729b01cb41e5e376b16261ce6278c)
- update rust crate clap to v4.5.6 by renovate[bot] in [8613fe1](https://github.com/jdx/usage/commit/8613fe1c9d6b555c192f0064de48b05d0271afe3)
- update rust crate clap to v4.5.7 by renovate[bot] in [233472d](https://github.com/jdx/usage/commit/233472d11d2d1e89a027cbfc4113d108becc7a3b)
- update rust crate regex to v1.10.5 by renovate[bot] in [5499b45](https://github.com/jdx/usage/commit/5499b455846ca36a3dfd071fa437480a617a4fd9)
- update rust crate log to v0.4.22 by renovate[bot] in [dae2215](https://github.com/jdx/usage/commit/dae2215a286d83e63352b3f035c7bac298c1dd58)
- update rust crate clap to v4.5.8 by renovate[bot] in [d6b32d5](https://github.com/jdx/usage/commit/d6b32d5989bc3ff2ecb917c66777d5e892c8536e)
- update rust crate serde to v1.0.204 by renovate[bot] in [98e346b](https://github.com/jdx/usage/commit/98e346b77c0e756bab3ccf71b355c17ad0291766)
- update rust crate clap to v4.5.9 by renovate[bot] in [a4f60f3](https://github.com/jdx/usage/commit/a4f60f377a5e783682446d907194b1aa1c9ae9a1)
- update rust crate strum to v0.26.3 by renovate[bot] in [4795d1e](https://github.com/jdx/usage/commit/4795d1edeab1ba307501f09d27bc9ac3cb8ff43f)
- update rust crate thiserror to v1.0.63 by renovate[bot] in [00daa8c](https://github.com/jdx/usage/commit/00daa8ce228945cab52bf792de172779b8cb969b)
- update dependency vitepress to v1.3.1 by renovate[bot] in [28087f2](https://github.com/jdx/usage/commit/28087f20007f028fea915d90f1f027066e91b941)
- lock file maintenance by renovate[bot] in [2a52de7](https://github.com/jdx/usage/commit/2a52de7b2def07464cf30369afad3e64298bb3f1)
- update rust crate predicates to v3.1.2 by renovate[bot] in [629c316](https://github.com/jdx/usage/commit/629c316361e8690e50bc196adcaa0e04b69b9cda)
- update rust crate assert_cmd to v2.0.15 by renovate[bot] in [1d68130](https://github.com/jdx/usage/commit/1d68130c8d4ff4e69d38afee863d2bd46a8c1167)
- update rust crate env_logger to v0.11.5 by renovate[bot] in [ebe133c](https://github.com/jdx/usage/commit/ebe133c944b9f163b71e768b14c4f57b7c282bd4)
- update rust crate clap to v4.5.13 by renovate[bot] in [a8e79bd](https://github.com/jdx/usage/commit/a8e79bd061a24b175a1a747b5bd5aa850672048b)
- update rust crate assert_cmd to v2.0.16 by renovate[bot] in [f6421c0](https://github.com/jdx/usage/commit/f6421c0e90c849261d37cab09869e257bedf637e)
- update dependency vitepress to v1.3.2 by renovate[bot] in [816f10f](https://github.com/jdx/usage/commit/816f10f3fdf27086bc62a9ee64bd10b85c02dd9b)
- update dependency vitepress to v1.3.3 by renovate[bot] in [f17b979](https://github.com/jdx/usage/commit/f17b97932341f73f375f405b1b7bc3e18f05339d)
- update rust crate clap to v4.5.16 by renovate[bot] in [db6d733](https://github.com/jdx/usage/commit/db6d73300f408a8696ccbda544de318fd67d3541)
- update dependency vitepress to v1.3.4 by renovate[bot] in [d34840a](https://github.com/jdx/usage/commit/d34840a1dd3f97706547666c4be958b41428d1e4)
- update rust crate regex to v1.10.6 by renovate[bot] in [49dece6](https://github.com/jdx/usage/commit/49dece6f8a19c32b48845449ddda203489c57deb)

## [0.3.0](https://github.com/jdx/usage/compare/v0.2.1..v0.3.0) - 2024-05-26

### 🚀 Features

- complete descriptions by Jeff Dickey in [a8afca7](https://github.com/jdx/usage/commit/a8afca7d6ad773431acfde8280e9dfb2884ef4e0)

## [0.2.1](https://github.com/jdx/usage/compare/v0.2.0..v0.2.1) - 2024-05-25

### 🔍 Other Changes

- updated deps by Jeff Dickey in [a457da9](https://github.com/jdx/usage/commit/a457da9ccec4890d63f3ab8e2215e51e64fd2425)

### 📦️ Dependency Updates

- update rust crate xx to v1 by renovate[bot] in [9ee59ba](https://github.com/jdx/usage/commit/9ee59badecf779217ebb2b5ce65e682b5650bf64)
- lock file maintenance by renovate[bot] in [5e30406](https://github.com/jdx/usage/commit/5e30406a7b4c086a4eecef080d7f9e6740b0dd00)
- update rust crate serde to v1.0.202 by renovate[bot] in [45fcd91](https://github.com/jdx/usage/commit/45fcd91205382d5f6608916ca3f01f619c0a7a27)
- update rust crate thiserror to v1.0.61 by renovate[bot] in [65b7d91](https://github.com/jdx/usage/commit/65b7d91c9d44b742c40fa94a78abd616c3325d5d)

## [0.2.0](https://github.com/jdx/usage/compare/v0.1.18..v0.2.0) - 2024-05-12

### 🚀 Features

- **(exec)** added `usage exec` command by jdx in [f6b8175](https://github.com/jdx/usage/commit/f6b817521b9c2ff4304dc9f4f70f327054a75b60)

### 🐛 Bug Fixes

- rust beta warning by Jeff Dickey in [8ba775e](https://github.com/jdx/usage/commit/8ba775e02daef37193fa0f43d59f4a4ad3081056)

### 🚜 Refactor

- created reusuable CLI parse function by Jeff Dickey in [8bc895a](https://github.com/jdx/usage/commit/8bc895a02ba6c7df32d47d0847b5b1985a2dbfdb)

### 📚 Documentation

- set GA by Jeff Dickey in [1a786c3](https://github.com/jdx/usage/commit/1a786c354a6e3f147453d8e6f38fb3916d21f889)
- update cliff.toml by Jeff Dickey in [df5f579](https://github.com/jdx/usage/commit/df5f579deac8d6f0fa2b0d2a492847950e338c94)

### 🔍 Other Changes

- **(aur)** added aur packaging by Jeff Dickey in [e00aff9](https://github.com/jdx/usage/commit/e00aff9739bf4c2286124cdb4724bd09f3b39a21)
- **(aur)** added aur packaging by Jeff Dickey in [e285fe9](https://github.com/jdx/usage/commit/e285fe9dcf6eabd684bb20607d64b8ebca29f663)
- **(release-plz)** fixed script by Jeff Dickey in [e4b2223](https://github.com/jdx/usage/commit/e4b2223da399ca30fa33917cf4088bb52ee7e49a)
- bump xx by Jeff Dickey in [c1bb0bb](https://github.com/jdx/usage/commit/c1bb0bb1c7600cf1ccb788c2d17651f6e93adf01)
- removed mega-linter by Jeff Dickey in [1aaa11f](https://github.com/jdx/usage/commit/1aaa11f49f9a5cd04419c2aebfb71b824f3c5ad1)
- fixing mise-action by Jeff Dickey in [c6a47fa](https://github.com/jdx/usage/commit/c6a47fa88cbd94de0fa0db2592a266b48c4c04ce)
- remove invalid config by Jeff Dickey in [eec7f7d](https://github.com/jdx/usage/commit/eec7f7d2324151bc809c45e514040dc353d544cc)
- better release PR title by Jeff Dickey in [849febb](https://github.com/jdx/usage/commit/849febbf6fc73fff6da6b3df15b9e31dad91580f)

### 📦️ Dependency Updates

- update dependency vitepress to v1.1.0 by renovate[bot] in [9ed3581](https://github.com/jdx/usage/commit/9ed3581f72acf66e5e96fbbbd1509ca6e9d9a15b)
- lock file maintenance by renovate[bot] in [16920d0](https://github.com/jdx/usage/commit/16920d0a112c884aa5ede6f5b712d9dee9dd3248)
- update dependency vitepress to v1.1.3 by renovate[bot] in [0f4911f](https://github.com/jdx/usage/commit/0f4911fcda305d39f63d44ea6a2b106ca802433e)
- lock file maintenance by renovate[bot] in [d6437e5](https://github.com/jdx/usage/commit/d6437e537e75ee54d323e4c064a50d52e915d079)
- update rust crate xx to 0.3 by renovate[bot] in [3298843](https://github.com/jdx/usage/commit/3298843a7a681aafe3df3bbf5e804fe7fcb398e2)
- update dependency vitepress to v1.1.4 by renovate[bot] in [8257ab4](https://github.com/jdx/usage/commit/8257ab46c5fd807d48ae86103f4d4a16475b94c7)
- lock file maintenance by renovate[bot] in [3c1e1e5](https://github.com/jdx/usage/commit/3c1e1e551e716ae5c98ede89a2e479a2b5af82ef)

## [0.1.18](https://github.com/jdx/usage/compare/v0.1.17..v0.1.18) - 2024-04-08

### 📚 Documentation

- **(changelog)** ran git-cliff by Jeff Dickey in [e2b6df1](https://github.com/jdx/usage/commit/e2b6df1b7fdb0318fa0eed709396cd202abd296b)
- improve CHANGELOG by jdx in [6280a73](https://github.com/jdx/usage/commit/6280a73c428ada2fda837db63f842eeb81c781ed)

### 🔍 Other Changes

- **(release-plz)** add all cargo files by Jeff Dickey in [6bc237d](https://github.com/jdx/usage/commit/6bc237d1babee025a0b4737781a6a742d93b7f4a)
- switch to dtolnay/rust-toolchain by Jeff Dickey in [d96d2a3](https://github.com/jdx/usage/commit/d96d2a37ff801d10868db265f26c10cf42181a11)

### 📦️ Dependency Updates

- update dependency vitepress to v1.0.1 by renovate[bot] in [37e4580](https://github.com/jdx/usage/commit/37e4580afecf336fe75b6edb9d906edeff199742)
- update actions/configure-pages action to v5 by renovate[bot] in [e08bae7](https://github.com/jdx/usage/commit/e08bae762d4c2619d47fef1ff98eb3809dc7a382)
- lock file maintenance by renovate[bot] in [f2149ec](https://github.com/jdx/usage/commit/f2149eca7937fbdb155c2ae206b3840b9a1f2a64)
- update dependency vitepress to v1.0.2 by renovate[bot] in [45f1fcb](https://github.com/jdx/usage/commit/45f1fcbc951440f982b8fbed7b3cea50ddac9226)
- lock file maintenance by renovate[bot] in [ee69aca](https://github.com/jdx/usage/commit/ee69acae0368a6dcefbcaadeb48af95cf1108b47)

## [0.1.17](https://github.com/jdx/usage/compare/v0.1.16..v0.1.17) - 2024-03-17

### 🔍 Other Changes

- ensure we publish the CLI by Jeff Dickey in [8b1f379](https://github.com/jdx/usage/commit/8b1f379ed94b5e85429846d0e3d1b0198a1449d1)
- bump release by Jeff Dickey in [3fa016a](https://github.com/jdx/usage/commit/3fa016a266753e9e5ebeb81eed61c74ced46e5cb)

## [0.1.16](https://github.com/jdx/usage/compare/v0.1.9..v0.1.16) - 2024-03-17

### 🐛 Bug Fixes

- **(completions)** add newline before error message by Jeff Dickey in [bbbafad](https://github.com/jdx/usage/commit/bbbafad126889ccc415e586b7601f7bb97c6f5a8)
- bug fix for release tagging by Jeff Dickey in [2c4832f](https://github.com/jdx/usage/commit/2c4832f7c7c67d8d5c477a11e56a49b487f574b8)

### 🚜 Refactor

- move usage-lib into its own dir by Jeff Dickey in [37e2379](https://github.com/jdx/usage/commit/37e2379122f123a85c4888e6efa1f62c631ac013)

### 🧪 Testing

- **(markdown-link-check)** ignore placeholder urls by Jeff Dickey in [6744453](https://github.com/jdx/usage/commit/67444538f25a11c09f842e20a5baa30fc3f41fae)
- **(markdown-link-check)** ignore placeholder urls by Jeff Dickey in [940dfb7](https://github.com/jdx/usage/commit/940dfb7cd5d1dbc8d2f1bab3029c1c4ba786f6ee)
- fix snapshots by Jeff Dickey in [0ea3d8b](https://github.com/jdx/usage/commit/0ea3d8b6ae7e3343c71c6d23b9e2b5d0f648a575)
- fix deprecation warnings by Jeff Dickey in [be8d6d5](https://github.com/jdx/usage/commit/be8d6d5b9090103d5596ff6a038ad63e538c1722)

### 🔍 Other Changes

- **(release-plz)** autopublish tag/gh release by Jeff Dickey in [5f78550](https://github.com/jdx/usage/commit/5f7855048912adda5ebfa6cfd2375cf5e5ccb79b)
- **(release-plz)** remove old logic by Jeff Dickey in [9ac8a0e](https://github.com/jdx/usage/commit/9ac8a0e95ae51398633486365a45a447bd8664e5)
- **(release-plz)** prefix versions with "v" by Jeff Dickey in [964503c](https://github.com/jdx/usage/commit/964503c57d8960abec4d6655257c1b904e585eba)
- added author field by Jeff Dickey in [b0e815a](https://github.com/jdx/usage/commit/b0e815a72bf4bfad6659a909a058cd86b7f9d56d)
- snapshots by Jeff Dickey in [3f0f16c](https://github.com/jdx/usage/commit/3f0f16c9b4fc2ff346a97644e97878916c1fa630)
- added brew tap to gh actions by Jeff Dickey in [e79f386](https://github.com/jdx/usage/commit/e79f386ff75bea7d35f3c90f0060a94656169c51)
- added git-cliff by Jeff Dickey in [6cca2bb](https://github.com/jdx/usage/commit/6cca2bbc77e459c45838e1957bc35eb42601a727)
- added release-please by Jeff Dickey in [e60127f](https://github.com/jdx/usage/commit/e60127f63a48a841b9aadfa04c9c4df045167dde)
- attempt to fix mega-linter by Jeff Dickey in [25a35e0](https://github.com/jdx/usage/commit/25a35e064c2ca29771d1c6b1ac5d2bea2b03b530)
- bootstrap release-please by jdx in [b6a7584](https://github.com/jdx/usage/commit/b6a758421231e33582c9571aa3690936faa1e59b)
- release-plz by Jeff Dickey in [b7aa490](https://github.com/jdx/usage/commit/b7aa490d7b401d86ac11569aae824951ab4de27c)
- cargo update by Jeff Dickey in [0aa872c](https://github.com/jdx/usage/commit/0aa872ca68822d32d9fa8a5228525124ed076abb)
- remove markdown link checker since it keeps failing by Jeff Dickey in [0668a1f](https://github.com/jdx/usage/commit/0668a1f6dae63bd3ea916939ab0a4c9c58fd0c13)
- fixing cargo metadata by Jeff Dickey in [64f19d7](https://github.com/jdx/usage/commit/64f19d7d40de0f897ccd22c07cd72e74b98b435f)
- use custom release-plz logic by Jeff Dickey in [bf4c151](https://github.com/jdx/usage/commit/bf4c151205d0560eefbf7a64cefd2524c57813db)
- bump version to try another release by Jeff Dickey in [badf251](https://github.com/jdx/usage/commit/badf251feb7fe86d763e4458261060b81f85fe7e)
- set metadata for usage-lib dependency by Jeff Dickey in [7e3538a](https://github.com/jdx/usage/commit/7e3538a304372c8d010386e22d39c02c9319d297)
- added git-cliff dependency by Jeff Dickey in [afd74d0](https://github.com/jdx/usage/commit/afd74d020d86fd77fe9b0696ae63863237297009)
- bump version to try another release by Jeff Dickey in [032f686](https://github.com/jdx/usage/commit/032f6860f569874e8ca2928f7db367191a8e69b3)
- bump release by Jeff Dickey in [4f3e3ea](https://github.com/jdx/usage/commit/4f3e3ea284968006e677402bd78afd3c592698b4)
- release on tags by Jeff Dickey in [6fd60be](https://github.com/jdx/usage/commit/6fd60be73ed06d62520fd2d39f175857243ec6e7)
- bump release by Jeff Dickey in [58be1c4](https://github.com/jdx/usage/commit/58be1c40f45fa86d1d8c6c6e58cbec85451c0d40)
- bump release by Jeff Dickey in [cd92e36](https://github.com/jdx/usage/commit/cd92e366ee60d9ea2cc6b43f9dadc7f27c0dd63e)

### 📦️ Dependency Updates

- update rust crate heck to v0.5.0 by renovate[bot] in [7026baf](https://github.com/jdx/usage/commit/7026baf79fe51e08310a1a9d0a56071445a1d0ea)
- update dependency vitepress to v1.0.0-rc.45 by renovate[bot] in [b4b8054](https://github.com/jdx/usage/commit/b4b8054d74d9df6826e2c44b051ec4823b646c0b)

## [0.1.9](https://github.com/jdx/usage/compare/v0.1.8..v0.1.9) - 2024-02-13

### 🐛 Bug Fixes

- fix actionlint by Jeff Dickey in [725bcf9](https://github.com/jdx/usage/commit/725bcf96055aafc9f0a58e0c8affe2c0ac7f3ba9)

### 🔍 Other Changes

- improve error by Jeff Dickey in [4621457](https://github.com/jdx/usage/commit/4621457b6cccde7f01ba60afe6c33870201975be)

## [0.1.8](https://github.com/jdx/usage/compare/v0.1.7..v0.1.8) - 2024-02-10

### 🐛 Bug Fixes

- fix binstall by Jeff Dickey in [a3b4513](https://github.com/jdx/usage/commit/a3b45132dd4b9f6b4d7a1ae224de455f28de75dd)

### 📦️ Dependency Updates

- update stefanzweifel/git-auto-commit-action action to v5 by renovate[bot] in [d94f323](https://github.com/jdx/usage/commit/d94f3231e91b21d39b8b3069298553308d64e721)

## [0.1.7](https://github.com/jdx/usage/compare/v0.1.6..v0.1.7) - 2024-02-10

### 🐛 Bug Fixes

- fix apple urls for binstall by Jeff Dickey in [06261f0](https://github.com/jdx/usage/commit/06261f0174bc0a95f216a9b22f85b0955f8c4a26)

## [0.1.6] - 2024-02-10

### 🔍 Other Changes

- add config for cargo-binstall by Jeff Dickey in [9711365](https://github.com/jdx/usage/commit/9711365fbfe1b39df03597af93caf9ca1b0e1b62)

### 📦️ Dependency Updates

- update actions/checkout action to v4 by renovate[bot] in [d764e27](https://github.com/jdx/usage/commit/d764e272964fa03207466a564469b63910599970)

<!-- generated by git-cliff -->
