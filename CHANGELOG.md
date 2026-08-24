# Changelog

## [6.3.0](https://github.com/jdx/usage/compare/v6.2.0..v6.3.0) - 2026-08-24

### 🚀 Features

- **(spec)** render heading prose in the Go help renderer by [@jdx](https://github.com/jdx) in [#1290](https://github.com/jdx/usage/pull/1290)

### 🐛 Bug Fixes

- **(cli)** keep long entries from widening help columns by [@jdx](https://github.com/jdx) in [#1293](https://github.com/jdx/usage/pull/1293)
- **(cli)** omit repeatability markers from output by [@jdx](https://github.com/jdx) in [#1295](https://github.com/jdx/usage/pull/1295)

## [6.2.0](https://github.com/jdx/usage/compare/v6.1.1..v6.2.0) - 2026-08-24

### 🚀 Features

- **(argv)** add embedded parse outcomes by [@jdx](https://github.com/jdx) in [#1250](https://github.com/jdx/usage/pull/1250)
- **(cli)** render inline formatting in help text by [@jdx](https://github.com/jdx) in [#1245](https://github.com/jdx/usage/pull/1245)
- **(cli)** split grouped help template sections by [@jdx](https://github.com/jdx) in [#1251](https://github.com/jdx/usage/pull/1251)
- **(complete)** add presentation labels to candidates by [@jdx](https://github.com/jdx) in [#1239](https://github.com/jdx/usage/pull/1239)
- **(complete)** expose structured completion traces by [@jdx](https://github.com/jdx) in [#1241](https://github.com/jdx/usage/pull/1241)
- **(complete)** add semantic candidate kinds by [@jdx](https://github.com/jdx) in [#1242](https://github.com/jdx/usage/pull/1242)
- **(complete)** add Elvish runtime completions by [@jdx](https://github.com/jdx) in [#1243](https://github.com/jdx/usage/pull/1243)
- **(derive)** let argument groups carry values by [@jdx](https://github.com/jdx) in [#1253](https://github.com/jdx/usage/pull/1253)
- **(derive)** add typed command finalization by [@jdx](https://github.com/jdx) in [#1254](https://github.com/jdx/usage/pull/1254)
- **(derive)** add runtime-computed defaults by [@jdx](https://github.com/jdx) in [#1256](https://github.com/jdx/usage/pull/1256)
- **(derive)** dispatch embedded control requests by [@jdx](https://github.com/jdx) in [#1270](https://github.com/jdx/usage/pull/1270)
- **(derive)** emit embedded_outcome_into for converted CLIs by [@jdx](https://github.com/jdx) in [#1281](https://github.com/jdx/usage/pull/1281)
- **(docs)** allow overriding markdown templates by [@jdx](https://github.com/jdx) in [#1267](https://github.com/jdx/usage/pull/1267)
- **(docs)** default to compact markdown references by [@jdx](https://github.com/jdx) in [#1272](https://github.com/jdx/usage/pull/1272)
- **(docs)** polish compact markdown references by [@jdx](https://github.com/jdx) in [#1280](https://github.com/jdx/usage/pull/1280)
- **(help)** expose addressable help topics by [@jdx](https://github.com/jdx) in [#1257](https://github.com/jdx/usage/pull/1257)
- **(help)** list commands by name in one aligned column by [@jdx](https://github.com/jdx) in [#1284](https://github.com/jdx/usage/pull/1284)
- **(help)** wrap the short help page by [@jdx](https://github.com/jdx) in [#1287](https://github.com/jdx/usage/pull/1287)
- **(parse)** add structured diagnostic reports by [@jdx](https://github.com/jdx) in [#1255](https://github.com/jdx/usage/pull/1255)
- **(parse)** add opt-in response files by [@jdx](https://github.com/jdx) in [#1259](https://github.com/jdx/usage/pull/1259)
- **(parse)** preserve ordered argument groups by [@jdx](https://github.com/jdx) in [#1271](https://github.com/jdx/usage/pull/1271)
- **(spec)** declare command outputs and exit codes by [@jdx](https://github.com/jdx) in [#1249](https://github.com/jdx/usage/pull/1249)
- **(spec)** add surface availability metadata by [@jdx](https://github.com/jdx) in [#1258](https://github.com/jdx/usage/pull/1258)
- **(spec)** add semantic note and warning blocks by [@jdx](https://github.com/jdx) in [#1273](https://github.com/jdx/usage/pull/1273)
- **(spec)** add output media types by [@jdx](https://github.com/jdx) in [#1274](https://github.com/jdx/usage/pull/1274)
- **(spec)** add help prose to heading sections by [@jdx](https://github.com/jdx) in [#1282](https://github.com/jdx/usage/pull/1282)
- add dynamic command catalogs by [@jdx](https://github.com/jdx) in [#1275](https://github.com/jdx/usage/pull/1275)

### 🐛 Bug Fixes

- **(completion)** handle attached values and emit built-ins by [@jdx](https://github.com/jdx) in [#1277](https://github.com/jdx/usage/pull/1277)
- **(derive)** preserve flattened command metadata by [@jdx](https://github.com/jdx) in [#1268](https://github.com/jdx/usage/pull/1268)
- **(derive)** skip choice checks for typed defaults by [@jdx](https://github.com/jdx) in [#1269](https://github.com/jdx/usage/pull/1269)
- **(derive)** suppress generated partial field lint by [@jdx](https://github.com/jdx) in [#1278](https://github.com/jdx/usage/pull/1278)
- **(derive)** keep an invalid choice after an override displaces the flag by [@jdx](https://github.com/jdx) in [#1286](https://github.com/jdx/usage/pull/1286)
- **(spec)** make the two KDL writers agree on three more nodes by [@jdx](https://github.com/jdx) in [#1289](https://github.com/jdx/usage/pull/1289)

### 🚜 Refactor

- **(deps)** replace versions with semver by [@jdx](https://github.com/jdx) in [#1285](https://github.com/jdx/usage/pull/1285)

### ⚡ Performance

- **(argv)** reduce sort code size by [@jdx](https://github.com/jdx) in [#1264](https://github.com/jdx/usage/pull/1264)
- **(markdown)** skip empty admonition context by [@jdx](https://github.com/jdx) in [#1279](https://github.com/jdx/usage/pull/1279)
- document usage-rs parser tradeoffs by [@jdx](https://github.com/jdx) in [#1265](https://github.com/jdx/usage/pull/1265)

### 🛡️ Security

- **(complete)** filter path candidates by extension by [@jdx](https://github.com/jdx) in [#1240](https://github.com/jdx/usage/pull/1240)

### 🔍 Other Changes

- update usage of deprecated `str downcase` thingy in nushell by [@TheBearodactyl](https://github.com/TheBearodactyl) in [#1262](https://github.com/jdx/usage/pull/1262)

### New Contributors

- @TheBearodactyl made their first contribution in [#1262](https://github.com/jdx/usage/pull/1262)

## [6.1.1](https://github.com/jdx/usage/compare/v6.1.0..v6.1.1) - 2026-08-23

### 🐛 Bug Fixes

- **(argv)** simplify generated completion headers by [@jdx](https://github.com/jdx) in [#1226](https://github.com/jdx/usage/pull/1226)
- **(argv)** plan for the target platform, not the host by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#1233](https://github.com/jdx/usage/pull/1233)
- **(complete)** keep the path separator the caller typed by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#1230](https://github.com/jdx/usage/pull/1230)
- **(config)** report config paths without the verbatim prefix by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#1232](https://github.com/jdx/usage/pull/1232)
- **(docs)** separate visible flag aliases by [@jdx](https://github.com/jdx) in [#1228](https://github.com/jdx/usage/pull/1228)
- **(test)** compile the platform-conditional fixtures warning-free on windows by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#1234](https://github.com/jdx/usage/pull/1234)

### ⚡ Performance

- **(derive)** outline invalid-value error construction from generated builds by [@jdx](https://github.com/jdx) in [#1235](https://github.com/jdx/usage/pull/1235)
- **(derive)** share the repeated-value collection loop across fields by [@jdx](https://github.com/jdx) in [#1236](https://github.com/jdx/usage/pull/1236)

### 🧪 Testing

- **(windows)** let the suite run where zsh, fish and bash-completion are not by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#1229](https://github.com/jdx/usage/pull/1229)

## [6.1.0](https://github.com/jdx/usage/compare/v6.0.0..v6.1.0) - 2026-08-22

### 🚀 Features

- **(cli)** read settings under a prefix mise does not strip by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#1213](https://github.com/jdx/usage/pull/1213)
- **(derive)** dispatch more of the matches CLIs already write by [@jdx](https://github.com/jdx) in [#1221](https://github.com/jdx/usage/pull/1221)
- **(spec)** apply runtime identity and flatten headings in help by [@jdx](https://github.com/jdx) in [#1220](https://github.com/jdx/usage/pull/1220)

### 🐛 Bug Fixes

- **(derive)** flow long help and emit kdl raw multiline strings by [@jdx](https://github.com/jdx) in [#1215](https://github.com/jdx/usage/pull/1215)

### 📚 Documentation

- **(rust)** drop the restated one-declaration line from the intro by [@jdx](https://github.com/jdx) in [#1211](https://github.com/jdx/usage/pull/1211)
- **(spec)** complete KDL reference by [@jdx](https://github.com/jdx) in [#1214](https://github.com/jdx/usage/pull/1214)

## [6.0.0](https://github.com/jdx/usage/compare/v5.1.0..v6.0.0) - 2026-08-22

### 🚀 Features

- **(argv)** add a zero-allocation argv parser by [@jdx](https://github.com/jdx) in [#798](https://github.com/jdx/usage/pull/798)
- **(argv)** emit a usage spec from static metadata by [@jdx](https://github.com/jdx) in [#801](https://github.com/jdx/usage/pull/801)
- **(argv)** a bound stops a variadic by [@jdx](https://github.com/jdx) in [#826](https://github.com/jdx/usage/pull/826)
- **(argv)** route a word that names nothing to the default subcommand by [@jdx](https://github.com/jdx) in [#848](https://github.com/jdx/usage/pull/848)
- **(argv)** join static tables at compile time by [@jdx](https://github.com/jdx) in [#851](https://github.com/jdx/usage/pull/851)
- **(argv)** render the usage line, byte-identical to usage-lib's by [@jdx](https://github.com/jdx) in [#854](https://github.com/jdx/usage/pull/854)
- **(argv)** render `-h`, byte-identical to usage-lib's by [@jdx](https://github.com/jdx) in [#860](https://github.com/jdx/usage/pull/860)
- **(argv)** render `--help` too, byte-identical to usage-lib's by [@jdx](https://github.com/jdx) in [#866](https://github.com/jdx/usage/pull/866)
- **(argv)** answer `--help` and `-h` by [@jdx](https://github.com/jdx) in [#870](https://github.com/jdx/usage/pull/870)
- **(argv)** answer the `help` subcommand by [@jdx](https://github.com/jdx) in [#872](https://github.com/jdx/usage/pull/872)
- **(argv)** split a command line the way the shell that typed it would by [@jdx](https://github.com/jdx) in [#874](https://github.com/jdx/usage/pull/874)
- **(argv)** read the cursor's position off a real parse by [@jdx](https://github.com/jdx) in [#876](https://github.com/jdx/usage/pull/876)
- **(argv)** offer what the reference offers, from compiled tables by [@jdx](https://github.com/jdx) in [#877](https://github.com/jdx/usage/pull/877)
- **(argv)** generate the shell script each shell wants by [@jdx](https://github.com/jdx) in [#887](https://github.com/jdx/usage/pull/887)
- **(argv)** let a Rust function answer for a value by [@jdx](https://github.com/jdx) in [#888](https://github.com/jdx/usage/pull/888)
- **(argv)** write the `run=` a declared completer answers by [@jdx](https://github.com/jdx) in [#890](https://github.com/jdx/usage/pull/890)
- **(argv)** say what went wrong the way clap says it by [@jdx](https://github.com/jdx) in [#895](https://github.com/jdx/usage/pull/895)
- **(argv)** suggest what was probably meant by [@jdx](https://github.com/jdx) in [#897](https://github.com/jdx/usage/pull/897)
- **(argv)** answer `--version`, which an adopter loses on the way from clap by [@jdx](https://github.com/jdx) in [#909](https://github.com/jdx/usage/pull/909)
- **(argv)** a flag whose value may be left off by [@jdx](https://github.com/jdx) in [#969](https://github.com/jdx/usage/pull/969)
- **(argv)** take flag-like detached values when declared by [@jdx](https://github.com/jdx) in [#1012](https://github.com/jdx/usage/pull/1012)
- **(bench)** count what a parse allocates, and stop allocating for commands nobody ran by [@jdx](https://github.com/jdx) in [#829](https://github.com/jdx/usage/pull/829)
- **(cli)** hold a spec's declaration order, the way clap-sort holds a clap CLI's by [@jdx](https://github.com/jdx) in [#915](https://github.com/jdx/usage/pull/915)
- **(cli)** parse usage's own command line with the parser usage ships by [@jdx](https://github.com/jdx) in [#965](https://github.com/jdx/usage/pull/965)
- **(cli)** support long version text by [@jdx](https://github.com/jdx) in [#1120](https://github.com/jdx/usage/pull/1120)
- **(cli)** check that examples still parse, and let the derive declare them by [@jdx](https://github.com/jdx) in [#1168](https://github.com/jdx/usage/pull/1168)
- **(cli)** add usage explain by [@jdx](https://github.com/jdx) in [#1179](https://github.com/jdx/usage/pull/1179)
- **(cli)** add usage diff for spec compatibility checking by [@jdx](https://github.com/jdx) in [#1171](https://github.com/jdx/usage/pull/1171)
- **(complete)** complete config keys and values from the spec by [@jdx](https://github.com/jdx) in [#840](https://github.com/jdx/usage/pull/840)
- **(complete)** add async runtime overlays by [@jdx](https://github.com/jdx) in [#1060](https://github.com/jdx/usage/pull/1060)
- **(complete)** support command value hints by [@jdx](https://github.com/jdx) in [#1081](https://github.com/jdx/usage/pull/1081)
- **(complete)** add shell quoting filter by [@jdx](https://github.com/jdx) in [#1114](https://github.com/jdx/usage/pull/1114)
- **(complete)** support full value hint vocabulary by [@jdx](https://github.com/jdx) in [#1119](https://github.com/jdx/usage/pull/1119)
- **(complete)** expand partial path segments by [@jdx](https://github.com/jdx) in [#1128](https://github.com/jdx/usage/pull/1128)
- **(complete)** support shell alias registration by [@jdx](https://github.com/jdx) in [#1158](https://github.com/jdx/usage/pull/1158)
- **(complete)** **breaking** remove the vendored bash-completion copy by [@jdx](https://github.com/jdx) in [#1176](https://github.com/jdx/usage/pull/1176)
- **(complete)** install a completion script where its shell looks for it by [@jdx](https://github.com/jdx) in [#1188](https://github.com/jdx/usage/pull/1188)
- **(config)** read config files as a layer by [@jdx](https://github.com/jdx) in [#856](https://github.com/jdx/usage/pull/856)
- **(config)** explain why a setting has the value it has by [@jdx](https://github.com/jdx) in [#857](https://github.com/jdx/usage/pull/857)
- **(config)** read a resolution as the types a struct holds by [@jdx](https://github.com/jdx) in [#862](https://github.com/jdx/usage/pull/862)
- **(config)** generate the settings registry from the spec by [@jdx](https://github.com/jdx) in [#864](https://github.com/jdx/usage/pull/864)
- **(config)** generate the settings struct a CLI reads by [@jdx](https://github.com/jdx) in [#865](https://github.com/jdx/usage/pull/865)
- **(config)** hold a value to the choices its setting declares by [@jdx](https://github.com/jdx) in [#868](https://github.com/jdx/usage/pull/868)
- **(config)** carry a setting's choices into the generated registry by [@jdx](https://github.com/jdx) in [#869](https://github.com/jdx/usage/pull/869)
- **(config)** say what sort of thing each warning is by [@jdx](https://github.com/jdx) in [#873](https://github.com/jdx/usage/pull/873)
- **(config)** carry the flags a setting declares into its registry by [@jdx](https://github.com/jdx) in [#880](https://github.com/jdx/usage/pull/880)
- **(config)** read the command line as a layer by [@jdx](https://github.com/jdx) in [#881](https://github.com/jdx/usage/pull/881)
- **(config)** compare the flags a spec declares with the flags a CLI binds by [@jdx](https://github.com/jdx) in [#884](https://github.com/jdx/usage/pull/884)
- **(config)** support optional props and aliases by [@jdx](https://github.com/jdx) in [#1134](https://github.com/jdx/usage/pull/1134)
- **(config)** read YAML config files by [@jdx](https://github.com/jdx) in [#1192](https://github.com/jdx/usage/pull/1192)
- **(config)** ask for provenance by key, like a value by [@jdx](https://github.com/jdx) in [#1195](https://github.com/jdx/usage/pull/1195)
- **(config)** a read that keeps every setting that reads by [@jdx](https://github.com/jdx) in [#1196](https://github.com/jdx/usage/pull/1196)
- **(config)** close Config derive and spec authoring gaps by [@jdx](https://github.com/jdx) in [#1202](https://github.com/jdx/usage/pull/1202)
- **(config)** gate deprecated settings by explicit CLI version by [@jdx](https://github.com/jdx) in [#1201](https://github.com/jdx/usage/pull/1201)
- **(derive)** compile a struct into parse tables and a spec by [@jdx](https://github.com/jdx) in [#803](https://github.com/jdx/usage/pull/803)
- **(derive)** compile subcommands from an enum by [@jdx](https://github.com/jdx) in [#816](https://github.com/jdx/usage/pull/816)
- **(derive)** check what a parse cannot decide on its own by [@jdx](https://github.com/jdx) in [#817](https://github.com/jdx/usage/pull/817)
- **(derive)** nest commands to any depth by [@jdx](https://github.com/jdx) in [#818](https://github.com/jdx/usage/pull/818)
- **(derive)** declare which flags conflict and which require each other by [@jdx](https://github.com/jdx) in [#820](https://github.com/jdx/usage/pull/820)
- **(derive)** let a flag displace another, the last one given winning by [@jdx](https://github.com/jdx) in [#821](https://github.com/jdx/usage/pull/821)
- **(derive)** let a command answer to more than one name by [@jdx](https://github.com/jdx) in [#827](https://github.com/jdx/usage/pull/827)
- **(derive)** let a variant hold its command in a `Box` by [@jdx](https://github.com/jdx) in [#828](https://github.com/jdx/usage/pull/828)
- **(derive)** let a field be the type it means by [@jdx](https://github.com/jdx) in [#833](https://github.com/jdx/usage/pull/833)
- **(derive)** declare the words a value may be by [@jdx](https://github.com/jdx) in [#838](https://github.com/jdx/usage/pull/838)
- **(derive)** hold the bytes a word arrived as by [@jdx](https://github.com/jdx) in [#841](https://github.com/jdx/usage/pull/841)
- **(derive)** declare the properties mise patches in by hand by [@jdx](https://github.com/jdx) in [#842](https://github.com/jdx/usage/pull/842)
- **(derive)** accept a value the OS accepts and UTF-8 does not by [@jdx](https://github.com/jdx) in [#844](https://github.com/jdx/usage/pull/844)
- **(derive)** share declarations between commands with flatten by [@jdx](https://github.com/jdx) in [#852](https://github.com/jdx/usage/pull/852)
- **(derive)** say three things about a CLI the spec could and the derive could not by [@jdx](https://github.com/jdx) in [#853](https://github.com/jdx/usage/pull/853)
- **(derive)** answer a completion request from the binary itself by [@jdx](https://github.com/jdx) in [#885](https://github.com/jdx/usage/pull/885)
- **(derive)** bind a flag to a setting, from what the parser saw by [@jdx](https://github.com/jdx) in [#889](https://github.com/jdx/usage/pull/889)
- **(derive)** a setting can be declared wherever a flag is by [@jdx](https://github.com/jdx) in [#896](https://github.com/jdx/usage/pull/896)
- **(derive)** let a field name the function that completes it by [@jdx](https://github.com/jdx) in [#892](https://github.com/jdx/usage/pull/892)
- **(derive)** say how an argument relates to `--`, all four ways by [@jdx](https://github.com/jdx) in [#900](https://github.com/jdx/usage/pull/900)
- **(derive)** a default a collecting field can hold by [@jdx](https://github.com/jdx) in [#902](https://github.com/jdx/usage/pull/902)
- **(derive)** say what a command does to the world by [@jdx](https://github.com/jdx) in [#905](https://github.com/jdx/usage/pull/905)
- **(derive)** name a value the way clap names it, and say which usage can read the spec by [@jdx](https://github.com/jdx) in [#907](https://github.com/jdx/usage/pull/907)
- **(derive)** let `parse()` answer a failure the way a program does by [@jdx](https://github.com/jdx) in [#910](https://github.com/jdx/usage/pull/910)
- **(derive)** read the package's version, and be called what the binary is called by [@jdx](https://github.com/jdx) in [#917](https://github.com/jdx/usage/pull/917)
- **(derive)** a command that takes nothing can be written that way by [@jdx](https://github.com/jdx) in [#923](https://github.com/jdx/usage/pull/923)
- **(derive)** say that a command cannot be run alone, which it knew and did not write by [@jdx](https://github.com/jdx) in [#937](https://github.com/jdx/usage/pull/937)
- **(derive)** keep command aliases on their args by [@jdx](https://github.com/jdx) in [#946](https://github.com/jdx/usage/pull/946)
- **(derive)** preserve verbatim doc comments by [@jdx](https://github.com/jdx) in [#949](https://github.com/jdx/usage/pull/949)
- **(derive)** support path value hints by [@jdx](https://github.com/jdx) in [#951](https://github.com/jdx/usage/pull/951)
- **(derive)** declare a group where the flags are declared by [@jdx](https://github.com/jdx) in [#934](https://github.com/jdx/usage/pull/934)
- **(derive)** add value-conditional requirements by [@jdx](https://github.com/jdx) in [#1002](https://github.com/jdx/usage/pull/1002)
- **(derive)** add skip for fields that are not arguments by [@jdx](https://github.com/jdx) in [#1009](https://github.com/jdx/usage/pull/1009)
- **(derive)** support inline subcommand fields by [@jdx](https://github.com/jdx) in [#1055](https://github.com/jdx/usage/pull/1055)
- **(derive)** accept runtime metadata expressions by [@jdx](https://github.com/jdx) in [#1056](https://github.com/jdx/usage/pull/1056)
- **(derive)** accept clap value attributes by [@jdx](https://github.com/jdx) in [#1057](https://github.com/jdx/usage/pull/1057)
- **(derive)** parse full argv with program name by [@jdx](https://github.com/jdx) in [#1063](https://github.com/jdx/usage/pull/1063)
- **(derive)** support clap no binary name by [@jdx](https://github.com/jdx) in [#1064](https://github.com/jdx/usage/pull/1064)
- **(derive)** support unit command structs by [@jdx](https://github.com/jdx) in [#1071](https://github.com/jdx/usage/pull/1071)
- **(derive)** reuse args across commands by [@jdx](https://github.com/jdx) in [#1076](https://github.com/jdx/usage/pull/1076)
- **(derive)** support runtime program identity by [@jdx](https://github.com/jdx) in [#1078](https://github.com/jdx/usage/pull/1078)
- **(derive)** preserve value enum metadata by [@jdx](https://github.com/jdx) in [#1079](https://github.com/jdx/usage/pull/1079)
- **(derive)** accept clap field spellings by [@jdx](https://github.com/jdx) in [#1086](https://github.com/jdx/usage/pull/1086)
- **(derive)** preserve hidden flag aliases by [@jdx](https://github.com/jdx) in [#1087](https://github.com/jdx/usage/pull/1087)
- **(derive)** resolve relationships through flatten by [@jdx](https://github.com/jdx) in [#1088](https://github.com/jdx/usage/pull/1088)
- **(derive)** support flattened overrides by [@jdx](https://github.com/jdx) in [#1089](https://github.com/jdx/usage/pull/1089)
- **(derive)** preserve flattened help headings by [@jdx](https://github.com/jdx) in [#1090](https://github.com/jdx/usage/pull/1090)
- **(derive)** support clap casing policies by [@jdx](https://github.com/jdx) in [#1094](https://github.com/jdx/usage/pull/1094)
- **(derive)** bind value enums directly by [@jdx](https://github.com/jdx) in [#1110](https://github.com/jdx/usage/pull/1110)
- **(derive)** accept portable clap field spellings by [@jdx](https://github.com/jdx) in [#1135](https://github.com/jdx/usage/pull/1135)
- **(derive)** inherit clap command metadata by [@jdx](https://github.com/jdx) in [#1136](https://github.com/jdx/usage/pull/1136)
- **(derive)** support clap implicit groups by [@jdx](https://github.com/jdx) in [#1137](https://github.com/jdx/usage/pull/1137)
- **(derive)** generate command dispatch by [@jdx](https://github.com/jdx) in [#1182](https://github.com/jdx/usage/pull/1182)
- **(derive)** add usage::Config derive for settings declared in code by [@jdx](https://github.com/jdx) in [#1180](https://github.com/jdx/usage/pull/1180)
- **(derive)** close remaining PLAN gaps for 6.x by [@jdx](https://github.com/jdx) in [#1197](https://github.com/jdx/usage/pull/1197)
- **(docs)** support granular help visibility by [@jdx](https://github.com/jdx) in [#1107](https://github.com/jdx/usage/pull/1107)
- **(docs)** customize subcommand presentation by [@jdx](https://github.com/jdx) in [#1108](https://github.com/jdx/usage/pull/1108)
- **(docs)** color process-facing help by [@jdx](https://github.com/jdx) in [#1111](https://github.com/jdx/usage/pull/1111)
- **(docs)** support help width controls by [@jdx](https://github.com/jdx) in [#1113](https://github.com/jdx/usage/pull/1113)
- **(docs)** support next-line help layout by [@jdx](https://github.com/jdx) in [#1117](https://github.com/jdx/usage/pull/1117)
- **(docs)** support flattened subcommand help by [@jdx](https://github.com/jdx) in [#1118](https://github.com/jdx/usage/pull/1118)
- **(docs)** support explicit display order by [@jdx](https://github.com/jdx) in [#1121](https://github.com/jdx/usage/pull/1121)
- **(docs)** group subcommands under help headings by [@jdx](https://github.com/jdx) in [#1153](https://github.com/jdx/usage/pull/1153)
- **(docs)** add recursive help by [@jdx](https://github.com/jdx) in [#1132](https://github.com/jdx/usage/pull/1132)
- **(generate)** add json-schema for a CLI's config file by [@jdx](https://github.com/jdx) in [#839](https://github.com/jdx/usage/pull/839)
- **(go)** emit Go parse tables from a spec, which is what Go has instead of a derive by [@jdx](https://github.com/jdx) in [#931](https://github.com/jdx/usage/pull/931)
- **(go)** emit the cold table too, so generated code can apply the rules by [@jdx](https://github.com/jdx) in [#959](https://github.com/jdx/usage/pull/959)
- **(go)** render the usage line, from a third table that costs nothing unused by [@jdx](https://github.com/jdx) in [#964](https://github.com/jdx/usage/pull/964)
- **(go)** render a failure as something a person can act on by [@jdx](https://github.com/jdx) in [#977](https://github.com/jdx/usage/pull/977)
- **(go)** generate a struct per command, and the Parse that fills them by [@jdx](https://github.com/jdx) in [#990](https://github.com/jdx/usage/pull/990)
- **(go)** answer the completion request a shell sends by [@jdx](https://github.com/jdx) in [#1005](https://github.com/jdx/usage/pull/1005)
- **(go)** enforce value-conditional requirements by [@jdx](https://github.com/jdx) in [#1003](https://github.com/jdx/usage/pull/1003)
- **(help)** line the flag column up, and give the short page a column at all by [@jdx](https://github.com/jdx) in [#912](https://github.com/jdx/usage/pull/912)
- **(help)** list the flags a command inherits by [@jdx](https://github.com/jdx) in [#913](https://github.com/jdx/usage/pull/913)
- **(help)** list `--help` and `--version`, which every page answers by [@jdx](https://github.com/jdx) in [#914](https://github.com/jdx/usage/pull/914)
- **(lib)** add usage-rs facade by [@jdx](https://github.com/jdx) in [#963](https://github.com/jdx/usage/pull/963)
- **(lib)** ship usage-rs as the one-crate rust default by [@jdx](https://github.com/jdx) in [#1041](https://github.com/jdx/usage/pull/1041)
- **(parse)** support inferred prefixes by [@jdx](https://github.com/jdx) in [#1080](https://github.com/jdx/usage/pull/1080)
- **(parse)** support arg required else help by [@jdx](https://github.com/jdx) in [#1093](https://github.com/jdx/usage/pull/1093)
- **(parse)** add narrow token boundary controls by [@jdx](https://github.com/jdx) in [#1097](https://github.com/jdx/usage/pull/1097)
- **(parse)** preserve trailing delimiters by [@jdx](https://github.com/jdx) in [#1098](https://github.com/jdx/usage/pull/1098)
- **(parse)** add scalar repeat policy by [@jdx](https://github.com/jdx) in [#1102](https://github.com/jdx/usage/pull/1102)
- **(parse)** add subcommand requirement policy by [@jdx](https://github.com/jdx) in [#1103](https://github.com/jdx/usage/pull/1103)
- **(parse)** add argument subcommand conflicts by [@jdx](https://github.com/jdx) in [#1104](https://github.com/jdx/usage/pull/1104)
- **(parse)** add subcommand value precedence by [@jdx](https://github.com/jdx) in [#1105](https://github.com/jdx/usage/pull/1105)
- **(parse)** support missing optional positionals by [@jdx](https://github.com/jdx) in [#1106](https://github.com/jdx/usage/pull/1106)
- **(parse)** support optional flag values by [@jdx](https://github.com/jdx) in [#1109](https://github.com/jdx/usage/pull/1109)
- **(parse)** support custom help and version actions by [@jdx](https://github.com/jdx) in [#1123](https://github.com/jdx/usage/pull/1123)
- **(parse)** accept explicit boolean values by [@jdx](https://github.com/jdx) in [#1124](https://github.com/jdx/usage/pull/1124)
- **(parse)** support non-strict choices by [@jdx](https://github.com/jdx) in [#1127](https://github.com/jdx/usage/pull/1127)
- **(parse)** support ordered environment fallbacks by [@jdx](https://github.com/jdx) in [#1130](https://github.com/jdx/usage/pull/1130)
- **(parse)** warn at runtime when a deprecated declaration is used by [@jdx](https://github.com/jdx) in [#1186](https://github.com/jdx/usage/pull/1186)
- **(spec)** support flag relationships by [@jdx](https://github.com/jdx) in [#793](https://github.com/jdx/usage/pull/793)
- **(spec)** add help_heading, and render it by [@jdx](https://github.com/jdx) in [#802](https://github.com/jdx/usage/pull/802)
- **(spec)** allow a mount at the top level by [@jdx](https://github.com/jdx) in [#806](https://github.com/jdx/usage/pull/806)
- **(spec)** make unknown flags configurable, and keep them as values by [@jdx](https://github.com/jdx) in [#810](https://github.com/jdx/usage/pull/810)
- **(spec)** add `conflicts` to flags by [@jdx](https://github.com/jdx) in [#819](https://github.com/jdx/usage/pull/819)
- **(spec)** say that one flag needs another, which nothing here could by [@jdx](https://github.com/jdx) in [#925](https://github.com/jdx/usage/pull/925)
- **(spec)** **breaking** a group, for the rule that no single flag can state by [@jdx](https://github.com/jdx) in [#927](https://github.com/jdx/usage/pull/927)
- **(spec)** a flag that has to be given on its own by [@jdx](https://github.com/jdx) in [#941](https://github.com/jdx/usage/pull/941)
- **(spec)** split a value the way clap splits one by [@jdx](https://github.com/jdx) in [#961](https://github.com/jdx/usage/pull/961)
- **(spec)** add value-conditional requirements by [@jdx](https://github.com/jdx) in [#1001](https://github.com/jdx/usage/pull/1001)
- **(spec)** refuse a detached value when require_equals is set by [@jdx](https://github.com/jdx) in [#1013](https://github.com/jdx/usage/pull/1013)
- **(spec)** bind a value when a flag is given with none by [@jdx](https://github.com/jdx) in [#1015](https://github.com/jdx/usage/pull/1015)
- **(spec)** forward unmatched words as an external subcommand by [@jdx](https://github.com/jdx) in [#1021](https://github.com/jdx/usage/pull/1021)
- **(spec)** bind a default when another flag is given by [@jdx](https://github.com/jdx) in [#1023](https://github.com/jdx/usage/pull/1023)
- **(spec)** add portable expression validation by [@jdx](https://github.com/jdx) in [#1037](https://github.com/jdx/usage/pull/1037)
- **(spec)** add borrowed metadata overlays by [@jdx](https://github.com/jdx) in [#1059](https://github.com/jdx/usage/pull/1059)
- **(spec)** omit versions from metadata views by [@jdx](https://github.com/jdx) in [#1066](https://github.com/jdx/usage/pull/1066)
- **(spec)** support positional conflicts and groups by [@jdx](https://github.com/jdx) in [#1085](https://github.com/jdx/usage/pull/1085)
- **(spec)** add fixed arity value names by [@jdx](https://github.com/jdx) in [#1099](https://github.com/jdx/usage/pull/1099)
- **(spec)** complete relationship families by [@jdx](https://github.com/jdx) in [#1100](https://github.com/jdx/usage/pull/1100)
- **(spec)** expose package metadata by [@jdx](https://github.com/jdx) in [#1116](https://github.com/jdx/usage/pull/1116)
- **(spec)** add deprecation milestones by [@jdx](https://github.com/jdx) in [#1129](https://github.com/jdx/usage/pull/1129)
- **(spec)** add executable views by [@jdx](https://github.com/jdx) in [#1143](https://github.com/jdx/usage/pull/1143)
- **(spec)** add deprecated config environment aliases by [@jdx](https://github.com/jdx) in [#1159](https://github.com/jdx/usage/pull/1159)
- **(spec)** declare source_code_link_template on the derive by [@jdx](https://github.com/jdx) in [#1184](https://github.com/jdx/usage/pull/1184)
- **(spec)** answer __usage_spec__ from a binary's own tables by [@jdx](https://github.com/jdx) in [#1183](https://github.com/jdx/usage/pull/1183)
- **(spec)** reusable flag declarations with flagset and use by [@jdx](https://github.com/jdx) in [#1170](https://github.com/jdx/usage/pull/1170)
- **(spec)** **breaking** lower the derive's flatten into a flagset by [@jdx](https://github.com/jdx) in [#1172](https://github.com/jdx/usage/pull/1172)
- **(test)** a test harness for an adopter's own suite by [@jdx](https://github.com/jdx) in [#1181](https://github.com/jdx/usage/pull/1181)

### 🐛 Bug Fixes

- **(argv)** stop a repeatable flag from eating a positional by [@jdx](https://github.com/jdx) in [#799](https://github.com/jdx/usage/pull/799)
- **(argv)** inherit `unknown_flags`, which reached one command out of a tree by [@jdx](https://github.com/jdx) in [#939](https://github.com/jdx/usage/pull/939)
- **(argv)** reject duplicate flags by [@jdx](https://github.com/jdx) in [#945](https://github.com/jdx/usage/pull/945)
- **(argv)** show choices when a subcommand is required by [@jdx](https://github.com/jdx) in [#947](https://github.com/jdx/usage/pull/947)
- **(argv)** a bare `-` binds where it was typed by [@jdx](https://github.com/jdx) in [#986](https://github.com/jdx/usage/pull/986)
- **(argv)** put zsh's magic comment first, and print fish's candidates as data by [@jdx](https://github.com/jdx) in [#1033](https://github.com/jdx/usage/pull/1033)
- **(ci)** unblock releases by cutting usage-derive's dev-dependency by [@jdx](https://github.com/jdx) in [#811](https://github.com/jdx/usage/pull/811)
- **(ci)** check the version the crates promise, and promise one that is true by [@jdx](https://github.com/jdx) in [#918](https://github.com/jdx/usage/pull/918)
- **(clap)** say what clap would do with an unknown flag by [@jdx](https://github.com/jdx) in [#899](https://github.com/jdx/usage/pull/899)
- **(cli)** recognize about as root command help by [@jdx](https://github.com/jdx) in [#794](https://github.com/jdx/usage/pull/794)
- **(complete)** resolve config keys through aliases and renames by [@jdx](https://github.com/jdx) in [#1169](https://github.com/jdx/usage/pull/1169)
- **(config)** accept case-insensitive boolean words by [@jdx](https://github.com/jdx) in [#1207](https://github.com/jdx/usage/pull/1207)
- **(derive)** let a `--`-only argument follow a variadic by [@jdx](https://github.com/jdx) in [#823](https://github.com/jdx/usage/pull/823)
- **(derive)** three more descriptions a spec keeps and the derive lost by [@jdx](https://github.com/jdx) in [#861](https://github.com/jdx/usage/pull/861)
- **(derive)** name the mistake when `settings` has nothing to collect by [@jdx](https://github.com/jdx) in [#904](https://github.com/jdx/usage/pull/904)
- **(derive)** emit the tables beside the user's types, not in a module above them by [@jdx](https://github.com/jdx) in [#938](https://github.com/jdx/usage/pull/938)
- **(derive)** a global flag may be given once per command, not once per line by [@jdx](https://github.com/jdx) in [#991](https://github.com/jdx/usage/pull/991)
- **(derive)** separate value metadata from parsing by [@jdx](https://github.com/jdx) in [#1054](https://github.com/jdx/usage/pull/1054)
- **(derive)** make defaulted fields optional in metadata by [@jdx](https://github.com/jdx) in [#1065](https://github.com/jdx/usage/pull/1065)
- **(derive)** isolate process exit from adopters by [@jdx](https://github.com/jdx) in [#1139](https://github.com/jdx/usage/pull/1139)
- **(derive)** propagate redeclared global values by [@jdx](https://github.com/jdx) in [#1140](https://github.com/jdx/usage/pull/1140)
- **(derive)** preserve set-false actions by [@jdx](https://github.com/jdx) in [#1156](https://github.com/jdx/usage/pull/1156)
- **(derive)** name the count type in standing presence checks by [@jdx](https://github.com/jdx) in [#1205](https://github.com/jdx/usage/pull/1205)
- **(docs)** link multi-word commands to their real source files by [@jdx](https://github.com/jdx) in [#845](https://github.com/jdx/usage/pull/845)
- **(docs)** link every command to the file that implements it by [@jdx](https://github.com/jdx) in [#846](https://github.com/jdx/usage/pull/846)
- **(docs)** keep hidden entries out of help by [@jdx](https://github.com/jdx) in [#859](https://github.com/jdx/usage/pull/859)
- **(docs)** list visible flag aliases by [@jdx](https://github.com/jdx) in [#1112](https://github.com/jdx/usage/pull/1112)
- **(help)** a command's page should say what that command does by [@jdx](https://github.com/jdx) in [#911](https://github.com/jdx/usage/pull/911)
- **(help)** a declared name is not a short form, and blank help is no help by [@jdx](https://github.com/jdx) in [#916](https://github.com/jdx/usage/pull/916)
- **(help)** render the page for the mount the words reached by [@jdx](https://github.com/jdx) in [#928](https://github.com/jdx/usage/pull/928)
- **(help)** a description ending in a break adds no blank line by [@jdx](https://github.com/jdx) in [#970](https://github.com/jdx/usage/pull/970)
- **(lib)** validate every variadic fallback by [@jdx](https://github.com/jdx) in [#1049](https://github.com/jdx/usage/pull/1049)
- **(parse)** keep every `--` after the first by [@jdx](https://github.com/jdx) in [#809](https://github.com/jdx/usage/pull/809)
- **(parse)** stop losing a flag that is missing its value by [@jdx](https://github.com/jdx) in [#807](https://github.com/jdx/usage/pull/807)
- **(parse)** answer the five vectors the reference implementation was failing by [@jdx](https://github.com/jdx) in [#930](https://github.com/jdx/usage/pull/930)
- **(parse)** **breaking** a command that needs a subcommand says so by [@jdx](https://github.com/jdx) in [#992](https://github.com/jdx/usage/pull/992)
- **(parse)** keep optional validation lint-clean by [@jdx](https://github.com/jdx) in [#1141](https://github.com/jdx/usage/pull/1141)
- **(parse)** honor separator after automatic args by [@jdx](https://github.com/jdx) in [#1164](https://github.com/jdx/usage/pull/1164)
- **(parse)** let a bundle contain a supplied short by [@jdx](https://github.com/jdx) in [#1175](https://github.com/jdx/usage/pull/1175)
- **(spec)** make the config block survive being written out by [@jdx](https://github.com/jdx) in [#832](https://github.com/jdx/usage/pull/832)
- **(spec)** apply default_subcommand only at the root by [@jdx](https://github.com/jdx) in [#850](https://github.com/jdx/usage/pull/850)
- **(spec)** split a clap default by the delimiter clap splits it by by [@jdx](https://github.com/jdx) in [#901](https://github.com/jdx/usage/pull/901)
- **(spec)** rank a subcommand name above another command's alias by [@jdx](https://github.com/jdx) in [#967](https://github.com/jdx/usage/pull/967)
- **(spec)** preserve clap value count bounds by [@jdx](https://github.com/jdx) in [#1032](https://github.com/jdx/usage/pull/1032)
- **(spec)** deduplicate derived completers by [@jdx](https://github.com/jdx) in [#1072](https://github.com/jdx/usage/pull/1072)
- **(spec)** canonicalize derived kdl by [@jdx](https://github.com/jdx) in [#1095](https://github.com/jdx/usage/pull/1095)

### 🚜 Refactor

- **(deps)** **breaking** stop shipping features and crates nobody uses by [@jdx](https://github.com/jdx) in [#1185](https://github.com/jdx/usage/pull/1185)
- **(deps)** drop heck from usage-derive by [@jdx](https://github.com/jdx) in [#1187](https://github.com/jdx/usage/pull/1187)
- **(deps)** take expr-lang without the builtins a spec cannot reach by [@jdx](https://github.com/jdx) in [#1191](https://github.com/jdx/usage/pull/1191)

### 📚 Documentation

- **(plan)** tick landed clap gaps and stop quoting vector counts by [@jdx](https://github.com/jdx) in [#1027](https://github.com/jdx/usage/pull/1027)
- correct current Rust limitations by [@jdx](https://github.com/jdx) in [#1029](https://github.com/jdx/usage/pull/1029)
- audit 6.x release documentation by [@jdx](https://github.com/jdx) in [#1084](https://github.com/jdx/usage/pull/1084)
- add third-party license notices by [@jdx](https://github.com/jdx) in [#1174](https://github.com/jdx/usage/pull/1174)

### ⚡ Performance

- **(derive)** fill the partial through &mut instead of returning it by [@jdx](https://github.com/jdx) in [#980](https://github.com/jdx/usage/pull/980)
- **(derive)** hold one subcommand's partial, not every subcommand's by [@jdx](https://github.com/jdx) in [#981](https://github.com/jdx/usage/pull/981)
- **(derive)** drop proc-macro-crate transitive deps by [@jdx](https://github.com/jdx) in [#1042](https://github.com/jdx/usage/pull/1042)

### 🧪 Testing

- **(clap)** preserve choices in external adopter probes by [@jdx](https://github.com/jdx) in [#1157](https://github.com/jdx/usage/pull/1157)
- **(corpus)** pin what completes where the cursor is by [@jdx](https://github.com/jdx) in [#998](https://github.com/jdx/usage/pull/998)
- **(derive)** cover verbatim doc compatibility by [@jdx](https://github.com/jdx) in [#1092](https://github.com/jdx/usage/pull/1092)
- **(docs)** preserve fleet footer spacing by [@jdx](https://github.com/jdx) in [#1142](https://github.com/jdx/usage/pull/1142)
- **(fleet)** refresh typed adopter fixtures by [@jdx](https://github.com/jdx) in [#1115](https://github.com/jdx/usage/pull/1115)
- **(parse)** cover mounted command discovery by [@jdx](https://github.com/jdx) in [#1131](https://github.com/jdx/usage/pull/1131)
- **(parse)** add clap micro-conformance by [@jdx](https://github.com/jdx) in [#1133](https://github.com/jdx/usage/pull/1133)
- **(spec)** import the argv questions clap's suite answers and ours did not by [@jdx](https://github.com/jdx) in [#926](https://github.com/jdx/usage/pull/926)
- **(spec)** verify portable parser settings by [@jdx](https://github.com/jdx) in [#1053](https://github.com/jdx/usage/pull/1053)

### 🛡️ Security

- **(config)** resolve settings from layers, with provenance by [@jdx](https://github.com/jdx) in [#849](https://github.com/jdx/usage/pull/849)
- **(config)** read the environment as a layer by [@jdx](https://github.com/jdx) in [#867](https://github.com/jdx/usage/pull/867)
- **(config)** give a deprecation notice from anywhere along a rename chain by [@jdx](https://github.com/jdx) in [#893](https://github.com/jdx/usage/pull/893)
- **(derive)** keep parsed fields live for lints by [@jdx](https://github.com/jdx) in [#1138](https://github.com/jdx/usage/pull/1138)
- **(docs)** render the config block by [@jdx](https://github.com/jdx) in [#837](https://github.com/jdx/usage/pull/837)
- **(go)** render the page `-h` prints, matching usage-lib on all 211 of mise's by [@jdx](https://github.com/jdx) in [#974](https://github.com/jdx/usage/pull/974)
- **(go)** render `--help` too, matching usage-lib on all 211 of mise's long pages by [@jdx](https://github.com/jdx) in [#975](https://github.com/jdx/usage/pull/975)
- **(parse)** require exact command and flag names by [@jdx](https://github.com/jdx) in [#1096](https://github.com/jdx/usage/pull/1096)
- **(spec)** the config vocabulary by [@jdx](https://github.com/jdx) in [#835](https://github.com/jdx/usage/pull/835)

### 🔍 Other Changes

- **(docs)** remove stale mise spec fixture by [@jdx](https://github.com/jdx) in [#1200](https://github.com/jdx/usage/pull/1200)
- **(perf)** say when the clap ratio slides, and record why the derive is stricter by [@jdx](https://github.com/jdx) in [#996](https://github.com/jdx/usage/pull/996)
- agent/complete files by [@jdx](https://github.com/jdx) in [#883](https://github.com/jdx/usage/pull/883)

### 📦️ Dependency Updates

- update rust crate syn to v3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#808](https://github.com/jdx/usage/pull/808)
- update rust crate toml to v1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#1016](https://github.com/jdx/usage/pull/1016)

## [5.1.0](https://github.com/jdx/usage/compare/v5.0.0..v5.1.0) - 2026-08-09

### 🚀 Features

- **(spec)** parse usage comments from strings by [@jdx](https://github.com/jdx) in [#782](https://github.com/jdx/usage/pull/782)

### 🐛 Bug Fixes

- **(spec)** avoid inferred metadata from included specs by [@jdx](https://github.com/jdx) in [#786](https://github.com/jdx/usage/pull/786)

### 🧪 Testing

- **(windows)** make the suite runnable on Windows by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#771](https://github.com/jdx/usage/pull/771)

### 📦️ Dependency Updates

- update rust crate rmcp to v3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#780](https://github.com/jdx/usage/pull/780)

## [5.0.0](https://github.com/jdx/usage/compare/v4.1.0..v5.0.0) - 2026-08-02

### 🚀 Features

- **(cli)** allow overriding the shell program with USAGE_SHELL_<SHELL> by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#767](https://github.com/jdx/usage/pull/767)

### 🐛 Bug Fixes

- **(cli)** forward parsed args to WSL bash via WSLENV on windows by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#764](https://github.com/jdx/usage/pull/764)
- **(cli)** let generate markdown write to stdout by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#766](https://github.com/jdx/usage/pull/766)
- **(complete)** use `type -P` so the CLI-presence guard ignores shell functions by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#760](https://github.com/jdx/usage/pull/760)
- **(parse)** enforce double_dash="required" for positional args by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#762](https://github.com/jdx/usage/pull/762)
- **(windows)** run `run=` scripts with sh when available by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#765](https://github.com/jdx/usage/pull/765)

### 🎨 Styling

- fix clippy and deprecation warnings in test and bench targets by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#763](https://github.com/jdx/usage/pull/763)

## [4.1.0](https://github.com/jdx/usage/compare/v4.0.0..v4.1.0) - 2026-07-30

### 🚀 Features

- **(cli)** declare what each usage command does to the world by [@jdx](https://github.com/jdx) in [#751](https://github.com/jdx/usage/pull/751)
- **(mcp)** serve a usage spec to an agent over stdio by [@jdx](https://github.com/jdx) in [#746](https://github.com/jdx/usage/pull/746)
- **(spec)** add a top-level `repository` field by [@jdx](https://github.com/jdx) in [#747](https://github.com/jdx/usage/pull/747)

### 🐛 Bug Fixes

- **(parse)** keep a re-declared global's aliases on one flag by [@jdx](https://github.com/jdx) in [#752](https://github.com/jdx/usage/pull/752)
- complete repeated variadic args by [@Jai-JAP](https://github.com/Jai-JAP) in [#753](https://github.com/jdx/usage/pull/753)

### New Contributors

- @Jai-JAP made their first contribution in [#753](https://github.com/jdx/usage/pull/753)

## [4.0.0](https://github.com/jdx/usage/compare/v3.6.0..v4.0.0) - 2026-07-25

### 🚀 Features

- **(spec)** allow effect= on flags and args by [@jdx](https://github.com/jdx) in [#742](https://github.com/jdx/usage/pull/742)

## [3.6.0](https://github.com/jdx/usage/compare/v3.5.7..v3.6.0) - 2026-07-25

### 🚀 Features

- **(spec)** add effect= to declare what a command does to the world by [@jdx](https://github.com/jdx) in [#739](https://github.com/jdx/usage/pull/739)

### 🚜 Refactor

- **(spec)** make missed SpecCommand fields a compile error, and fix the four that were already missed by [@jdx](https://github.com/jdx) in [#740](https://github.com/jdx/usage/pull/740)

## [3.5.7](https://github.com/jdx/usage/compare/v3.5.6..v3.5.7) - 2026-07-25

### 🐛 Bug Fixes

- **(parse)** don't leak the mounting CLI's flags into mounted commands; scan past non-global flags by [@jdx](https://github.com/jdx) in [#738](https://github.com/jdx/usage/pull/738)

## [3.5.6](https://github.com/jdx/usage/compare/v3.5.5..v3.5.6) - 2026-07-21

### 🐛 Bug Fixes

- **(cli)** avoid trailing semicolon in macro expression position by [@jdx](https://github.com/jdx) in [#729](https://github.com/jdx/usage/pull/729)
- **(completion)** write spec cache to private dir instead of world-writable tmp by [@jdx](https://github.com/jdx) in [#727](https://github.com/jdx/usage/pull/727)
- **(lib)** remove needless borrows in format args by [@jdx](https://github.com/jdx) in [#726](https://github.com/jdx/usage/pull/726)
- **(markdown)** preserve HTML in fenced code blocks by [@risu729](https://github.com/risu729) in [#720](https://github.com/jdx/usage/pull/720)
- **(nu)** create completion cache dir with mode 700 and fix home-dir lookup by [@jdx](https://github.com/jdx) in [#731](https://github.com/jdx/usage/pull/731)

## [3.5.5](https://github.com/jdx/usage/compare/v3.5.4..v3.5.5) - 2026-07-14

### 🐛 Bug Fixes

- **(parse)** allow hyphen-prefixed flag values by [@jdx](https://github.com/jdx) in [#715](https://github.com/jdx/usage/pull/715)

### 🧪 Testing

- invoke typescript compiler through npx package by [@jdx](https://github.com/jdx) in [#719](https://github.com/jdx/usage/pull/719)

## [3.5.4](https://github.com/jdx/usage/compare/v3.5.3..v3.5.4) - 2026-07-06

### 🐛 Bug Fixes

- **(complete)** skip unreadable files in fish shebang completion scan by [@GrantD-ADSK](https://github.com/GrantD-ADSK) in [#707](https://github.com/jdx/usage/pull/707)

### 🔍 Other Changes

- Update sponsor references for jdx.dev by [@jdx](https://github.com/jdx) in [#702](https://github.com/jdx/usage/pull/702)

### 📦️ Dependency Updates

- update rust crate itertools to 0.15 by [@renovate[bot]](https://github.com/renovate[bot]) in [#699](https://github.com/jdx/usage/pull/699)
- update rust crate tera to v2 by [@renovate[bot]](https://github.com/renovate[bot]) in [#705](https://github.com/jdx/usage/pull/705)

### New Contributors

- @GrantD-ADSK made their first contribution in [#707](https://github.com/jdx/usage/pull/707)

## [3.5.3](https://github.com/jdx/usage/compare/v3.5.2..v3.5.3) - 2026-06-23

### 🐛 Bug Fixes

- **(docs)** show negated flags in cli help by [@jdx](https://github.com/jdx) in [#694](https://github.com/jdx/usage/pull/694)
- **(zsh)** preserve options for default completion by [@jdx](https://github.com/jdx) in [#693](https://github.com/jdx/usage/pull/693)

## [3.5.1](https://github.com/jdx/usage/compare/v3.5.0..v3.5.1) - 2026-06-17

### 🐛 Bug Fixes

- **(parse)** dedupe required flag validation errors by [@jdx](https://github.com/jdx) in [#685](https://github.com/jdx/usage/pull/685)
- **(zsh)** isolate generated completion options by [@jdx](https://github.com/jdx) in [#686](https://github.com/jdx/usage/pull/686)
- allow for variadic arguments to capture unknown flags as well by [@rtpg](https://github.com/rtpg) in [#676](https://github.com/jdx/usage/pull/676)

### New Contributors

- @rtpg made their first contribution in [#676](https://github.com/jdx/usage/pull/676)

## [3.5.0](https://github.com/jdx/usage/compare/v3.3.0..v3.5.0) - 2026-06-11

### 🚀 Features

- **(spec)** output KDL multiline strings for descriptions by [@gaojunran](https://github.com/gaojunran) in [#639](https://github.com/jdx/usage/pull/639)
- add sponsors command by [@jdx](https://github.com/jdx) in [#662](https://github.com/jdx/usage/pull/662)
- generate SDK for TypeScript and Python by [@gaojunran](https://github.com/gaojunran) in [#623](https://github.com/jdx/usage/pull/623)

### 🐛 Bug Fixes

- **(nushell)** use caret for invoking cmd in completion script by [@silvanshade](https://github.com/silvanshade) in [#638](https://github.com/jdx/usage/pull/638)
- **(parse)** keep inherited global flags when a subcommand re-declares them as non-global by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#649](https://github.com/jdx/usage/pull/649)
- **(parse)** union orphan aliases when merging a re-declared global flag by [@JamBalaya56562](https://github.com/JamBalaya56562) in [#659](https://github.com/jdx/usage/pull/659)
- **(zsh)** consistently single-quote choice values containing spaces by [@jdx](https://github.com/jdx) in [#635](https://github.com/jdx/usage/pull/635)
- **(zsh)** escape colons in completion insert strings by [@davidolrik](https://github.com/davidolrik) in [#670](https://github.com/jdx/usage/pull/670)
- **(zsh)** show all matches when subcommand names contain `:` by [@zeitlinger](https://github.com/zeitlinger) in [#666](https://github.com/jdx/usage/pull/666)

### 📦️ Dependency Updates

- update rust crate ctor to 0.12 by [@renovate[bot]](https://github.com/renovate[bot]) in [#625](https://github.com/jdx/usage/pull/625)
- update rust crate ctor to v1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#637](https://github.com/jdx/usage/pull/637)

### New Contributors

- @zeitlinger made their first contribution in [#666](https://github.com/jdx/usage/pull/666)
- @davidolrik made their first contribution in [#670](https://github.com/jdx/usage/pull/670)
- @JamBalaya56562 made their first contribution in [#659](https://github.com/jdx/usage/pull/659)
- @silvanshade made their first contribution in [#638](https://github.com/jdx/usage/pull/638)

## [3.3.0](https://github.com/jdx/usage/compare/v3.2.1..v3.3.0) - 2026-05-03

### 🚀 Features

- **(complete)** auto-completion for usage shebang scripts by [@jdx](https://github.com/jdx) in [#620](https://github.com/jdx/usage/pull/620)

## [3.2.1](https://github.com/jdx/usage/compare/v3.2.0..v3.2.1) - 2026-04-22

### 🐛 Bug Fixes

- **(zsh)** escape values without descriptions by [@david-hamilton-glean](https://github.com/david-hamilton-glean) in [#597](https://github.com/jdx/usage/pull/597)
- use CARGO_BIN_EXE_usage if set by [@kybe236](https://github.com/kybe236) in [#568](https://github.com/jdx/usage/pull/568)

### 📦️ Dependency Updates

- update rust crate ctor to 0.9 by [@renovate[bot]](https://github.com/renovate[bot]) in [#577](https://github.com/jdx/usage/pull/577)
- update rust crate ctor to 0.10 by [@renovate[bot]](https://github.com/renovate[bot]) in [#587](https://github.com/jdx/usage/pull/587)

### New Contributors

- @david-hamilton-glean made their first contribution in [#597](https://github.com/jdx/usage/pull/597)
- @kybe236 made their first contribution in [#568](https://github.com/jdx/usage/pull/568)

## [3.2.0](https://github.com/jdx/usage/compare/v3.1.0..v3.2.0) - 2026-03-23

### 🚀 Features

- Support env-backed choices with `choices env=...` by [@mustafa0x](https://github.com/mustafa0x) in [#548](https://github.com/jdx/usage/pull/548)

### 🐛 Bug Fixes

- **(zsh)** escape parentheses and brackets in completion descriptions by [@jdx](https://github.com/jdx) in [#559](https://github.com/jdx/usage/pull/559)

### New Contributors

- @mustafa0x made their first contribution in [#548](https://github.com/jdx/usage/pull/548)

## [3.1.0](https://github.com/jdx/usage/compare/v3.0.0..v3.1.0) - 2026-03-22

### 🚀 Features

- **(cli)** render all doc-related fields in --help output by [@jdx](https://github.com/jdx) in [#554](https://github.com/jdx/usage/pull/554)
- **(cli)** support reading spec from stdin via --file - by [@jdx](https://github.com/jdx) in [#555](https://github.com/jdx/usage/pull/555)

### 🐛 Bug Fixes

- **(zsh)** remove trailing space from completions and add directory slash by [@jdx](https://github.com/jdx) in [#556](https://github.com/jdx/usage/pull/556)
- use field assignment for non-exhaustive Spec in benchmarks by [@jdx](https://github.com/jdx) in [#552](https://github.com/jdx/usage/pull/552)

## [3.0.0](https://github.com/jdx/usage/compare/v2.18.2..v3.0.0) - 2026-03-13

### 🚀 Features

- **(spec)** **breaking** add support for license, before/after help metadata by [@jdx](https://github.com/jdx) in [#542](https://github.com/jdx/usage/pull/542)

### 📦️ Dependency Updates

- update rust crate roff to v1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#529](https://github.com/jdx/usage/pull/529)

## [2.18.2](https://github.com/jdx/usage/compare/v2.18.1..v2.18.2) - 2026-03-01

### 🐛 Bug Fixes

- **(bash,zsh)** use >| to avoid noclobber errors in tab completion by [@nkakouros](https://github.com/nkakouros) in [#524](https://github.com/jdx/usage/pull/524)

### 📦️ Dependency Updates

- update rust crate strum to 0.28 by [@renovate[bot]](https://github.com/renovate[bot]) in [#522](https://github.com/jdx/usage/pull/522)

### New Contributors

- @nkakouros made their first contribution in [#524](https://github.com/jdx/usage/pull/524)

## [2.18.1](https://github.com/jdx/usage/compare/v2.18.0..v2.18.1) - 2026-02-24

### 🐛 Bug Fixes

- **(lib)** validate choices for variadic args and flags by [@jdx](https://github.com/jdx) in [#520](https://github.com/jdx/usage/pull/520)

## [2.17.0](https://github.com/jdx/usage/compare/v2.16.2..v2.17.0) - 2026-02-16

### 🚀 Features

- Add support for nushell by [@abusch](https://github.com/abusch) in [#485](https://github.com/jdx/usage/pull/485)

### New Contributors

- @abusch made their first contribution in [#485](https://github.com/jdx/usage/pull/485)

## [2.16.2](https://github.com/jdx/usage/compare/v2.16.1..v2.16.2) - 2026-02-12

### 🐛 Bug Fixes

- **(lib)** add missing child node support to arg parser by [@jdx](https://github.com/jdx) in [#489](https://github.com/jdx/usage/pull/489)

## [2.16.1](https://github.com/jdx/usage/compare/v2.16.0..v2.16.1) - 2026-01-31

### 🐛 Bug Fixes

- **(parse)** handle variadic ellipsis inside brackets like [args...] by [@jdx](https://github.com/jdx) in [#481](https://github.com/jdx/usage/pull/481)

### 📦️ Dependency Updates

- update rust crate criterion to 0.8 by [@renovate[bot]](https://github.com/renovate[bot]) in [#475](https://github.com/jdx/usage/pull/475)

## [2.16.0](https://github.com/jdx/usage/compare/v2.15.1..v2.16.0) - 2026-01-29

### 🚀 Features

- **(windows)** add Windows binaries and fix completion support by [@jdx](https://github.com/jdx) in [#472](https://github.com/jdx/usage/pull/472)

## [2.15.1](https://github.com/jdx/usage/compare/v2.15.0..v2.15.1) - 2026-01-28

### 🐛 Bug Fixes

- **(parse)** handle nested subcommands after default_subcommand switch by [@jdx](https://github.com/jdx) in [#469](https://github.com/jdx/usage/pull/469)

## [2.15.0](https://github.com/jdx/usage/compare/v2.14.0..v2.15.0) - 2026-01-26

### 🚀 Features

- **(parse)** add Parser builder for custom env var handling by [@jdx](https://github.com/jdx) in [#464](https://github.com/jdx/usage/pull/464)

### 🧪 Testing

- **(cli)** use fish output format for cleaner assertions by [@ilyagr](https://github.com/ilyagr) in [#461](https://github.com/jdx/usage/pull/461)

## [2.14.0](https://github.com/jdx/usage/compare/v2.13.1..v2.14.0) - 2026-01-26

### 🚀 Features

- **(lint)** add more lint checks by [@jdx](https://github.com/jdx) in [#446](https://github.com/jdx/usage/pull/446)
- add missing builder methods by [@jdx](https://github.com/jdx) in [#444](https://github.com/jdx/usage/pull/444)

### 🐛 Bug Fixes

- replace unwrap calls with proper error handling in fig.rs by [@jdx](https://github.com/jdx) in [#454](https://github.com/jdx/usage/pull/454)
- improve error messages with more context by [@jdx](https://github.com/jdx) in [#449](https://github.com/jdx/usage/pull/449)
- skip powershell test if pwsh is not installed by [@jdx](https://github.com/jdx) in [#457](https://github.com/jdx/usage/pull/457)
- match completion prefix against unescaped names by [@ilyagr](https://github.com/ilyagr) in [#460](https://github.com/jdx/usage/pull/460)

### 🚜 Refactor

- simplify Spec::merge with local macros by [@jdx](https://github.com/jdx) in [#451](https://github.com/jdx/usage/pull/451)

### 📚 Documentation

- escape generic type parameters in macro doc comments by [@jdx](https://github.com/jdx) in [#453](https://github.com/jdx/usage/pull/453)
- add rustdoc for public API functions by [@jdx](https://github.com/jdx) in [#450](https://github.com/jdx/usage/pull/450)
- add documentation to public API structs by [@jdx](https://github.com/jdx) in [#455](https://github.com/jdx/usage/pull/455)

### ⚡ Performance

- remove unnecessary clone in set_subcommand_ancestors by [@jdx](https://github.com/jdx) in [#448](https://github.com/jdx/usage/pull/448)

### 🧪 Testing

- add test coverage for untested modules by [@jdx](https://github.com/jdx) in [#447](https://github.com/jdx/usage/pull/447)

### 🔍 Other Changes

- remove commented-out trait implementations in mount.rs by [@jdx](https://github.com/jdx) in [#445](https://github.com/jdx/usage/pull/445)

### New Contributors

- @ilyagr made their first contribution in [#460](https://github.com/jdx/usage/pull/460)

## [2.13.1](https://github.com/jdx/usage/compare/v2.13.0..v2.13.1) - 2026-01-19

### 🐛 Bug Fixes

- use correct PowerShell casing in enum variant by [@jdx](https://github.com/jdx) in [#438](https://github.com/jdx/usage/pull/438)

## [2.13.0](https://github.com/jdx/usage/compare/v2.12.0..v2.13.0) - 2026-01-19

### 🚀 Features

- add spec lint command by [@jdx](https://github.com/jdx) in [#430](https://github.com/jdx/usage/pull/430)
- add PowerShell completion support by [@jdx](https://github.com/jdx) in [#431](https://github.com/jdx/usage/pull/431)

### 🐛 Bug Fixes

- replace unsafe path unwrap chains with proper error handling by [@jdx](https://github.com/jdx) in [#424](https://github.com/jdx/usage/pull/424)
- pass positional args through to executed scripts by [@jdx](https://github.com/jdx) in [#425](https://github.com/jdx/usage/pull/425)
- replace unimplemented!() with proper errors for unsupported shells by [@jdx](https://github.com/jdx) in [#432](https://github.com/jdx/usage/pull/432)

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

## [2.12.0](https://github.com/jdx/usage/compare/v2.11.0..v2.12.0) - 2026-01-14

### 🚀 Features

- Allowing preserving double dashes for variadic args by [@alcroito](https://github.com/alcroito) in [#417](https://github.com/jdx/usage/pull/417)

### New Contributors

- @alcroito made their first contribution in [#417](https://github.com/jdx/usage/pull/417)

## [2.11.0](https://github.com/jdx/usage/compare/v2.10.0..v2.11.0) - 2025-12-31

### 🚀 Features

- add default_subcommand and restart_token for naked task completions by [@jdx](https://github.com/jdx) in [#410](https://github.com/jdx/usage/pull/410)

### 🐛 Bug Fixes

- handle --help flag in exec command for non-shell scripts by [@jdx](https://github.com/jdx) in [#409](https://github.com/jdx/usage/pull/409)

### 🧪 Testing

- add non-shell script tests by [@muzimuzhi](https://github.com/muzimuzhi) in [#406](https://github.com/jdx/usage/pull/406)

## [2.10.0](https://github.com/jdx/usage/compare/v2.9.0..v2.10.0) - 2025-12-19

### 🚀 Features

- add variadic argument improvements and builder API by [@jdx](https://github.com/jdx) in [#401](https://github.com/jdx/usage/pull/401)

### 🐛 Bug Fixes

- unhide exec command and fix docs shebang for non-shell scripts by [@jdx](https://github.com/jdx) in [#402](https://github.com/jdx/usage/pull/402)

## [2.9.0](https://github.com/jdx/usage/compare/v2.8.0..v2.9.0) - 2025-12-03

### 🚀 Features

- Support `Vec<String>` for default values of variadic flags by [@iamkroot](https://github.com/iamkroot) in [#388](https://github.com/jdx/usage/pull/388)

### 🐛 Bug Fixes

- treat count flags as repeatable by [@frederikb](https://github.com/frederikb) in [#383](https://github.com/jdx/usage/pull/383)

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

### New Contributors

- @gaojunran made their first contribution in [#363](https://github.com/jdx/usage/pull/363)

## [2.5.1](https://github.com/jdx/usage/compare/v2.5.0..v2.5.1) - 2025-10-26

### 🐛 Bug Fixes

- pass global flags to mount commands during completion by [@jdx](https://github.com/jdx) in [#354](https://github.com/jdx/usage/pull/354)

### 🧪 Testing

- add comprehensive test for default="" behavior by [@jdx](https://github.com/jdx) in [#357](https://github.com/jdx/usage/pull/357)

## [2.5.0](https://github.com/jdx/usage/compare/v2.4.0..v2.5.0) - 2025-10-25

### 🚀 Features

- Print default values if specified by [@iamkroot](https://github.com/iamkroot) in [#350](https://github.com/jdx/usage/pull/350)

### 🐛 Bug Fixes

- add fallback for shell by [@MeanderingProgrammer](https://github.com/MeanderingProgrammer) in [#347](https://github.com/jdx/usage/pull/347)
- complete descriptions serialized as string instead of bool by [@iamkroot](https://github.com/iamkroot) in [#349](https://github.com/jdx/usage/pull/349)

### 📦️ Dependency Updates

- update rust crate ctor to 0.6 by [@renovate[bot]](https://github.com/renovate[bot]) in [#352](https://github.com/jdx/usage/pull/352)

### New Contributors

- @iamkroot made their first contribution in [#350](https://github.com/jdx/usage/pull/350)

## [2.4.0](https://github.com/jdx/usage/compare/v2.3.2..v2.4.0) - 2025-10-21

### 🚀 Features

- add env attribute support for flags by [@jdx](https://github.com/jdx) in [#336](https://github.com/jdx/usage/pull/336)
- add env attribute support for args by [@jdx](https://github.com/jdx) in [#346](https://github.com/jdx/usage/pull/346)

### 🐛 Bug Fixes

- handle colons in zsh completions without description by [@MeanderingProgrammer](https://github.com/MeanderingProgrammer) in [#341](https://github.com/jdx/usage/pull/341)

### New Contributors

- @MeanderingProgrammer made their first contribution in [#341](https://github.com/jdx/usage/pull/341)

## [2.3.2](https://github.com/jdx/usage/compare/v2.3.1..v2.3.2) - 2025-09-29

### 🐛 Bug Fixes

- **(zsh)** compdef ordering by [@jdx](https://github.com/jdx) in [#335](https://github.com/jdx/usage/pull/335)

## [2.3.1](https://github.com/jdx/usage/compare/v2.3.0..v2.3.1) - 2025-09-28

### 🐛 Bug Fixes

- issues with very large specs by [@jdx](https://github.com/jdx) in [#330](https://github.com/jdx/usage/pull/330)

## [2.3.0](https://github.com/jdx/usage/compare/v2.2.2..v2.3.0) - 2025-09-28

### 🚀 Features

- add @generated comments to all generators by [@jdx](https://github.com/jdx) in [#310](https://github.com/jdx/usage/pull/310)

### 🐛 Bug Fixes

- **(completions)** ignore aliases and functions named usage (2nd attempt) by [@risu729](https://github.com/risu729) in [#304](https://github.com/jdx/usage/pull/304)
- use temp files to avoid 'argument list too long' error in shell completions by [@jdx](https://github.com/jdx) in [#329](https://github.com/jdx/usage/pull/329)

### 📦️ Dependency Updates

- update rust crate ctor to 0.5 by [@renovate[bot]](https://github.com/renovate[bot]) in [#327](https://github.com/jdx/usage/pull/327)

## [2.2.2](https://github.com/jdx/usage/compare/v2.2.1..v2.2.2) - 2025-07-16

### ◀️ Revert

- Revert "fix(completions): ignore aliases and functions named usage" by [@jdx](https://github.com/jdx) in [#301](https://github.com/jdx/usage/pull/301)

## [2.2.1](https://github.com/jdx/usage/compare/v2.2.0..v2.2.1) - 2025-07-16

### 🐛 Bug Fixes

- **(completions)** ignore aliases and functions named usage by [@risu729](https://github.com/risu729) in [#300](https://github.com/jdx/usage/pull/300)

## [2.2.0](https://github.com/jdx/usage/compare/v2.1.1..v2.2.0) - 2025-07-11

### 🚀 Features

- Generalize bash command to support bash/zsh/fish by [@NorthIsUp](https://github.com/NorthIsUp) in [#280](https://github.com/jdx/usage/pull/280)

### 🐛 Bug Fixes

- fall back to listing files on unknown completions by [@jdx](https://github.com/jdx) in [#296](https://github.com/jdx/usage/pull/296)

### 🔍 Other Changes

- clippy by [@jdx](https://github.com/jdx) in [f6d5e38](https://github.com/jdx/usage/commit/f6d5e381d902574ad2a9ebf8366bcdfa17098593)

### New Contributors

- @NorthIsUp made their first contribution in [#280](https://github.com/jdx/usage/pull/280)

## [2.1.0](https://github.com/jdx/usage/compare/v2.0.7..v2.1.0) - 2025-04-26

### 🚀 Features

- use ellipsis character by [@jdx](https://github.com/jdx) in [#269](https://github.com/jdx/usage/pull/269)

## [2.0.7](https://github.com/jdx/usage/compare/v2.0.6..v2.0.7) - 2025-03-24

### 🐛 Bug Fixes

- implement short flag chaining and update flag handling logic by [@aroemen](https://github.com/aroemen) in [#258](https://github.com/jdx/usage/pull/258)

### 🔍 Other Changes

- updated deps by [@jdx](https://github.com/jdx) in [7a498e6](https://github.com/jdx/usage/commit/7a498e60e90420af8bec0e97ddbc9f69fdbcd8d5)

### New Contributors

- @aroemen made their first contribution in [#258](https://github.com/jdx/usage/pull/258)

## [2.0.6](https://github.com/jdx/usage/compare/v2.0.5..v2.0.6) - 2025-03-18

### 🐛 Bug Fixes

- **(lib)** make ParseValue cloneable by [@risu729](https://github.com/risu729) in [#252](https://github.com/jdx/usage/pull/252)

### New Contributors

- @risu729 made their first contribution in [#252](https://github.com/jdx/usage/pull/252)

## [2.0.5](https://github.com/jdx/usage/compare/v2.0.4..v2.0.5) - 2025-02-16

### 🐛 Bug Fixes

- 2 bugs with flags and var=#true by [@jdx](https://github.com/jdx) in [#235](https://github.com/jdx/usage/pull/235)

## [2.0.3](https://github.com/jdx/usage/compare/v2.0.0..v2.0.3) - 2025-01-10

### 🐛 Bug Fixes

- add v1-fallback for kdl by [@jdx](https://github.com/jdx) in [9516e15](https://github.com/jdx/usage/commit/9516e15d53c0769a1227ec4ab37e0622b4e7bead)

### ◀️ Revert

- Revert "fix: add v1-fallback for kdl" by [@jdx](https://github.com/jdx) in [ef98628](https://github.com/jdx/usage/commit/ef98628658cb3adcc3284aa341b70329743fa3da)
- Revert "chore: attempt to fix kdl v1-fallback" by [@jdx](https://github.com/jdx) in [c440c2a](https://github.com/jdx/usage/commit/c440c2a4fb843da0670b72f0b6c233602d7c9066)

### 🔍 Other Changes

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

- upgraded itertools by [@jdx](https://github.com/jdx) in [b3cb03a](https://github.com/jdx/usage/commit/b3cb03a5319e22672ff1e87500b861f7af47b157)

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
- added completes to cmds by [@jdx](https://github.com/jdx) in [f421d9e](https://github.com/jdx/usage/commit/f421d9e5b8a88eae70914ff0be44bee824dc0aa1)

## [1.3.5](https://github.com/jdx/usage/compare/v1.3.4..v1.3.5) - 2024-12-09

### 🔍 Other Changes

- bump to miette-7 by [@jdx](https://github.com/jdx) in [#21](https://github.com/jdx/usage/pull/21)

## [1.3.4](https://github.com/jdx/usage/compare/v1.3.3..v1.3.4) - 2024-12-03

### 🔍 Other Changes

- added shellcheck for bash completion file by [@jdx](https://github.com/jdx) in [#176](https://github.com/jdx/usage/pull/176)
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

## [1.3.0](https://github.com/jdx/usage/compare/v1.2.0..v1.3.0) - 2024-11-10

### 🚀 Features

- min_usage_version by [@jdx](https://github.com/jdx) in [#166](https://github.com/jdx/usage/pull/166)

### 🐛 Bug Fixes

- **(fig)** better generate spec for fig mount commands by [@miguelmig](https://github.com/miguelmig) in [#165](https://github.com/jdx/usage/pull/165)
- completions for bins with dashes by [@jdx](https://github.com/jdx) in [adbb347](https://github.com/jdx/usage/commit/adbb3478b86a4eede4f9812c73fc547f13f00842)
- bash script with snake case escapes by [@jdx](https://github.com/jdx) in [4e5ba4a](https://github.com/jdx/usage/commit/4e5ba4a6fa9d3adfe04c27a24b489c15af94ef69)

## [1.2.0](https://github.com/jdx/usage/compare/v1.1.1..v1.2.0) - 2024-11-05

### 🚀 Features

- added cache-key to generated completions by [@jdx](https://github.com/jdx) in [#159](https://github.com/jdx/usage/pull/159)

### 🐛 Bug Fixes

- require --file or --usage-cmd on `usage g completion` by [@jdx](https://github.com/jdx) in [3cae2ae](https://github.com/jdx/usage/commit/3cae2ae4a1ad6a97358bb49d9d0f3e15c65feb40)

## [1.1.1](https://github.com/jdx/usage/compare/v1.0.1..v1.1.1) - 2024-11-04

### 🚀 Features

- added completions for usage-cli itself by [@jdx](https://github.com/jdx) in [#151](https://github.com/jdx/usage/pull/151)

### 🐛 Bug Fixes

- pass exit codes with `usage bash` and `usage exec` by [@jdx](https://github.com/jdx) in [#152](https://github.com/jdx/usage/pull/152)
- tweaks to fig completions by [@jdx](https://github.com/jdx) in [#153](https://github.com/jdx/usage/pull/153)

### 🔍 Other Changes

- Add fig generate completion subcommand by [@miguelmig](https://github.com/miguelmig) in [#148](https://github.com/jdx/usage/pull/148)
- fix cli assets by [@jdx](https://github.com/jdx) in [ab8c6a0](https://github.com/jdx/usage/commit/ab8c6a0a14af1d4ec829660183ec58605afa33c7)

## [1.0.1](https://github.com/jdx/usage/compare/v1.0.0..v1.0.1) - 2024-10-31

### 🐛 Bug Fixes

- allow calling `usage g completion -f` by [@jdx](https://github.com/jdx) in [#143](https://github.com/jdx/usage/pull/143)

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

## [0.11.0](https://github.com/jdx/usage/compare/v0.10.0..v0.11.0) - 2024-10-14

### 🚀 Features

- support single quotes in zsh descriptions by [@jasisk](https://github.com/jasisk) in [#128](https://github.com/jdx/usage/pull/128)
- render help in cli parsing by [@jdx](https://github.com/jdx) in [7c49fcb](https://github.com/jdx/usage/commit/7c49fcba4567da7ad8c7af9c4bb72a7c276a4a57)
- implemented more cli help for args/flags/subcommands by [@jdx](https://github.com/jdx) in [669f44e](https://github.com/jdx/usage/commit/669f44ea0459f997444c46ebfac1f42c00e210b4)

### 🐛 Bug Fixes

- bug with help and args by [@jdx](https://github.com/jdx) in [6c615f9](https://github.com/jdx/usage/commit/6c615f9f8b1c6798fcba3ed88890b2891505c6ec)
- allow building without docs feature by [@jdx](https://github.com/jdx) in [212f96c](https://github.com/jdx/usage/commit/212f96ccb118f393ed6d5141996e02ec3e3630d9)

### 🔍 Other Changes

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

## [0.8.4](https://github.com/jdx/usage/compare/v0.8.3..v0.8.4) - 2024-09-29

### 🐛 Bug Fixes

- capitalize ARGS/FLAGS in md docs by [@jdx](https://github.com/jdx) in [3a314d5](https://github.com/jdx/usage/commit/3a314d5bcb7a1552a4cf2e833bd81b35a7e9e514)
- move usage out of header by [@jdx](https://github.com/jdx) in [9a43a72](https://github.com/jdx/usage/commit/9a43a72ae26606cc9c03ee718627c1a6636d77f2)

## [0.8.3](https://github.com/jdx/usage/compare/v0.8.2..v0.8.3) - 2024-09-28

### 🐛 Bug Fixes

- minor whitespace bug in md output by [@jdx](https://github.com/jdx) in [dcced73](https://github.com/jdx/usage/commit/dcced7300a3abfd2cde2eee2879d27fa30b50694)
- added aliases to command info by [@jdx](https://github.com/jdx) in [ac745d6](https://github.com/jdx/usage/commit/ac745d66215566500faa684b93192392bf307521)
- tweak usage output by [@jdx](https://github.com/jdx) in [c488b76](https://github.com/jdx/usage/commit/c488b76249c6ab6eb022cc022567faed82332074)
- make html_encode optional by [@jdx](https://github.com/jdx) in [cc629ee](https://github.com/jdx/usage/commit/cc629ee36acbbd2fe9a4e69c4b3216334f356739)

## [0.8.2](https://github.com/jdx/usage/compare/v0.8.1..v0.8.2) - 2024-09-28

### 🐛 Bug Fixes

- whitespace in md generation by [@jdx](https://github.com/jdx) in [3cb7769](https://github.com/jdx/usage/commit/3cb776920cd9bd18693cdc0e547b98b0efd25aca)
- escape html in md by [@jdx](https://github.com/jdx) in [a691143](https://github.com/jdx/usage/commit/a6911436156c15246c69ea66e62e2745e419b813)
- more work on html encoding md by [@jdx](https://github.com/jdx) in [b5cb342](https://github.com/jdx/usage/commit/b5cb342fa79ac70bd2723c026f3184021e5ae3ac)

## [0.8.1](https://github.com/jdx/usage/compare/v0.8.0..v0.8.1) - 2024-09-28

### 🐛 Bug Fixes

- improving md generation by [@jdx](https://github.com/jdx) in [#117](https://github.com/jdx/usage/pull/117)

## [0.8.0](https://github.com/jdx/usage/compare/v0.7.4..v0.8.0) - 2024-09-27

### 🚀 Features

- basic support for markdown generation in lib by [@jdx](https://github.com/jdx) in [de004c8](https://github.com/jdx/usage/commit/de004c87890bda993288503fe49e02b342c72487)

## [0.7.1](https://github.com/jdx/usage/compare/v0.7.0..v0.7.1) - 2024-09-27

### 🐛 Bug Fixes

- fail parsing if required args/flags not found by [@jdx](https://github.com/jdx) in [409145a](https://github.com/jdx/usage/commit/409145ae5db937bffa121e63f00f8f827c49b294)

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

## [0.3.1](https://github.com/jdx/usage/compare/v0.3.0..v0.3.1) - 2024-08-28

### 🐛 Bug Fixes

- make shebang scripts work with comments by [@jdx](https://github.com/jdx) in [9eb2a64](https://github.com/jdx/usage/commit/9eb2a64ff0e3c463f53fe0c283bbb932e5b3dd77)

## [0.3.0](https://github.com/jdx/usage/compare/v0.2.1..v0.3.0) - 2024-05-26

### 🚀 Features

- complete descriptions by [@jdx](https://github.com/jdx) in [a8afca7](https://github.com/jdx/usage/commit/a8afca7d6ad773431acfde8280e9dfb2884ef4e0)

## [0.2.1](https://github.com/jdx/usage/compare/v0.2.0..v0.2.1) - 2024-05-25

### 🔍 Other Changes

- updated deps by [@jdx](https://github.com/jdx) in [a457da9](https://github.com/jdx/usage/commit/a457da9ccec4890d63f3ab8e2215e51e64fd2425)

### 📦️ Dependency Updates

- update rust crate xx to v1 by [@renovate[bot]](https://github.com/renovate[bot]) in [#64](https://github.com/jdx/usage/pull/64)

## [0.2.0](https://github.com/jdx/usage/compare/v0.1.18..v0.2.0) - 2024-05-12

### 🚀 Features

- **(exec)** added `usage exec` command by [@jdx](https://github.com/jdx) in [#51](https://github.com/jdx/usage/pull/51)

### 🐛 Bug Fixes

- rust beta warning by [@jdx](https://github.com/jdx) in [8ba775e](https://github.com/jdx/usage/commit/8ba775e02daef37193fa0f43d59f4a4ad3081056)

### 🚜 Refactor

- created reusuable CLI parse function by [@jdx](https://github.com/jdx) in [8bc895a](https://github.com/jdx/usage/commit/8bc895a02ba6c7df32d47d0847b5b1985a2dbfdb)

### 📚 Documentation

- set GA by [@jdx](https://github.com/jdx) in [1a786c3](https://github.com/jdx/usage/commit/1a786c354a6e3f147453d8e6f38fb3916d21f889)

### 🔍 Other Changes

- bump xx by [@jdx](https://github.com/jdx) in [c1bb0bb](https://github.com/jdx/usage/commit/c1bb0bb1c7600cf1ccb788c2d17651f6e93adf01)

### 📦️ Dependency Updates

- update rust crate xx to 0.3 by [@renovate[bot]](https://github.com/renovate[bot]) in [#59](https://github.com/jdx/usage/pull/59)

## [0.1.17](https://github.com/jdx/usage/compare/v0.1.16..v0.1.17) - 2024-03-17

### 🔍 Other Changes

- bump release by [@jdx](https://github.com/jdx) in [3fa016a](https://github.com/jdx/usage/commit/3fa016a266753e9e5ebeb81eed61c74ced46e5cb)

## [0.1.16](https://github.com/jdx/usage/compare/v0.1.9..v0.1.16) - 2024-03-17

### 🚜 Refactor

- move usage-lib into its own dir by [@jdx](https://github.com/jdx) in [37e2379](https://github.com/jdx/usage/commit/37e2379122f123a85c4888e6efa1f62c631ac013)

### 🔍 Other Changes

- added author field by [@jdx](https://github.com/jdx) in [b0e815a](https://github.com/jdx/usage/commit/b0e815a72bf4bfad6659a909a058cd86b7f9d56d)
- fixing cargo metadata by [@jdx](https://github.com/jdx) in [64f19d7](https://github.com/jdx/usage/commit/64f19d7d40de0f897ccd22c07cd72e74b98b435f)
- bump version to try another release by [@jdx](https://github.com/jdx) in [badf251](https://github.com/jdx/usage/commit/badf251feb7fe86d763e4458261060b81f85fe7e)
- set metadata for usage-lib dependency by [@jdx](https://github.com/jdx) in [7e3538a](https://github.com/jdx/usage/commit/7e3538a304372c8d010386e22d39c02c9319d297)
- bump version to try another release by [@jdx](https://github.com/jdx) in [032f686](https://github.com/jdx/usage/commit/032f6860f569874e8ca2928f7db367191a8e69b3)
- bump release by [@jdx](https://github.com/jdx) in [4f3e3ea](https://github.com/jdx/usage/commit/4f3e3ea284968006e677402bd78afd3c592698b4)
- bump release by [@jdx](https://github.com/jdx) in [58be1c4](https://github.com/jdx/usage/commit/58be1c40f45fa86d1d8c6c6e58cbec85451c0d40)
- bump release by [@jdx](https://github.com/jdx) in [cd92e36](https://github.com/jdx/usage/commit/cd92e366ee60d9ea2cc6b43f9dadc7f27c0dd63e)

### 📦️ Dependency Updates

- update rust crate heck to v0.5.0 by [@renovate[bot]](https://github.com/renovate[bot]) in [#30](https://github.com/jdx/usage/pull/30)

## [0.1.8](https://github.com/jdx/usage/compare/v0.1.7..v0.1.8) - 2024-02-10

### 🐛 Bug Fixes

- fix binstall by [@jdx](https://github.com/jdx) in [a3b4513](https://github.com/jdx/usage/commit/a3b45132dd4b9f6b4d7a1ae224de455f28de75dd)

<!-- generated by git-cliff -->
