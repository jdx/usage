//! Why a setting has the value it has.
//!
//! hk's `config explain` is the best of its kind in the fleet — it names the winning source and
//! the exact identifier, and it does per-item provenance for lists. It also costs about two
//! hundred lines, and it is written against a *second* merge function that exists only to
//! answer this question. Two merges can disagree, and when they do the explanation describes a
//! resolution that never happened.
//!
//! Here there is one merge and its provenance is the answer, so this module is a renderer and
//! nothing more. Every adopter gets the command; none of them writes it.
//!
//! The output is plain text on purpose. A CLI that wants JSON has the same [`Resolved`] this
//! reads and better taste than a library about what its own JSON should look like.

use std::fmt::Write as _;

use crate::registry::{PropId, Registry};
use crate::resolve::Resolved;
use crate::source::SourceKind;
use crate::value::{one_line, shown};

/// One setting explained: what it is, where it came from, and what else tried.
///
/// ```text
/// jobs = 4
///   set by  MISE_JOBS
///   type    uint
///
///   also considered, lowest precedence first:
///     the default
///     mise.toml#jobs
/// ```
///
/// Returns `None` for a key the registry does not have, which the caller reports its own way —
/// a CLI usually wants to suggest a near miss, and this module has no business guessing.
pub fn explain(resolved: &Resolved, key: &str) -> Option<String> {
    let registry = resolved.registry();
    let found = registry.lookup(key)?;
    let meta = registry.get(found.id);
    let mut out = String::new();

    // The name the *user* asked about, then the setting it turned out to be. Printing only the
    // canonical name would answer a question nobody asked.
    if let Some(old) = found.renamed_from {
        let _ = writeln!(out, "{old} is now {}", meta.key);
    }

    match resolved.get(found.id) {
        Some(value) => {
            let _ = writeln!(out, "{} = {}", meta.key, shown(value));
        }
        None => {
            let _ = writeln!(out, "{} is unset", meta.key);
        }
    }

    if let Some(origin) = resolved.origin(found.id) {
        // "set by the default" reads oddly, and a rewrite is not something anybody *set*.
        let verb = match origin.kind {
            SourceKind::DEFAULTS => "default",
            SourceKind::COERCED => "derived",
            _ => "set by",
        };
        let _ = writeln!(out, "  {verb:<8}{}", one_line(origin.describe()));
    }
    // The spec's own spelling, not the prose an error message uses: a reader searching the docs
    // for "a non-negative integer" finds nothing, and `uint` is what the author wrote.
    let _ = writeln!(out, "  {:<8}{}", "type", meta.ty.name());
    // What it will take, when it says. A user reading an explanation because their value was refused
    // needs the list here rather than in a warning they have already scrolled past.
    if !meta.choices.is_empty() {
        let _ = writeln!(out, "  {:<8}{}", "one of", one_line(&meta.allowed()));
    }
    if let Some(help) = meta.help {
        // Through the same helper as everything else: an adopter's help is a doc comment, and a
        // doc comment with a second paragraph in it split one fact into several records.
        let _ = writeln!(out, "  {:<8}{}", "", one_line(help));
    }

    // Everything that contributed, which for a `union` or `deep` setting is how a list assembled
    // from four files is accounted for. The winner is the last of them, so printing it again
    // would be noise — but for anything with more than one contributor, *which* others there
    // were is the question being asked.
    let contributors = resolved.contributors(found.id);
    if contributors.len() > 1 {
        let _ = writeln!(out, "\n  also considered, lowest precedence first:");
        for origin in &contributors[..contributors.len() - 1] {
            // No padding: the column it was aligning for is one this crate cannot fill. A
            // contributor's *value* is not kept — provenance is origins — so the width only ever
            // added trailing spaces to the last thing on the line, and a path copied out of an
            // explanation stopped being the path the merge recorded.
            let _ = writeln!(out, "    {}", one_line(origin.describe()));
        }
    }

    // Where else it *could* come from, which is the other half of the question: a user who does
    // not like the answer needs to know what to change.
    // The blank line opens whichever of these comes first, rather than belonging to the
    // environment: a setting with git or pkl bindings and no environment variables had its
    // `also` line jammed against the type or the help above it, reading as part of them.
    if !meta.envs.is_empty() || !meta.cli.is_empty() || !meta.bindings.is_empty() {
        let _ = writeln!(out);
    }
    if !meta.cli.is_empty() {
        // Before the environment, because it is the way a user is most likely to reach for next: an
        // explanation that listed the variables and not the flag answered half the question.
        let _ = writeln!(out, "  command line {}", one_line(&meta.cli.join(", ")));
    }
    if !meta.envs.is_empty() {
        let _ = writeln!(out, "  environment  {}", one_line(&meta.envs.join(", ")));
    }
    if !meta.bindings.is_empty() {
        let bindings: Vec<String> = meta
            .bindings
            .iter()
            .map(|(kind, key)| format!("{kind} {key}"))
            .collect();
        let _ = writeln!(out, "  also         {}", one_line(&bindings.join(", ")));
    }
    // Starting from the *asked-about* name, because a deprecation notice lives on the old name and
    // reading it off the setting that replaced it printed nothing at all for the one case where it
    // matters — and following the renames from there, because a notice can sit anywhere along a
    // chain: `a` renamed to `b`, and `b` the one carrying the notice that says to use `c`.
    let deprecated = deprecation_along(registry, found.renamed_from.unwrap_or(meta.key));
    if let Some(why) = deprecated {
        let _ = writeln!(out, "\n  deprecated: {}", one_line(why));
    }

    Some(out)
}

