//! Named sets of flag declarations, and the `use` node that pulls one in.
//!
//! mise's checked-in spec is 5,592 lines for 711 flags, and most of that count is
//! the same handful of declarations written again under the next command. The
//! derive has an answer for this — `flatten` — and a hand-written spec had none.
//!
//! A flagset is authoring sugar, resolved while the file is read:
//!
//! ```kdl
//! flagset "output" {
//!     flag "-v --verbose" help="Print more"
//!     flag "--json" help="JSON output"
//! }
//!
//! cmd "build" {
//!     use "output"
//!     flag "--release"
//! }
//! ```
//!
//! By the time anything downstream sees the spec, `build` holds three ordinary
//! flags in that order. Nothing else in the model, in help, in completions or in
//! the generated parsers learns a new concept, which is the point: the same rule
//! the derive follows, that a spec is the semantic model, means reuse has to
//! disappear into what it stands for rather than becoming vocabulary every
//! consumer must implement.
//!
//! The cost of that choice is that it is one-way. A spec parsed and re-emitted
//! contains the expanded flags, not the `flagset` and `use` nodes that produced
//! them, the same way `include` does not survive a round trip.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::miette::SourceSpan;
use indexmap::IndexMap;
use serde::Serialize;

use crate::error::UsageErr;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::NodeHelper;
use crate::spec::spec_flag_forms_overlap;
use crate::{SpecCommand, SpecFlag};

/// A named, reusable set of flag declarations.
///
/// Declared at the root of a spec, never inside a command: a set that one command
/// can see and its sibling cannot is a scoping rule to explain for no benefit.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecFlagSet {
    pub name: String,
    /// Flags declared directly in the set.
    pub flags: Vec<SpecFlag>,
    /// Sets this one composes, so a big set can be assembled from small ones.
    ///
    /// Emptied while the file is read, like a command's: what it named becomes part of
    /// [`Self::flags`], so a file that includes this one inherits a set with nothing left to
    /// resolve.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<SpecUse>,
    /// The node, for an error raised while flattening the set rather than at one `use`.
    #[serde(skip)]
    pub(crate) span: SourceSpan,
    /// The file this set was declared in, resolved so two routes to one file agree.
    ///
    /// A name may be declared once, and what makes that one rule rather than two is that a
    /// *declaration* is what is counted: a shared file reaching a spec through two includes
    /// is one declaration arriving twice, not two of them.
    #[serde(skip)]
    pub(crate) declared_in: PathBuf,
}

/// One `use` node: the flagsets whose declarations belong where it stands.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct SpecUse {
    /// The sets named, in the order they were written.
    pub names: Vec<String>,
    /// Where the expansion belongs in the owner's flag list.
    ///
    /// Help order is spec order, so a `use` between two flags has to expand
    /// between them rather than at the end of the list.
    pub at: usize,
    /// Kept for the error a bad name produces, which is reported after the whole
    /// file is read rather than at the node.
    #[serde(skip)]
    pub(crate) span: SourceSpan,
}

impl Default for SpecFlagSet {
    fn default() -> Self {
        Self {
            name: String::new(),
            flags: vec![],
            uses: vec![],
            // Nowhere to point: a set that was not read from a file has no node, and the
            // errors this span serves can only come from one that was.
            span: (0, 0).into(),
            declared_in: PathBuf::new(),
        }
    }
}

/// Where a file's declarations come from, as a name two routes to that file share.
///
/// A diamond of includes reaches one shared file as `a/../common.usage.kdl` from one side and
/// `b/../common.usage.kdl` from the other. Those are the same declaration, and comparing the
/// paths as written would call them two.
pub(crate) fn declaring_file(file: &Path) -> PathBuf {
    // A spec parsed from a string has no file, and canonicalizing nothing fails: the empty
    // path is then the honest answer, and it cannot collide with a real one.
    std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf())
}

impl SpecFlagSet {
    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper) -> Result<Self, UsageErr> {
        node.ensure_arg_len(1..=1)?;
        let mut set = Self {
            name: node.arg(0)?.ensure_string()?,
            flags: vec![],
            uses: vec![],
            span: node.span(),
            declared_in: declaring_file(&ctx.file),
        };
        if let Some((k, v)) = node.props().first() {
            bail_parse!(ctx, v.entry.span(), "unsupported flagset prop {k}");
        }
        for child in node.children() {
            match child.name() {
                "flag" => set.flags.push(SpecFlag::parse(ctx, &child)?),
                "use" => set.uses.push(SpecUse::parse(ctx, &child, set.flags.len())?),
                // Positionals are deliberately not here. A set of flags is reusable
                // because a flag is identified by its spelling wherever it lands; a
                // positional is identified by its position, so the same set spliced
                // into two commands with different arguments means two different
                // things. `flatten` on the derive side has the field order of one
                // struct to go by and this has nothing.
                "arg" => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "a flagset holds flags, not arguments: declare the argument on \
                     each command that takes it"
                ),
                k => bail_parse!(ctx, child.node.name().span(), "unsupported flagset key {k}"),
            }
        }
        Ok(set)
    }
}

