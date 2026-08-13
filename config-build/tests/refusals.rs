//! What a registry has to be refused for, and the one thing it must stay identical to.
//!
//! A build script is where strictness belongs: the alternative to refusing a declaration that
//! cannot mean what it says is a warning on every run of a shipped binary, for a mistake only the
//! author of the spec can fix.

use usage_config_build::{source, source_of_spec, Error};

/// A spec with `config { … }` around the settings under test.
fn spec(settings: &str) -> String {
    format!("name \"mycli\"\nbin \"mycli\"\nconfig {{\n{settings}\n}}\n")
}

/// Every problem a spec's settings are refused for.
fn problems(settings: &str) -> Vec<String> {
    match source_of_spec(&spec(settings), "mycli.usage.kdl") {
        Ok(generated) => panic!("should have been refused, generated:\n{generated}"),
        Err(Error::Registry(problems)) => problems,
        Err(other) => panic!("refused for the wrong reason: {other}"),
    }
}

#[test]
fn the_checked_in_registry_is_what_the_generator_produces() {
    // The generated file is checked in so that `cargo test` compiles it — which is the only way to
    // know the generator emits code rather than plausible text. That only works while the two
    // agree, so this is the test that fails when a change to the emitter is not regenerated:
    //
    //   cargo run -p usage-config-build --example gen > config-build/tests/golden/settings.rs
    let generated = source("tests/fixtures/hk.usage.kdl").expect("the fixture is valid");
    let checked_in = std::fs::read_to_string("tests/golden/settings.rs").expect("golden");
    assert_eq!(
        generated, checked_in,
        "tests/golden/settings.rs is stale; regenerate it with the command in this test"
    );
}

#[test]
fn an_included_spec_is_watched_and_read() {
    // `include` is how a CLI with many settings keeps them in a file of their own, which makes that
    // file the one most likely to be edited — and watching only the file the build script names left
    // editing the settings rebuilding nothing, so the generated registry went stale in silence.
    let watched = usage_config_build::watched("tests/fixtures/split.usage.kdl").expect("parses");
    let names: Vec<String> = watched
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        names,
        vec!["split.usage.kdl", "split-settings.usage.kdl"],
        "the spec and what it included"
    );

    // And the settings in the included file are the ones generated, which is the other half of it.
    let generated = source("tests/fixtures/split.usage.kdl").expect("generates");
    assert!(generated.contains("PropMeta::new(\"jobs\""), "{generated}");
    assert!(generated.contains("SPLIT_JOBS"), "{generated}");
}

#[test]
fn a_rename_to_a_setting_that_is_not_there_is_refused() {
    // The runtime folds an old name into its replacement by looking the replacement up. Naming one
    // that does not exist leaves every value written under the old key silently unread.
    let problems = problems("    prop \"old\" renamed_to=\"new\"\n    prop \"jobs\" type=\"uint\"");
    assert_eq!(
        problems,
        vec!["`old` is renamed to `new`, which is not a setting"]
    );
}

#[test]
fn renames_that_go_round_in_a_circle_are_refused() {
    // A cycle makes `lookup` give up and answer `None`, so both settings become unreachable rather
    // than wrong — the hardest kind of bug to notice, and one only the spec's author can fix.
    let problems = problems(
        "    prop \"a\" renamed_to=\"b\"\n    \
         prop \"b\" renamed_to=\"c\"\n    \
         prop \"c\" renamed_to=\"a\"",
    );
    assert!(
        problems.iter().any(|p| p.contains("form a cycle")),
        "{problems:?}"
    );
    // Named from each end, because the author has to find the loop and any of its names will do.
    assert_eq!(problems.len(), 3, "{problems:?}");
}

#[test]
fn an_old_name_with_a_default_of_its_own_is_refused() {
    // The merge folds a rename to its target, whose default is the one that shows up — so a
    // default on the old name is seeded where nothing reads it. Left to run time this is a
    // warning on every run of a shipped binary.
    let problems = problems(
        "    prop \"concurrency\" type=\"uint\" default=8 renamed_to=\"jobs\"\n    \
         prop \"jobs\" type=\"uint\" default=4",
    );
    assert_eq!(
        problems,
        vec![
            "`concurrency` is renamed to `jobs` and declares a default of its own: put the \
             default on the setting that replaced it"
        ]
    );
}