/// The first deprecation notice along the rename chain that starts at `key`.
///
/// Bounded by the number of settings there are, so a registry whose renames form a cycle stops
/// rather than following them forever — the same guard [`Registry::lookup`] uses, and for the same
/// reason: this is an authoring mistake, and hanging is a worse way to report one than nothing at
/// all. `usage-config-build` refuses such a registry outright.
fn deprecation_along(registry: Registry, key: &str) -> Option<&'static str> {
    let mut current = registry.lookup_exact(key)?;
    for _ in 0..registry.props.len() {
        let meta = registry.get(current);
        if let Some(why) = meta.deprecated {
            return Some(why);
        }
        current = meta
            .renamed_to
            .and_then(|next| registry.lookup_exact(next))?;
    }
    None
}

/// Every warning the resolution produced, as lines.
///
/// Separate from [`explain`] because they answer different questions and belong in different
/// places: a warning is about the *whole* resolution and a CLI prints it when its logging is up,
/// while an explanation is about one setting somebody asked after.
pub fn warnings(resolved: &Resolved) -> Vec<String> {
    resolved
        .warnings
        .iter()
        .map(|warning| {
            let message = one_line(&warning.message);
            match &warning.origin {
                // The message quotes the value it rejected, which is a value out of a file and
                // can hold anything — including the newline that would hide every warning after
                // this one.
                Some(origin) => format!("{message} ({})", one_line(origin.describe())),
                None => message,
            }
        })
        .collect()
}