impl SpecUse {
    pub(crate) fn parse(
        ctx: &ParsingContext,
        node: &NodeHelper,
        at: usize,
    ) -> Result<Self, UsageErr> {
        node.ensure_arg_len(1..)?;
        if let Some((k, v)) = node.props().first() {
            bail_parse!(ctx, v.entry.span(), "unsupported use prop {k}");
        }
        if !node.children().is_empty() {
            bail_parse!(
                ctx,
                node.span(),
                "`use` names flagsets and holds nothing: declare the flags in the \
                 flagset itself"
            );
        }
        Ok(Self {
            names: node
                .args()
                .map(|a| a.ensure_string())
                .collect::<Result<Vec<_>, _>>()?,
            at,
            span: node.span(),
        })
    }
}

/// Replace every `use` in the file with the flags it names.
///
/// Runs once, after the whole file is read, so a `use` may name a set declared below it or
/// brought in by an `include`. What it cannot see is a set declared in a file that includes
/// *this* one: each file resolves its own `use` nodes, and an unresolved one is an error
/// there rather than a value carried up to be resolved by whoever reads the file next.
///
/// Which is why the sets are flattened first, before any command is looked at, even though
/// nothing may use them. Resolving a set only when a command asked for one left an included
/// file's `use` to be answered by the file that included it — with the wrong flagsets in
/// scope, and an error pointing into the wrong source.
pub(crate) fn expand(
    ctx: &ParsingContext,
    cmd: &mut SpecCommand,
    flagsets: &mut IndexMap<String, SpecFlagSet>,
) -> Result<(), UsageErr> {
    let mut cache = {
        let mut resolver = Resolver {
            ctx,
            flagsets,
            cache: HashMap::new(),
            stack: vec![],
        };
        for (name, set) in resolver.flagsets {
            resolver.resolve(name, set.span)?;
        }
        resolver.cache
    };
    // A set that used another is now the flags of both. Written back so that what travels
    // through an `include` is a set and not a question.
    for (name, set) in flagsets.iter_mut() {
        if let Some(flags) = cache.get(name) {
            set.flags = flags.clone();
        }
        set.uses.clear();
    }
    let mut resolver = Resolver {
        ctx,
        flagsets,
        cache: core::mem::take(&mut cache),
        stack: vec![],
    };
    expand_cmd(cmd, &mut resolver)
}

fn expand_cmd(cmd: &mut SpecCommand, resolver: &mut Resolver) -> Result<(), UsageErr> {
    // Taken rather than read: the expansion _is_ the flags now, and leaving the
    // request behind would let a second pass double it.
    let uses = std::mem::take(&mut cmd.uses);
    splice(&mut cmd.flags, &uses, resolver)?;
    for sub in cmd.subcommands.values_mut() {
        expand_cmd(sub, resolver)?;
    }
    Ok(())
}

/// Insert what each `use` names, at the position where it stood.
fn splice(
    flags: &mut Vec<SpecFlag>,
    uses: &[SpecUse],
    resolver: &mut Resolver,
) -> Result<(), UsageErr> {
    let mut inserted = 0;
    for u in uses {
        let mut at = (u.at + inserted).min(flags.len());
        for name in &u.names {
            for flag in resolver.resolve(name, u.span)? {
                // The nearer declaration owns the spelling, as it does when a
                // subcommand redeclares a global: a command that says `use "output"`
                // and then declares its own `--json` gets its own, and a set
                // reachable twice through composition contributes once.
                if flags.iter().any(|f| spec_flag_forms_overlap(f, &flag)) {
                    continue;
                }
                flags.insert(at, flag);
                at += 1;
                inserted += 1;
            }
        }
    }
    Ok(())
}