#[test]
fn an_old_name_with_a_list_default_is_refused_too() {
    // The same mistake in the other spelling. A list default is a child node rather than a value,
    // and returning it early skipped the check above — so this was the one shape of an alias
    // carrying a default that was generated instead of refused.
    let problems = problems(
        "    prop \"old_ports\" type=\"list<uint>\" renamed_to=\"ports\" {\n        \
         default 80 443\n    }\n    \
         prop \"ports\" type=\"list<uint>\"",
    );
    assert_eq!(
        problems,
        vec![
            "`old_ports` is renamed to `ports` and declares a default of its own: put the default \
             on the setting that replaced it"
        ]
    );
}

#[test]
fn a_default_declared_twice_is_refused() {
    // `default=1` beside a `default 80 443` node. The spec takes both — I checked, rather than
    // trusting the comment I had written saying it did not — and there is no reading of a property
    // that has two defaults. Generated, the scalar was dropped and nothing said so.
    assert_eq!(
        problems(
            "    prop \"ports\" type=\"list<uint>\" default=1 {\n        default 80 443\n    }"
        ),
        vec!["`ports` declares a default twice, as `default=` and as a `default` node: keep one"]
    );
}

#[test]
fn a_key_that_cannot_be_a_name_is_refused() {
    // KDL takes `prop ""` perfectly happily, and it generated `pub const : PropId = …` — code the
    // *adopter's* crate fails to compile, in a file they did not write.
    assert_eq!(
        problems("    prop \"\" type=\"string\""),
        vec!["a setting with no name cannot be generated"]
    );
    // And a piece of a dotted key with no name in it: empty,
    assert_eq!(
        problems("    prop \"task..output\" type=\"string\""),
        vec![
            "`task..output` has a part with no name in it: every piece of a dotted key needs an \
             ASCII letter or digit"
        ]
    );
    // or nothing but separators, where every character becomes an underscore. That generates an
    // *anonymous* const — legal Rust, and unreferenceable, so the code naming it does not compile.
    assert_eq!(
        problems("    prop \"-\" type=\"string\""),
        vec![
            "`-` has a part with no name in it: every piece of a dotted key needs an ASCII letter \
             or digit"
        ]
    );
}

#[test]
fn a_key_that_holds_a_newline_stays_on_its_doc_comment() {
    // A doc comment is a line. A key can hold a newline as easily as help text can, and the rest of
    // it then reads as code — in a file the adopter did not write.
    let generated = source_of_spec(
        &spec("    prop \"a\\nb\" type=\"string\""),
        "mycli.usage.kdl",
    )
    .expect("should generate");
    // A doc comment needs no escapes, only a single line: the newline becomes a space.
    assert!(generated.contains("/// `a b`"), "{generated}");
    for line in generated.lines() {
        let trimmed = line.trim_start();
        assert!(
            trimmed.starts_with("///") || trimmed.starts_with("//") || !trimmed.starts_with("b`"),
            "the key ended its own comment:\n{generated}"
        );
    }
}

#[test]
fn a_key_with_no_ascii_in_it_is_refused() {
    // `é` is a letter, and `ident_of` builds names from ASCII — so it became `_`, and the promised
    // `prop::` const was an anonymous one nothing can refer to. Rust would take `É` as an identifier;
    // what an arbitrary letter uppercases to is not something to bet a generated name on.
    assert_eq!(
        problems("    prop \"é\" type=\"string\""),
        vec![
            "`é` has a part with no name in it: every piece of a dotted key needs an ASCII letter \
             or digit"
        ]
    );
}

#[test]
fn a_parser_nothing_implements_is_refused() {
    // Not dropped and not carried: a spec naming a parser that does not exist would have its
    // values split by a rule nobody wrote, so a layer handing over text produces one item where
    // the author meant several.
    let problems = problems("    prop \"paths\" type=\"list<string>\" parse=\"split_on_vibes\"");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("`paths` names the parser `split_on_vibes`, which is not one of: "),
        "{problems:?}"
    );
    // The message lists what there is, because the author's next move is to pick one.
    assert!(problems[0].contains("list_by_comma"), "{problems:?}");
}

#[test]
fn a_table_keyed_by_something_a_file_cannot_spell_is_refused() {
    // Keys in TOML and JSON are text. A `map<int, …>` would be honoured by handing the CLI string
    // keys — quietly, which is the worst way to not support something.
    let problems = problems("    prop \"ports\" type=\"map<int, string>\"");
    assert_eq!(
        problems,
        vec![
            "`ports` is a `map<int, …>`, and a settings table's keys are text: \
             write `map<string, …>`"
        ]
    );
}

