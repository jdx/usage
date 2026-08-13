//! The generated registry, compiled and resolved against.
//!
//! A generator can be tested by comparing strings, and a string that looks like Rust is not Rust.
//! So the output of `usage-config-build` for [the fixture spec](fixtures/hk.usage.kdl) is checked
//! in beside this file and included here: it is compiled by `cargo test`, and every assertion
//! below is against the registry that compilation produced.

use std::collections::BTreeMap;

use usage_config::{
    resolve, FileLayer, FileScope, Layers, Merge, Parser, Scope, SourceKind, Ty, Value,
};

include!("golden/settings.rs");

#[test]
fn every_declared_default_resolves_as_its_type() {
    // No layers at all: what a CLI gets from its registry alone, which is also the only thing a
    // `const` default can be wrong about.
    let resolved = resolve(SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    let mut fold = resolved.fold();

    let jobs: Option<u64> = fold.required(prop::JOBS);
    let stash: Option<String> = fold.required(prop::STASH);
    let trusted: Option<bool> = fold.required(prop::TRUSTED);
    let output: Option<String> = fold.required(prop::TASK_OUTPUT);
    let ports: Option<Vec<u64>> = fold.required(prop::PORTS);
    let timeout: Option<String> = fold.optional(prop::TIMEOUT);
    fold.finish().expect("every default fits its type");

    assert_eq!(jobs, Some(4));
    assert_eq!(stash, Some("git".to_string()));
    assert_eq!(trusted, Some(false));
    assert_eq!(output, Some("prefix".to_string()));
    // A list default is a child node in the spec and stays typed all the way here: three numbers,
    // not three strings.
    assert_eq!(ports, Some(vec![80, 443]));
    // No default and an `option<duration>`: absent is a state, not a failure.
    assert_eq!(timeout, None);
}

#[test]
fn a_generated_id_is_the_setting_it_names() {
    // The one thing emitting ids as indices can get wrong, and it would be silent: every read
    // would answer about the wrong setting. So each const is checked against the name it claims.
    for id in SETTINGS_REGISTRY.ids() {
        let key = SETTINGS_REGISTRY.get(id).key;
        // `lookup_exact`, not `lookup`: the folding one answers about the setting that *replaced*
        // an old name, which is right for reading a value and wrong for asking where a name lives.
        assert_eq!(
            SETTINGS_REGISTRY.lookup_exact(key),
            Some(id),
            "`{key}` is not where the registry says it is"
        );
    }
    assert_eq!(SETTINGS_REGISTRY.get(prop::JOBS).key, "jobs");
    assert_eq!(SETTINGS_REGISTRY.get(prop::TASK_OUTPUT).key, "task.output");
    assert_eq!(
        SETTINGS_REGISTRY.get(prop::URL_REPLACEMENTS).key,
        "url_replacements"
    );
}

#[test]
fn what_the_spec_said_about_each_setting_is_what_the_registry_holds() {
    let meta = |id| SETTINGS_REGISTRY.get(id);

    // Types, including the two that are not a name: a union is `Any`, because the spec has said
    // usage cannot decide what belongs there.
    assert_eq!(meta(prop::JOBS).ty, Ty::Uint);
    assert_eq!(meta(prop::EXCLUDE).ty, Ty::List(&Ty::String));
    assert_eq!(meta(prop::PATH).ty, Ty::List(&Ty::Path));
    assert_eq!(meta(prop::URL_REPLACEMENTS).ty, Ty::Map(&Ty::String));
    assert_eq!(meta(prop::TIMEOUT).ty, Ty::Option(&Ty::Duration));
    assert_eq!(meta(prop::EITHER).ty, Ty::Any);

    // Merge policy, scope, and the named parser: each of these is a `prop` attribute that a
    // hand-written registry has to remember to copy, and every one of them is a real setting in
    // hk's own file.
    assert_eq!(meta(prop::EXCLUDE).merge, Merge::Union);
    assert_eq!(meta(prop::URL_REPLACEMENTS).merge, Merge::Deep);
    assert_eq!(meta(prop::JOBS).merge, Merge::Replace);
    assert_eq!(meta(prop::TRUSTED).scope, Scope::Global);
    assert_eq!(meta(prop::CI).scope, Scope::Env);
    assert_eq!(meta(prop::JOBS).scope, Scope::Any);
    assert_eq!(meta(prop::EXCLUDE).parse, Some(Parser::ListByComma));
    assert_eq!(meta(prop::PATH).parse, Some(Parser::ListByOsPathSeparator));
    assert_eq!(meta(prop::JOBS).parse, None);
    assert!(meta(prop::CI).hide);
    assert!(!meta(prop::JOBS).hide);

    // The environment in precedence order, and help as the spec wrote it — quotes and all.
    assert_eq!(meta(prop::JOBS).envs, &["HK_JOBS", "HK_JOB"]);
    assert_eq!(meta(prop::CI).envs, &["CI"]);
    assert_eq!(meta(prop::JOBS).help, Some("How many jobs to run at once"));
    assert_eq!(
        meta(prop::EITHER).help,
        Some("A \"union\" only hk understands")
    );

    // Bindings, which is the whole mechanism behind hk's git and pkl layers: a custom layer asks
    // the registry for its own kind and iterates what it finds.
    assert_eq!(
        SETTINGS_REGISTRY
            .bindings(SourceKind::new("git"))
            .collect::<Vec<_>>(),
        vec![(prop::JOBS, "hk.jobs")]
    );
    assert_eq!(
        SETTINGS_REGISTRY
            .bindings(SourceKind::new("pkl"))
            .collect::<Vec<_>>(),
        vec![
            (prop::EXCLUDE, "exclude"),
            (prop::EXCLUDE, "defaults.exclude")
        ]
    );
}

#[test]
fn a_renamed_setting_folds_into_the_one_that_replaced_it() {
    let resolved = resolve(SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    // The old name still answers, about the setting that replaced it.
    assert_eq!(resolved.get_key("concurrency"), Some(&Value::Int(4)));
    assert_eq!(
        SETTINGS_REGISTRY.get(prop::CONCURRENCY).renamed_to,
        Some("jobs")
    );
    let explained = usage_config::explain(&resolved, "concurrency").expect("declared");
    assert!(
        explained.starts_with("concurrency is now jobs\n"),
        "{explained}"
    );
    assert!(
        explained.contains("deprecated: Use jobs instead."),
        "{explained}"
    );
}

#[test]
fn a_file_read_against_the_generated_registry_means_what_the_spec_said() {
    // End to end, which is the point of the whole crate: a spec declared these settings, a build
    // generated the registry, and a config file is read through it with nothing hand-written in
    // between.
    let dir = std::env::temp_dir().join(format!("usage_config_build_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("hk.toml");
    std::fs::write(
        &path,
        "jobs = 8\nexclude = \"target,dist\"\nconcurrency = 2\n\
         [url_replacements]\n\"git@github.com:\" = \"https://github.com/\"\n",
    )
    .expect("write");

    let layer = FileLayer::at(&path, FileScope::Project);
    let resolved = resolve(SETTINGS_REGISTRY, Layers::new().then(&layer)).expect("resolves");
    let mut fold = resolved.fold();
    let jobs: Option<u64> = fold.required(prop::JOBS);
    let exclude: Option<Vec<String>> = fold.required(prop::EXCLUDE);
    let replacements: Option<BTreeMap<String, String>> = fold.required(prop::URL_REPLACEMENTS);
    fold.finish().expect("reads");

    // The file's value beats the declared default…
    assert_eq!(jobs, Some(8));
    // …the named parser splits what the file wrote as one string…
    assert_eq!(
        exclude,
        Some(vec!["target".to_string(), "dist".to_string()])
    );
    // …a `map` arrives as a table…
    assert_eq!(
        replacements.as_ref().and_then(|m| m.get("git@github.com:")),
        Some(&"https://github.com/".to_string())
    );
    // …and the deprecated key in the same file is folded, with something said about it.
    let warnings = usage_config::explain::warnings(&resolved);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("concurrency is deprecated")),
        "{warnings:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_settings_struct_holds_what_each_type_says() {
    // The struct is generated, so this is the compiler agreeing as much as it is a test: every
    // field below has to exist, with that type, or this file does not build.
    let resolved = resolve(SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    let settings = Settings::read(&resolved).expect("every default fits its field");

    // A declared default means the field is the value itself — there is nothing to unwrap for a
    // setting that always has one.
    assert_eq!(settings.jobs, 4u64);
    assert_eq!(settings.stash, "git");
    assert!(!settings.trusted);
    assert_eq!(settings.ports, vec![80u64, 443]);
    // A dotted key is a nested struct, spelled the way the config file spells it.
    assert_eq!(settings.task.output, "prefix");
    // No default, or `option<…>`: absent is a state the type can hold, so nothing fails at run
    // time for a shape the spec allows.
    assert_eq!(settings.exclude, None);
    assert_eq!(settings.timeout, None);
    assert_eq!(settings.ci, None);
    // A keyword is an unremarkable name for a setting, and `r#` is how Rust spells it — for the
    // field and for the local the generated reader binds it to, which is what `let match: …` taught
    // me the hard way.
    assert_eq!(settings.r#match, "all");
    // A union has no Rust type. The value arrives as it was written, which is what declaring one
    // asked for.
    assert_eq!(settings.either, None);
    // And an old name is not a field at all: `concurrency` folds into `jobs`, so a field for it
    // would be a second name for one value.
    assert_eq!(
        SETTINGS_REGISTRY.get(prop::CONCURRENCY).renamed_to,
        Some("jobs")
    );
}

#[test]
fn a_file_fills_the_struct_and_a_bad_value_names_itself() {
    let dir =
        std::env::temp_dir().join(format!("usage_config_build_struct_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");

    let path = dir.join("hk.toml");
    std::fs::write(
        &path,
        "jobs = 8\nexclude = \"target,dist\"\npath = \"/bin\"\n[task]\noutput = \"interleave\"\n",
    )
    .expect("write");
    let layer = FileLayer::at(&path, FileScope::Project);
    let resolved = resolve(SETTINGS_REGISTRY, Layers::new().then(&layer)).expect("resolves");
    let settings = Settings::read(&resolved).expect("reads");
    assert_eq!(settings.jobs, 8);
    assert_eq!(
        settings.exclude,
        Some(vec!["target".to_string(), "dist".to_string()])
    );
    assert_eq!(
        settings.path,
        Some(vec![std::path::PathBuf::from("/bin")]),
        "a `list<path>` arrives as paths"
    );
    assert_eq!(settings.task.output, "interleave");

    // And the failure a struct read can still have: a hook writing past the declared type, which
    // is the one place the merge did not check. The error names the setting and the hook.
    let mut resolved = resolve(SETTINGS_REGISTRY, Layers::new()).expect("resolves");
    resolved.coerced(prop::JOBS, Value::Int(-1), "one job when raw");
    let err = Settings::read(&resolved).expect_err("a `uint` field cannot hold -1");
    assert_eq!(
        err.to_string(),
        "jobs expected a non-negative integer but has `-1` (set by one job when raw)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