struct Resolver<'a> {
    ctx: &'a ParsingContext,
    flagsets: &'a IndexMap<String, SpecFlagSet>,
    /// A set composed by several commands is resolved once.
    cache: HashMap<String, Vec<SpecFlag>>,
    /// The chain currently being resolved, so a cycle is reported as the path
    /// that closes it rather than as a stack overflow.
    stack: Vec<String>,
}

impl Resolver<'_> {
    fn resolve(&mut self, name: &str, span: SourceSpan) -> Result<Vec<SpecFlag>, UsageErr> {
        if let Some(flags) = self.cache.get(name) {
            return Ok(flags.clone());
        }
        if self.stack.iter().any(|seen| seen == name) {
            let ctx = self.ctx;
            let path = self
                .stack
                .iter()
                .map(String::as_str)
                .chain([name])
                .collect::<Vec<_>>()
                .join(" -> ");
            bail_parse!(ctx, span, "flagset cycle: {path}");
        }
        let flagsets = self.flagsets;
        let Some(set) = flagsets.get(name) else {
            let ctx = self.ctx;
            let known = flagsets.keys().cloned().collect::<Vec<_>>().join(", ");
            let hint = match known.is_empty() {
                true => "no flagsets are declared".to_string(),
                false => format!("declared: {known}"),
            };
            bail_parse!(ctx, span, "unknown flagset \"{name}\" ({hint})");
        };
        self.stack.push(name.to_string());
        let mut flags = set.flags.clone();
        let result = splice(&mut flags, &set.uses, self);
        self.stack.pop();
        result?;
        self.cache.insert(name.to_string(), flags.clone());
        Ok(flags)
    }
}

#[cfg(test)]
mod tests {
    use crate::Spec;
    use insta::assert_snapshot;

    fn parse(input: &str) -> Spec {
        Spec::parse(&Default::default(), input).unwrap()
    }