#[test]
fn two_keys_that_generate_one_const_are_refused() {
    // `task.output` and `task_output` are two settings and one `prop::TASK_OUTPUT`. Generating
    // both would not compile — a good outcome reported badly, in a file the author did not write.
    let problems = problems(
        "    prop \"task.output\" type=\"string\"\n    prop \"task_output\" type=\"string\"",
    );
    assert_eq!(
        problems,
        vec![
            "`task_output` and `task.output` both generate `prop::TASK_OUTPUT`: rename one of them"
        ]
    );
}

#[test]
fn everything_wrong_is_reported_at_once() {
    // Three mistakes, one build. Reporting the first would make fixing a registry a sequence of
    // builds, which is how the fleet's own generators behave and why nobody enjoys editing them.
    let problems = problems(
        "    prop \"a\" parse=\"nope\"\n    \
         prop \"b\" renamed_to=\"nowhere\"\n    \
         prop \"c\" type=\"map<uint, string>\"",
    );
    assert_eq!(problems.len(), 3, "{problems:?}");
    let all = problems.join("\n");
    assert!(all.contains("`a` names the parser `nope`"), "{all}");
    assert!(all.contains("`b` is renamed to `nowhere`"), "{all}");
    assert!(all.contains("`c` is a `map<uint, …>`"), "{all}");
}

#[test]
fn a_spec_with_no_settings_is_not_an_empty_registry() {
    // A build script that generates an empty registry has been pointed at the wrong file, and the
    // failure it would otherwise produce is "no such setting" for every read in the CLI.
    let err = source_of_spec("name \"mycli\"\nbin \"mycli\"\n", "mycli.usage.kdl")
        .expect_err("should be refused");
    assert!(matches!(err, Error::NoSettings), "{err}");
    assert_eq!(err.to_string(), "this spec declares no `config` settings");
}

#[test]
fn a_spec_that_does_not_parse_says_so_as_the_parser_put_it() {
    // Nothing here re-words a KDL error: the spec's own parser has the line and column.
    let err = source_of_spec("config {\n  prop\n", "mycli.usage.kdl").expect_err("refused");
    assert!(matches!(err, Error::Spec(_)), "{err}");
}

#[test]
fn a_setting_that_is_also_a_group_of_settings_is_refused() {
    // `python` as a setting and `python.compile` as another: one field name with two things to be,
    // a value and a table. The spec can say it; no struct can hold it.
    let problems =
        problems("    prop \"python\" type=\"string\"\n    prop \"python.compile\" type=\"bool\"");
    assert_eq!(
        problems,
        vec![
            "`python` is a setting and a group of settings: it cannot be both a value and a table"
        ]
    );
}

#[test]
fn two_keys_that_generate_one_field_are_refused() {
    // The `prop::` consts do not catch these: `foo-bar.x` and `foo_bar.y` are two distinct consts
    // and one field, because a dash and an underscore are the same character in an identifier. The
    // check has to be on the names that are emitted, not on the segments they came from.
    assert_eq!(
        problems("    prop \"foo-bar.x\" type=\"string\"\n    prop \"foo_bar.y\" type=\"string\""),
        vec![
            "`foo-bar` and `foo_bar` both generate the field `foo_bar`: rename one of them",
            "`foo-bar` and `foo_bar` are both groups named `SettingsFooBar`: rename one of them"
        ]
    );

    // And two groups whose *type* names collide, which a field name does not: a struct name loses
    // the case of its first letter.
    assert_eq!(
        problems("    prop \"a.x\" type=\"string\"\n    prop \"A.y\" type=\"string\""),
        vec!["`A` and `a` are both groups named `SettingsA`: rename one of them"]
    );
}

#[test]
fn two_groups_that_generate_one_struct_are_refused_however_deep_they_are() {
    // Struct names are built by concatenation and all declared in one module, so `http.client` and
    // `http_client` arrive at `SettingsHttpClient` from different depths. Compared among siblings
    // they are not siblings at all, and the adopter's crate got two structs with one name.
    assert_eq!(
        problems(
            "    prop \"http.client.timeout\" type=\"string\"\n    \
             prop \"http_client.retries\" type=\"uint\""
        ),
        vec![
            "`http.client` and `http_client` are both groups named `SettingsHttpClient`: \
             rename one of them"
        ]
    );
}