/// Every setting and its value, one per line, for a `config ls`.
///
/// Hidden settings are left out — they are settable and documented nowhere, so listing them
/// would be the one place they surface. Sorted by key, because a registry's order is the order
/// somebody wrote a TOML file in and a list a human reads should not depend on that.
pub fn list(resolved: &Resolved) -> Vec<String> {
    let registry = resolved.registry();
    let mut ids: Vec<PropId> = registry
        .ids()
        .filter(|id| !registry.get(*id).hide && registry.get(*id).renamed_to.is_none())
        .collect();
    ids.sort_by_key(|id| registry.get(*id).key);
    ids.into_iter()
        .map(|id| {
            let meta = registry.get(id);
            match resolved.get(id) {
                Some(value) => format!("{} = {}", meta.key, shown(value)),
                None => format!("{} is unset", meta.key),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::{Entry, Layer, LayerCtx, LayerError, LayerOutput};
    use crate::registry::{Merge, PropMeta, Registry, Scope};
    use crate::resolve::{resolve, Layers};
    use crate::source::{FileScope, Origin};
    use crate::ty::Ty;
    use crate::value::{Const, Value};

    static PROPS: &[PropMeta] = &[
        PropMeta {
            default: Some(Const::Int(4)),
            envs: &["HK_JOBS", "HK_JOB"],
            cli: &["--jobs", "-j"],
            bindings: &[("git", "hk.jobs")],
            help: Some("How many jobs to run at once"),
            ..PropMeta::new("jobs", Ty::Uint)
        },
        PropMeta {
            merge: Merge::Union,
            ..PropMeta::new("exclude", Ty::List(&Ty::String))
        },
        PropMeta {
            deprecated: Some("Use jobs instead."),
            renamed_to: Some("jobs"),
            ..PropMeta::new("concurrency", Ty::Uint)
        },
        PropMeta {
            hide: true,
            ..PropMeta::new("internal", Ty::Bool)
        },
        PropMeta {
            scope: Scope::Global,
            ..PropMeta::new("trusted", Ty::Bool)
        },
        // Bindings and no environment variable, which is where the blank line went missing.
        PropMeta {
            bindings: &[("pkl", "hk.stash")],
            ..PropMeta::new("stash", Ty::String)
        },
    ];
    const REGISTRY: Registry = Registry::new(PROPS);

    struct Fixed(Vec<Entry>);

    impl Layer for Fixed {
        fn source(&self) -> SourceKind {
            SourceKind::FILE
        }
        fn load(&self, _ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
            Ok(LayerOutput {
                entries: self.0.clone(),
                warnings: Vec::new(),
            })
        }
    }

    fn id(key: &str) -> PropId {
        REGISTRY.lookup(key).expect("declared").id
    }

    #[test]
    fn an_explanation_names_the_winner_and_what_it_beat() {
        // The question a user asks: not "what is it" but "why is it that". The answer needs the
        // exact identifier — `HK_JOBS`, not "the environment" — because that is the thing they
        // have to go and change.
        let env = Fixed(vec![Entry::new(
            id("jobs"),
            Value::Int(8),
            Origin::new(SourceKind::ENV, "HK_JOBS"),
        )]);
        let file = Fixed(vec![Entry::new(
            id("jobs"),
            Value::Int(2),
            Origin::file("hk.toml#jobs", FileScope::Project),
        )]);
        let resolved =
            resolve(REGISTRY, Layers::new().then(&env).then(&file)).expect("should resolve");

        let text = explain(&resolved, "jobs").expect("declared");
        assert!(text.starts_with("jobs = 8\n"), "{text}");
        assert!(text.contains("set by  HK_JOBS"), "{text}");
        assert!(text.contains("type    uint"), "{text}");
        assert!(text.contains("How many jobs to run at once"), "{text}");
        // What else tried, lowest first, and the winner not repeated among them.
        assert!(text.contains("also considered"), "{text}");
        assert!(text.contains("the default"), "{text}");
        assert!(text.contains("hk.toml#jobs"), "{text}");
        assert_eq!(
            text.matches("HK_JOBS").count(),
            2,
            "once as the winner, once as a place it can be set:\n{text}"
        );
        // Nothing is padded to a column, because there is no second column: a path copied out of
        // an explanation has to be the path the merge recorded, trailing spaces and all.
        for line in text.lines() {
            assert_eq!(line, line.trim_end(), "trailing space:\n{text}");
        }
        // The flag, before the variables: a user reading an explanation because they do not like the
        // answer reaches for the command line first, and listing the variables alone answered half
        // the question they asked.
        // And where else it could come from, which is the other half of the question. Their
        // positions rather than their presence: an explanation that lists the same three lines in
        // the other order is a different answer to "what do I change", and `contains` cannot tell.
        let cli = text.find("command line --jobs, -j").expect(text.as_str());
        let env = text
            .find("environment  HK_JOBS, HK_JOB")
            .expect(text.as_str());
        let also = text.find("also         git hk.jobs").expect(text.as_str());
        assert!(cli < env && env < also, "{text}");
    }

    #[test]
    fn a_default_is_not_described_as_something_somebody_set() {
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        let text = explain(&resolved, "jobs").expect("declared");
        assert!(text.contains("default the default"), "{text}");
        assert!(!text.contains("set by"), "nobody set it: {text}");
        // With one contributor there is nothing it beat, so no list of also-considereds.
        assert!(!text.contains("also considered"), "{text}");
    }

    #[test]
    fn a_rewritten_value_says_it_was_derived() {
        // mise's `raw` implying `jobs = 1`. Calling this "set by" would send the user looking
        // for a file that never said it.
        let mut resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        resolved.coerced(id("jobs"), Value::Int(1), "raw implies one job");
        let text = explain(&resolved, "jobs").expect("declared");
        assert!(text.contains("jobs = 1"), "{text}");
        assert!(text.contains("derived raw implies one job"), "{text}");
    }

    #[test]
    fn asking_after_an_old_name_answers_about_both() {
        // Somebody reading a config file written a year ago. Printing only the canonical name
        // would answer a question they did not ask.
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        let text = explain(&resolved, "concurrency").expect("declared");
        assert!(text.starts_with("concurrency is now jobs\n"), "{text}");
        assert!(text.contains("jobs = 4"), "{text}");
        assert!(text.contains("deprecated: Use jobs instead."), "{text}");
    }

    #[test]
    fn a_name_renamed_twice_still_finds_the_notice_along_the_way() {
        // A setting renamed once, then again: `threads` became `concurrency`, which became `jobs`.
        // The notice worth printing is the one on the step that has it, and reading only the name
        // the user typed found nothing — a rename is not itself an explanation of what to do
        // instead.
        static CHAIN: &[PropMeta] = &[
            PropMeta {
                default: Some(Const::Int(4)),
                ..PropMeta::new("jobs", Ty::Uint)
            },
            PropMeta {
                deprecated: Some("Use jobs instead."),
                renamed_to: Some("jobs"),
                ..PropMeta::new("concurrency", Ty::Uint)
            },
            PropMeta {
                renamed_to: Some("concurrency"),
                ..PropMeta::new("threads", Ty::Uint)
            },
        ];
        const CHAINED: Registry = Registry::new(CHAIN);

        let resolved = resolve(CHAINED, Layers::new()).expect("should resolve");
        let text = explain(&resolved, "threads").expect("declared");
        assert!(text.starts_with("threads is now jobs\n"), "{text}");
        assert!(text.contains("deprecated: Use jobs instead."), "{text}");
    }

    #[test]
    fn a_setting_nothing_supplied_says_so_rather_than_guessing() {
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        let text = explain(&resolved, "exclude").expect("declared");
        assert!(text.contains("exclude is unset"), "{text}");
        // A key the registry does not have is the caller's to report: a CLI usually wants to
        // suggest a near miss, and this module has no business guessing at one.
        assert_eq!(explain(&resolved, "nonesuch"), None);
    }

    #[test]
    fn a_setting_that_is_empty_says_so_rather_than_trailing_off() {
        // Emptied on purpose, which is a supported thing to do: `HK_EXCLUDE=` is how a declared
        // default is turned off. The value's own text is nothing at all, so `key = {}` printed a
        // line that stopped after the `=` — with a trailing space, and looking truncated rather
        // than empty. Set is not unset, and both are worth saying.
        let cleared = Fixed(vec![Entry::new(
            id("exclude"),
            Value::List(Vec::new()),
            Origin::new(SourceKind::ENV, "HK_EXCLUDE"),
        )]);
        let empty_text = Fixed(vec![Entry::new(
            id("stash"),
            Value::from(""),
            Origin::new(SourceKind::ENV, "HK_STASH"),
        )]);
        let resolved =
            resolve(REGISTRY, Layers::new().then(&cleared).then(&empty_text)).expect("resolves");

        let text = explain(&resolved, "exclude").expect("declared");
        assert!(text.starts_with("exclude = []\n"), "{text}");
        assert!(text.contains("set by  HK_EXCLUDE"), "{text}");
        let text = explain(&resolved, "stash").expect("declared");
        assert!(text.starts_with("stash = \"\"\n"), "{text}");

        // And the same in a listing, where every other line has a value on it.
        let listed = list(&resolved);
        assert!(listed.contains(&"exclude = []".to_string()), "{listed:?}");
        assert!(listed.contains(&"stash = \"\"".to_string()), "{listed:?}");
        for line in &listed {
            assert_eq!(line, line.trim_end(), "trailing space: {listed:?}");
        }
    }

    #[test]
    fn every_contributor_to_a_union_is_accounted_for() {
        // What hk's per-item provenance is for: a list assembled from several places, where
        // "where did this come from" has more than one answer.
        let env = Fixed(vec![Entry::new(
            id("exclude"),
            Value::List(vec![Value::from("target")]),
            Origin::new(SourceKind::ENV, "HK_EXCLUDE"),
        )]);
        let file = Fixed(vec![Entry::new(
            id("exclude"),
            Value::List(vec![Value::from("vendor")]),
            Origin::file("hk.toml#exclude", FileScope::Project),
        )]);
        let resolved =
            resolve(REGISTRY, Layers::new().then(&env).then(&file)).expect("should resolve");
        let text = explain(&resolved, "exclude").expect("declared");
        assert!(text.contains("exclude = vendor,target"), "{text}");
        assert!(text.contains("hk.toml#exclude"), "{text}");
        assert!(text.contains("HK_EXCLUDE"), "{text}");
    }

    #[test]
    fn a_value_with_a_newline_in_it_still_occupies_one_line() {
        // Both renderers here are line-oriented, and a multi-line string is a perfectly ordinary
        // thing to put in a TOML file — its continuation looked like another setting in a
        // listing, and like provenance in an explanation.
        let file = Fixed(vec![Entry::new(
            id("stash"),
            Value::from("first\nsecond"),
            Origin::file("hk.toml#stash", FileScope::Project),
        )]);
        let resolved = resolve(REGISTRY, Layers::new().then(&file)).expect("should resolve");

        let text = explain(&resolved, "stash").expect("declared");
        assert!(text.contains("stash = first\\nsecond"), "{text}");
        assert_eq!(
            text.lines().filter(|l| l.starts_with("stash")).count(),
            1,
            "the value should not start a second record:\n{text}"
        );
        // And a listing keeps one setting per line, which is the whole shape of its output.
        let lines = list(&resolved);
        assert!(
            lines.iter().any(|l| l == "stash = first\\nsecond"),
            "{lines:?}"
        );
        assert_eq!(lines.len(), 4, "{lines:?}");
    }

    #[test]
    fn nothing_interpolated_into_a_line_can_leave_it() {
        // Three things here can carry a newline, and all three did: the value, the *origin* — a
        // path may contain one — and a warning message, which quotes the value it rejected. Any
        // of them splitting its line makes the rest read as another record, and for warnings it
        // hides every one after it.
        struct Odd;
        impl Layer for Odd {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                let mut out = LayerOutput::new();
                // A path with a newline in it, and a value the declared type will refuse.
                let odd = Origin::file("hk\n.toml#jobs", FileScope::Project);
                match ctx.entry_for_key("jobs", "lots\nand lots", odd) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
                // Two contributors, so the also-considered list is rendered — and the one
                // that loses is the one whose path carries the newline.
                out.push(Entry::new(
                    id("stash"),
                    Value::from("lower"),
                    Origin::file("also\nodd#stash", FileScope::System),
                ));
                out.push(Entry::new(
                    id("stash"),
                    Value::from("first\nsecond"),
                    // The winner's path carries one too: its line is rendered by a different
                    // branch from the also-considered list, and only one of the two was covered.
                    Origin::file("winner\nodd#stash", FileScope::Project),
                ));
                Ok(out)
            }
        }
        let odd = Odd;
        let resolved = resolve(REGISTRY, Layers::new().then(&odd)).expect("should resolve");

        // One warning, one line, whatever the rejected value contained.
        let lines = warnings(&resolved);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].lines().count(), 1, "{lines:?}");

        // And an explanation stays one fact per line, with the odd path escaped into its own.
        let text = explain(&resolved, "stash").expect("declared");
        for line in text.lines() {
            assert!(
                !line.trim_start().starts_with("odd"),
                "a path's second half became a record of its own:\n{text}"
            );
        }
        assert!(
            text.contains("also\\nodd#stash"),
            "the losing contributor's path should be escaped in place:\n{text}"
        );
        assert_eq!(
            text.matches("also\\nodd#stash").count(),
            1,
            "once, in the also-considered list:\n{text}"
        );
        assert!(
            text.contains("winner\\nodd#stash"),
            "the winner's own path should be escaped too:\n{text}"
        );

        // A path is reported as it is, though: doubling the separators in
        // `C:\Users\me\hk.toml` gives a reader something to copy that leads nowhere.
        struct Windows;
        impl Layer for Windows {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, _ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                Ok(LayerOutput {
                    entries: vec![Entry::new(
                        id("stash"),
                        Value::from(r"C:\Users\me"),
                        Origin::file(r"C:\Users\me\hk.toml#stash", FileScope::Global),
                    )],
                    warnings: Vec::new(),
                })
            }
        }
        let windows = Windows;
        let resolved = resolve(REGISTRY, Layers::new().then(&windows)).expect("should resolve");
        let text = explain(&resolved, "stash").expect("declared");
        assert!(text.contains(r"C:\Users\me\hk.toml#stash"), "{text}");
        assert!(!text.contains(r"C:\\Users"), "separators doubled:\n{text}");
    }

    #[test]
    fn metadata_stays_on_its_own_line_too() {
        // An adopter's help is a doc comment, and a doc comment with a second paragraph is the
        // ordinary case — not an exotic one. Written raw, one fact became several records that
        // read like provenance.
        static PROSE: &[PropMeta] = &[PropMeta {
            help: Some("One line\n\nAnd a second paragraph."),
            deprecated: Some("Gone soon.\nReally."),
            ..PropMeta::new("wordy", Ty::Bool)
        }];
        const WORDY: Registry = Registry::new(PROSE);
        let resolved = resolve(WORDY, Layers::new()).expect("should resolve");
        let text = explain(&resolved, "wordy").expect("declared");
        assert!(
            text.contains("One line\\n\\nAnd a second paragraph."),
            "{text}"
        );
        assert!(text.contains("deprecated: Gone soon.\\nReally."), "{text}");
        // Four lines: the value, the type, the help, and the deprecation — plus the blank one
        // before it. Any of them splitting would push the count up.
        assert_eq!(text.lines().count(), 5, "{text:?}");
    }

    #[test]
    fn a_setting_with_bindings_and_no_environment_still_reads_as_a_section() {
        // The blank line used to belong to the environment section, so a setting with git or pkl
        // bindings and no variables had its `also` line jammed against the type above it.
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        let text = explain(&resolved, "stash").expect("declared");
        assert!(text.contains("\n\n  also         pkl hk.stash"), "{text:?}");
    }

    #[test]
    fn a_listing_leaves_out_what_is_hidden_and_sorts_what_is_left() {
        // A registry's order is the order somebody wrote a TOML file in; a list a human reads
        // should not depend on that. And a hidden setting is documented nowhere, so a listing
        // would be the one place it surfaced.
        let resolved = resolve(REGISTRY, Layers::new()).expect("should resolve");
        let lines = list(&resolved);
        assert_eq!(
            lines,
            [
                "exclude is unset",
                "jobs = 4",
                "stash is unset",
                "trusted is unset"
            ],
            "sorted, without `internal` or the old name for `jobs`"
        );
    }

    #[test]
    fn a_warning_names_its_place_once() {
        // A type error used to put the origin in its own text as well, so the rendered line said
        // the same file twice — harder to scan, and no more informative for it. The message says
        // what is wrong and the renderer says where, which also means every warning is rendered
        // the same way.
        struct Bad;
        impl Layer for Bad {
            fn source(&self) -> SourceKind {
                SourceKind::FILE
            }
            fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
                let mut out = LayerOutput::new();
                let origin = Origin::file("hk.toml#jobs", FileScope::Project);
                match ctx.entry_for_key("jobs", "lots", origin) {
                    Ok(entry) => out.push(entry),
                    Err(warning) => out.warn(warning),
                }
                Ok(out)
            }
        }
        let bad = Bad;
        let resolved = resolve(REGISTRY, Layers::new().then(&bad)).expect("should resolve");
        let lines = warnings(&resolved);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(
            lines[0].matches("hk.toml#jobs").count(),
            1,
            "the place should appear once: {lines:?}"
        );
        assert!(lines[0].ends_with("(hk.toml#jobs)"), "{lines:?}");
    }

    #[test]
    fn warnings_carry_the_place_that_caused_them() {
        // A message without the file is a message a user cannot act on.
        let project = Fixed(vec![Entry::new(
            id("trusted"),
            Value::Bool(true),
            Origin::file("hk.toml#trusted", FileScope::Project),
        )]);
        let resolved = resolve(REGISTRY, Layers::new().then(&project)).expect("should resolve");
        let lines = warnings(&resolved);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("trusted cannot be set"), "{lines:?}");
        assert!(lines[0].ends_with("(hk.toml#trusted)"), "{lines:?}");
    }
}