    /// The message a bad spec produced. `UsageErr::InvalidInput` renders as a source
    /// diagnostic, so `to_string` is the headline rather than what went wrong.
    fn err(input: &str) -> String {
        match Spec::parse(&Default::default(), input).unwrap_err() {
            crate::error::UsageErr::InvalidInput(msg, _, _) => msg,
            err => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn a_set_expands_where_the_use_stands() {
        // The `use` is between two flags, so the set's flags land between them: help order
        // is spec order, and a set that always appended would reorder the command.
        let spec = parse(
            r#"
bin "ex"
flagset "output" {
    flag "-v --verbose" help="Print more"
    flag "--json" help="JSON output"
}
cmd "build" {
    flag "--release"
    use "output"
    flag "--target" {
        arg "<triple>"
    }
}
        "#,
        );
        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd build {
            flag --release
            flag "-v --verbose" help="Print more"
            flag --json help="JSON output"
            flag --target {
                arg <triple>
            }
        }
        "#);
    }

    #[test]
    fn one_use_names_several_sets_and_the_root_can_use_them_too() {
        let spec = parse(
            r#"
bin "ex"
use "logging" "output"
flagset "logging" {
    flag "-v --verbose" global=#true
}
flagset "output" {
    flag "--json"
}
        "#,
        );
        // Declared below the `use` that names them: a spec is read whole before it is
        // resolved, so declaration order is the author's business.
        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        flag "-v --verbose" global=#true
        flag --json
        "#);
    }

    #[test]
    fn a_set_composes_other_sets_and_a_diamond_contributes_once() {
        let spec = parse(
            r#"
bin "ex"
flagset "common" {
    flag "-v --verbose"
}
flagset "output" {
    use "common"
    flag "--json"
}
flagset "input" {
    use "common"
    flag "--stdin"
}
cmd "run" {
    use "output" "input"
}
        "#,
        );
        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd run {
            flag "-v --verbose"
            flag --json
            flag --stdin
        }
        "#);
    }

    #[test]
    fn the_commands_own_declaration_wins() {
        // The same rule a redeclared global follows: the nearer declaration owns the
        // spelling. A command that wants one flag of a set said differently keeps the set.
        let spec = parse(
            r#"
bin "ex"
flagset "output" {
    flag "--json" help="JSON output"
    flag "-q --quiet"
}
cmd "build" {
    use "output"
    flag "--json" help="build's own JSON, with a schema"
}
        "#,
        );
        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd build {
            flag "-q --quiet"
            flag --json help="build's own JSON, with a schema"
        }
        "#);
    }

    #[test]
    fn a_short_form_collision_counts_as_the_same_flag() {
        // Overlap is per-form, as it is everywhere else: `-j` is taken even though the
        // long names differ.
        let spec = parse(
            r#"
bin "ex"
flagset "common" {
    flag "-j --jobs" {
        arg "<n>"
    }
}
cmd "build" {
    use "common"
    flag "-j --job-count" {
        arg "<n>"
    }
}
        "#,
        );
        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd build {
            flag "-j --job-count" {
                arg <n>
            }
        }
        "#);
    }

    #[test]
    fn a_set_reaches_every_depth_of_the_tree() {
        let spec = parse(
            r#"
bin "ex"
flagset "common" {
    flag "-v --verbose"
}
cmd "remote" {
    use "common"
    cmd "add" {
        use "common"
        arg "<name>"
    }
}
        "#,
        );
        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd remote {
            flag "-v --verbose"
            cmd add {
                flag "-v --verbose"
                arg <name>
            }
        }
        "#);
    }

    #[test]
    fn an_included_file_can_hold_the_shared_sets() {
        // The reason `include` exists is a file of declarations shared by a CLI's specs,
        // and flagsets are exactly that kind of declaration.
        let dir = tempfile::tempdir().unwrap();
        let common = dir.path().join("common.usage.kdl");
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(
            &common,
            "flagset \"common\" {\n    flag \"-v --verbose\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            &root,
            "bin \"ex\"\ninclude file=\"./common.usage.kdl\"\ncmd \"build\" {\n    use \"common\"\n}\n",
        )
        .unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd build {
            flag "-v --verbose"
        }
        "#);
    }

    #[test]
    fn a_set_is_resolved_by_the_file_that_wrote_it() {
        // The composition an included file writes is answered by the included file. Letting
        // the includer answer it would make what a file means depend on who read it, and
        // would report the mistake against the wrong source.
        let dir = tempfile::tempdir().unwrap();
        let common = dir.path().join("common.usage.kdl");
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(&common, "flagset \"child\" {\n    use \"parent-only\"\n}\n").unwrap();
        std::fs::write(
            &root,
            "bin \"ex\"\ninclude file=\"./common.usage.kdl\"\nflagset \"parent-only\" {\n                 flag \"--from-parent\"\n}\ncmd \"build\" {\n    use \"child\"\n}\n",
        )
        .unwrap();

        let err = Spec::parse_file(&root).unwrap_err();
        let crate::error::UsageErr::InvalidInput(msg, _, source) = err else {
            panic!("unexpected error: {err:?}");
        };
        assert!(
            msg.contains("unknown flagset \"parent-only\" (declared: child)"),
            "{msg}"
        );
        // Named against the file that wrote the `use`, which is the half a lazy resolve got
        // wrong even where it happened to refuse.
        assert!(
            source.name().ends_with("common.usage.kdl"),
            "{:?}",
            source.name()
        );
    }

    #[test]
    fn a_use_goes_with_the_flags_an_include_replaced() {
        // An included file that declares root flags owns the root's flags — that is what
        // `include` has always meant, and why the merge drops groups with them. A `use` is a
        // declaration of flags, so it goes the same way: keeping it would splice the set into
        // the incoming list at a position from a list that is gone.
        let dir = tempfile::tempdir().unwrap();
        let included = dir.path().join("overrides.usage.kdl");
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(&included, "flag \"--from-include\"\n").unwrap();
        std::fs::write(
            &root,
            "bin \"ex\"\nflagset \"common\" {\n    flag \"-v --verbose\"\n}\nflag \"--own\"\n             use \"common\"\ninclude file=\"./overrides.usage.kdl\"\n",
        )
        .unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        flag --from-include
        "#);
    }

    #[test]
    fn a_use_survives_an_include_that_declares_no_flags() {
        // The ordinary shape: a file of shared declarations, and a spec that uses them. The
        // rule above must not reach this one.
        let dir = tempfile::tempdir().unwrap();
        let included = dir.path().join("common.usage.kdl");
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(
            &included,
            "flagset \"common\" {\n    flag \"-v --verbose\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            &root,
            "bin \"ex\"\nuse \"common\"\ninclude file=\"./common.usage.kdl\"\n",
        )
        .unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        flag "-v --verbose"
        "#);
    }

    #[test]
    fn a_set_nothing_uses_is_still_resolved() {
        // Every set is flattened whether or not a command asks for one, so a mistake inside
        // an unused set is reported rather than waiting for the day something uses it.
        let msg = err("flagset \"a\" {\n    use \"missing\"\n}\n");
        assert!(msg.contains("unknown flagset \"missing\""), "{msg}");
    }

    #[test]
    fn an_included_set_may_compose_one_from_its_own_file() {
        let dir = tempfile::tempdir().unwrap();
        let common = dir.path().join("common.usage.kdl");
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(
            &common,
            "flagset \"logging\" {\n    flag \"-v --verbose\"\n}\nflagset \"common\" {\n                 use \"logging\"\n    flag \"--config\"\n}\n",
        )
        .unwrap();
        std::fs::write(
            &root,
            "bin \"ex\"\ninclude file=\"./common.usage.kdl\"\ncmd \"build\" {\n    use \"common\"\n}\n",
        )
        .unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd build {
            flag "-v --verbose"
            flag --config
        }
        "#);
    }

    #[test]
    fn the_flags_a_set_brought_parse_like_any_other() {
        // The expansion is the whole feature: nothing downstream of the parse knows a
        // flagset was involved.
        let spec = parse(
            r#"
bin "ex"
flagset "common" {
    flag "-j --jobs" {
        arg "<n>"
    }
}
cmd "build" {
    use "common"
}
        "#,
        );
        let words = ["ex", "build", "--jobs", "4"].map(String::from);
        let parsed = crate::parse(&spec, &words).unwrap();
        assert_eq!(parsed.as_env().get("usage_jobs").unwrap(), "4");
    }

    #[test]
    fn a_global_from_a_set_is_inherited_like_any_other() {
        // Expansion happens before anything reads the spec, so a set's flags take part in
        // every rule an inline declaration would: `global` here, and equally `conflicts`,
        // `group` membership, or a completer keyed on the flag's value name.
        let spec = parse(
            r#"
bin "ex"
flagset "logging" {
    flag "-v --verbose" global=#true
}
use "logging"
cmd "build"
        "#,
        );
        let words = ["ex", "build", "--verbose"].map(String::from);
        let parsed = crate::parse(&spec, &words).unwrap();
        assert_eq!(parsed.as_env().get("usage_verbose").unwrap(), "true");
    }

    #[test]
    fn an_unknown_set_says_what_is_declared() {
        let msg = err(r#"
bin "ex"
flagset "output" {
    flag "--json"
}
cmd "build" {
    use "outupt"
}
        "#);
        assert!(
            msg.contains("unknown flagset \"outupt\" (declared: output)"),
            "{msg}"
        );
    }

    #[test]
    fn a_use_with_no_sets_at_all_says_so() {
        let msg = err("bin \"ex\"\ncmd \"build\" {\n    use \"output\"\n}\n");
        assert!(
            msg.contains("unknown flagset \"output\" (no flagsets are declared)"),
            "{msg}"
        );
    }

    #[test]
    fn a_cycle_is_reported_as_the_path_that_closes_it() {
        let msg = err(r#"
bin "ex"
flagset "a" {
    use "b"
}
flagset "b" {
    use "a"
}
cmd "build" {
    use "a"
}
        "#);
        assert!(msg.contains("flagset cycle: a -> b -> a"), "{msg}");
    }

    #[test]
    fn a_set_that_uses_itself_is_the_same_error() {
        let msg = err("bin \"ex\"\nflagset \"a\" {\n    use \"a\"\n}\nuse \"a\"\n");
        assert!(msg.contains("flagset cycle: a -> a"), "{msg}");
    }

    #[test]
    fn a_name_may_be_declared_once() {
        let msg = err(r#"
flagset "output" {
    flag "--json"
}
flagset "output" {
    flag "--yaml"
}
        "#);
        assert!(msg.contains("a flagset may be declared only once"), "{msg}");
    }

    /// The message from a spec read off disk, so an include can take part.
    fn err_file(root: &std::path::Path) -> String {
        match Spec::parse_file(root).unwrap_err() {
            crate::error::UsageErr::InvalidInput(msg, _, _) => msg,
            err => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn a_name_an_include_also_declares_is_refused_whichever_side_wrote_it_first() {
        // Declared twice is declared twice, and an `include` does not make it a choice.
        // Extending the map would have let the incoming set take the name — but only when
        // the `include` stood below the declaration, since a `flagset` written after an
        // `include` already hit the once-only check. So which set answered a `use` came
        // down to where in the file the `include` was written.
        let dir = tempfile::tempdir().unwrap();
        let common = dir.path().join("common.usage.kdl");
        std::fs::write(&common, "flagset \"output\" {\n    flag \"--yaml\"\n}\n").unwrap();
        let own = "flagset \"output\" {\n    flag \"--json\"\n}\n";
        let include = "include file=\"./common.usage.kdl\"\n";

        let after = dir.path().join("after.usage.kdl");
        std::fs::write(&after, format!("bin \"ex\"\n{own}{include}")).unwrap();
        let msg = err_file(&after);
        assert!(
            msg.contains("a flagset may be declared only once")
                && msg.contains("common.usage.kdl")
                && msg.contains("\"output\""),
            "{msg}"
        );

        // The other order was already refused, and still says so.
        let before = dir.path().join("before.usage.kdl");
        std::fs::write(&before, format!("bin \"ex\"\n{include}{own}")).unwrap();
        let msg = err_file(&before);
        assert!(msg.contains("a flagset may be declared only once"), "{msg}");
    }

    #[test]
    fn a_shared_file_may_reach_a_spec_by_two_routes() {
        // The shape the feature asks for. Each file resolves its own `use` nodes, so every
        // file whose commands name the shared sets includes the file that declares them —
        // and a spec that includes two such files sees the shared set arrive twice. That is
        // one declaration by two routes, not two declarations, so the once-only rule has
        // nothing to say about it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("cmds")).unwrap();
        std::fs::write(
            dir.path().join("common.usage.kdl"),
            "flagset \"common\" {\n    flag \"-v --verbose\"\n}\n",
        )
        .unwrap();
        for cmd in ["build", "test"] {
            // Reached as `cmds/../common.usage.kdl` from here and as `./common.usage.kdl`
            // from the root: the same file, spelled two ways.
            let body = format!(
                "include file=\"../common.usage.kdl\"\ncmd \"{cmd}\" {{\n    use \"common\"\n}}\n"
            );
            std::fs::write(
                dir.path().join("cmds").join(format!("{cmd}.usage.kdl")),
                body,
            )
            .unwrap();
        }
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(
            &root,
            "bin \"ex\"\ninclude file=\"./common.usage.kdl\"\ninclude file=\"./cmds/build.usage.kdl\"\ninclude file=\"./cmds/test.usage.kdl\"\n",
        )
        .unwrap();

        let spec = Spec::parse_file(&root).unwrap();

        assert_snapshot!(spec, @r#"
        name ex
        bin ex
        cmd build {
            flag "-v --verbose"
        }
        cmd test {
            flag "-v --verbose"
        }
        "#);
    }

    #[test]
    fn two_includes_may_not_declare_the_same_name() {
        // Neither file is the nearer declaration, so there is nothing to prefer: a CLI
        // whose shared files have grown a collision hears about it here rather than at
        // whichever command happened to use the name.
        let dir = tempfile::tempdir().unwrap();
        for (file, flag) in [("a.usage.kdl", "--json"), ("b.usage.kdl", "--yaml")] {
            let body = format!("flagset \"output\" {{\n    flag \"{flag}\"\n}}\n");
            std::fs::write(dir.path().join(file), body).unwrap();
        }
        let root = dir.path().join("ex.usage.kdl");
        std::fs::write(
            &root,
            "bin \"ex\"\ninclude file=\"./a.usage.kdl\"\ninclude file=\"./b.usage.kdl\"\n",
        )
        .unwrap();

        let msg = err_file(&root);
        assert!(
            msg.contains("a flagset may be declared only once") && msg.contains("b.usage.kdl"),
            "{msg}"
        );
    }

    #[test]
    fn a_set_holds_flags_and_says_so_about_arguments() {
        let msg = err("flagset \"output\" {\n    arg \"<file>\"\n}\n");
        assert!(
            msg.contains("a flagset holds flags, not arguments"),
            "{msg}"
        );
    }

    #[test]
    fn a_set_rejects_what_it_has_no_meaning_for() {
        let msg = err("flagset \"output\" {\n    cmd \"nested\"\n}\n");
        assert!(msg.contains("unsupported flagset key cmd"), "{msg}");
        let msg = err("flagset \"output\" help=\"a set\" {\n    flag \"--json\"\n}\n");
        assert!(msg.contains("unsupported flagset prop help"), "{msg}");
    }

    #[test]
    fn a_use_names_sets_and_nothing_else() {
        let msg = err("use \"output\" {\n    flag \"--json\"\n}\n");
        assert!(
            msg.contains("`use` names flagsets and holds nothing"),
            "{msg}"
        );
        let msg = err("flagset \"o\" {\n    flag \"--json\"\n}\nuse from=\"o\"\n");
        assert!(msg.contains("expected 1.. arguments, got 0"), "{msg}");
    }
}