#[test]
fn a_key_that_needs_escaping_is_escaped_where_it_becomes_a_literal() {
    // `a"b` is a nameable key — it has letters in it — and the generated reader quotes the key into
    // an `expect` message. Interpolated raw, the quote ended that literal early and the rest of the
    // message read as code, in a file the adopter did not write.
    let generated = source_of_spec(
        &spec("    prop \"a\\\"b\" type=\"uint\" default=1"),
        "mycli.usage.kdl",
    )
    .expect("should generate");
    assert!(
        generated.contains("`a\\\"b` has a declared default"),
        "{generated}"
    );
    // The same key in `PropMeta`, which was escaped all along, and in a doc comment, where it needs
    // no escape and only one line.
    assert!(
        generated.contains("PropMeta::new(\"a\\\"b\""),
        "{generated}"
    );
    assert!(generated.contains("/// (`a\"b`)"), "{generated}");
}

#[test]
fn a_setting_named_after_the_readers_own_bindings_is_still_a_setting() {
    // `fold` is what the generated reader calls its fold, and an unprefixed local shadowed it: every
    // read after it, and `fold.finish()`, stopped compiling. Nothing about `fold` is special to a
    // config file, so the fix is on this side — every local is prefixed.
    let generated = source_of_spec(
        &spec("    prop \"fold\" type=\"string\"\n    prop \"resolved\" type=\"bool\""),
        "mycli.usage.kdl",
    )
    .expect("should generate");
    assert!(
        generated.contains("let read_fold: Option<String> ="),
        "{generated}"
    );
    assert!(
        generated.contains("let read_resolved: Option<bool> ="),
        "{generated}"
    );
    // And the fields keep the names the config file uses.
    assert!(
        generated.contains("pub fold: Option<String>,"),
        "{generated}"
    );
}

#[test]
fn a_name_is_built_from_ascii_wherever_it_is_built() {
    // A Rust identifier is not "any alphanumeric character": `½` is one and cannot appear in an
    // identifier, and a Unicode digit cannot start one. The `prop::` consts were ASCII already, so
    // keeping more in the fields made one key produce names from two alphabets — a const the adopter
    // could compile beside a field they could not.
    let generated = source_of_spec(
        &spec("    prop \"caf\u{e9}.si\u{bd}e\" type=\"string\""),
        "mycli.usage.kdl",
    )
    .expect("should generate");
    assert!(
        generated.contains("pub const CAF__SI_E: PropId"),
        "{generated}"
    );
    assert!(generated.contains("pub caf_: SettingsCaf,"), "{generated}");
    assert!(
        generated.contains("pub si_e: Option<String>,"),
        "{generated}"
    );
    // Every line that *declares* a name is ASCII. The key itself is not — it appears as a string
    // literal in `PropMeta`, and as data it is exactly what the spec said.
    for line in generated.lines() {
        let declares = line.trim_start();
        let declares = declares.starts_with("pub const ")
            || declares.starts_with("pub struct ")
            || declares.starts_with("pub fn ")
            || declares.starts_with("let ")
            || (declares.starts_with("pub ") && declares.contains(':'));
        assert!(
            !declares || line.is_ascii(),
            "a generated name kept a character no identifier can hold: {line}"
        );
    }
}

#[test]
fn a_key_that_starts_with_a_digit_is_a_field_all_the_same() {
    // `2fa` is a real name for a real setting, and neither a const nor a field can start with a
    // digit — so both get the same prefix rather than one of them being invalid Rust.
    let generated = source_of_spec(
        &spec("    prop \"2fa.token\" type=\"string\""),
        "mycli.usage.kdl",
    )
    .expect("should generate");
    assert!(
        generated.contains("pub const _2FA_TOKEN: PropId"),
        "{generated}"
    );
    assert!(generated.contains("pub _2fa: Settings2fa,"), "{generated}");
}

#[test]
fn a_key_rust_has_no_spelling_for_is_refused() {
    // Most keywords are fine as fields — `type` is written `r#type`, and settings called that are
    // perfectly ordinary. Four are not: there is no `r#self`, so the field cannot be written at all.
    assert_eq!(
        problems("    prop \"self\" type=\"string\""),
        vec!["`self` cannot be a field: `self` is a keyword Rust has no spelling for"]
    );

    // And the same for a group, whose name becomes a field too.
    assert_eq!(
        problems("    prop \"crate.name\" type=\"string\""),
        vec!["`crate` cannot be a field: `crate` is a keyword Rust has no spelling for"]
    );
}
