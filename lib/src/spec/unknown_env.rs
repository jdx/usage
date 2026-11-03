use std::collections::BTreeSet;
use std::ffi::OsStr;

use serde::Serialize;

use crate::Spec;
use crate::error::Result;
use crate::kdl::{self, KdlNode};
use crate::spec::cmd::SpecCommand;
use crate::spec::context::ParsingContext;
use crate::spec::helpers::{NodeHelper, string_entry};

/// Refuse environment variables under the application's prefix that nothing declares.
///
/// `unknown_flags "error"` turns a misspelled flag into an error. A misspelled or stale
/// environment variable gets no such check by default — `MYAPP_LISTEN_TLS_CRET` is read by
/// nobody, and the program runs on its defaults. That failure is worst where nobody watches a
/// command line: systemd units, container manifests, Helm values.
///
/// ```kdl
/// unknown_env prefix="MOQ_" {
///     allow "MOQ_VAAPI_DEVICE" "MOQ_V4L2_ENCODER"
/// }
/// ```
///
/// The prefix only scopes the check. It never builds a name: every variable is still spelled
/// out in full where it is declared, so a name seen in an environment can be found by searching
/// for it.
///
/// A variable is known when any flag or argument anywhere in the command tree names it — as
/// `env`, an `env_fallback` or a `deprecated_env` — or when a `config` property reads it, or
/// when it is listed in `allow`. The whole tree counts, not only the command that ran: a
/// variable meant for `serve` is not a typo when the user runs `publish`. `allow` is for
/// variables read outside the spec, such as by a library.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct SpecUnknownEnv {
    /// Only variables whose names start with this are checked.
    pub prefix: String,
    /// Variables under the prefix that the spec does not declare but something still reads.
    pub allow: Vec<String>,
}

impl SpecUnknownEnv {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            allow: Vec::new(),
        }
    }

    /// A variable read outside the spec, which is therefore not a mistake.
    pub fn allow(mut self, name: impl Into<String>) -> Self {
        self.allow.push(name.into());
        self
    }

    pub(crate) fn parse(ctx: &ParsingContext, node: &NodeHelper<'_>) -> Result<Self> {
        node.ensure_arg_len(0..=0)?;
        node.ensure_props_unrepeated(&["prefix"])?;
        let mut decl = Self::default();
        for (key, value) in node.props() {
            match key {
                "prefix" => decl.prefix = value.ensure_string()?,
                key => bail_parse!(ctx, value.entry.span(), "unsupported unknown_env key {key}"),
            }
        }
        for child in node.children() {
            match child.name() {
                "allow" => {
                    // Names only. Anything else here was meant as a name and would otherwise be
                    // dropped, leaving a variable the author meant to allow refused at run time.
                    if let Some((key, value)) = child.props().into_iter().next() {
                        bail_parse!(ctx, value.entry.span(), "unsupported allow key {key}");
                    }
                    if !child.children().is_empty() {
                        bail_parse!(ctx, child.span(), "allow takes names, not child nodes");
                    }
                    for name in child.args() {
                        decl.allow.push(name.ensure_string()?);
                    }
                }
                key => bail_parse!(
                    ctx,
                    child.node.name().span(),
                    "unsupported unknown_env value {key}"
                ),
            }
        }
        // An empty prefix would make every variable in the environment this CLI's business,
        // `PATH` and `HOME` included.
        if decl.prefix.is_empty() {
            bail_parse!(
                ctx,
                node.span(),
                "unknown_env needs a non-empty prefix, as in unknown_env prefix=\"MYAPP_\""
            );
        }
        Ok(decl)
    }

    /// The variables under the prefix that `spec` neither declares nor allows, sorted.
    ///
    /// `names` is the environment's variable names. A name that is not valid UTF-8 cannot be
    /// one the spec declares, so one under the prefix is reported in its lossy spelling rather
    /// than skipped.
    pub fn unknown<'n>(
        &self,
        spec: &Spec,
        names: impl IntoIterator<Item = &'n OsStr>,
    ) -> Vec<String> {
        self.unknown_with(spec, &[], names, false)
    }

    /// [`Self::unknown`], also counting what `resolved` declares.
    ///
    /// A mount brings commands in at parse time, so the spec alone does not hold them. The
    /// parser passes the commands it walked, each with everything under it. `exact` compares
    /// names exactly on every platform, for a map that is read by exact key.
    pub(crate) fn unknown_with<'n>(
        &self,
        spec: &Spec,
        resolved: &[SpecCommand],
        names: impl IntoIterator<Item = &'n OsStr>,
        exact: bool,
    ) -> Vec<String> {
        let fold = cfg!(windows) && !exact;
        // An empty prefix owns nothing. The KDL reader refuses one; a policy built in code that
        // has one must not make `PATH` an error.
        if self.prefix.is_empty() {
            return Vec::new();
        }
        let mut known = declared_env_names(spec);
        for cmd in resolved {
            collect_cmd(cmd, &mut known);
        }
        let mut unknown: Vec<String> = names
            .into_iter()
            .filter(|name| {
                let lossy = name.to_string_lossy();
                // A name that is not UTF-8 is never a declared one, even when its lossy
                // spelling happens to equal a declaration that contains U+FFFD.
                // The prefix decides whose variable it is, which is the platform's question even
                // for a map read by exact key: `ex_ready` is this CLI's on Windows, and unknown.
                starts_with_env(&lossy, &self.prefix, cfg!(windows))
                    && name.to_str().is_none_or(|name| {
                        !known.iter().any(|k| same_env(k, name, fold))
                            && !self.allow.iter().any(|a| same_env(a, name, fold))
                    })
            })
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        // Each variable once, by the comparison that matched it: under folding `EX_TYPO` and
        // `ex_typo` are one variable, and listing it twice would read as two mistakes.
        unknown.sort_by_cached_key(|name| fold_env(name, fold).into_owned());
        unknown.dedup_by(|a, b| same_env(a, b, fold));
        unknown
    }
}

impl From<&SpecUnknownEnv> for KdlNode {
    fn from(decl: &SpecUnknownEnv) -> Self {
        let mut node = KdlNode::new("unknown_env");
        node.push(string_entry(Some("prefix"), &decl.prefix));
        if !decl.allow.is_empty() {
            let mut children = kdl::KdlDocument::new();
            let mut allow = KdlNode::new("allow");
            for name in &decl.allow {
                allow.push(string_entry(None, name));
            }
            children.nodes_mut().push(allow);
            node.set_children(children);
        }
        node
    }
}

/// Every environment variable `spec` reads: each flag's and argument's `env`, `env_fallback`
/// and `deprecated_env` across the whole command tree, clauses included, and every `config`
/// property's.
pub fn declared_env_names(spec: &Spec) -> BTreeSet<&str> {
    let mut names = BTreeSet::new();
    collect_cmd(&spec.cmd, &mut names);
    for prop in spec.config.props.values() {
        names.extend(prop.env.iter().map(String::as_str));
        names.extend(prop.envs.iter().map(String::as_str));
        names.extend(prop.deprecated_envs.iter().map(String::as_str));
    }
    names
}

fn collect_cmd<'a>(cmd: &'a SpecCommand, names: &mut BTreeSet<&'a str>) {
    for flag in &cmd.flags {
        names.extend(flag.env_names());
    }
    for arg in &cmd.args {
        names.extend(arg.env_names());
    }
    if let Some(clause) = &cmd.clause {
        for flag in &clause.flags {
            names.extend(flag.env_names());
        }
        for arg in &clause.args {
            names.extend(arg.env_names());
        }
    }
    for sub in cmd.subcommands.values() {
        collect_cmd(sub, names);
    }
}

/// A name as Windows compares it, when `fold` says to.
///
/// Windows variable names are case-insensitive — `std::env::var("PATH")` finds `Path` — so a
/// variable set as `Moq_Port` is the one a spec declares as `MOQ_PORT`. Windows folds one
/// character at a time, `ä` with `Ä` but `ß` never with `SS`, so a character whose uppercase is
/// longer than itself is kept as it is rather than expanded.
fn fold_env(name: &str, fold: bool) -> std::borrow::Cow<'_, str> {
    if !fold {
        return name.into();
    }
    name.chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect::<String>()
        .into()
}

fn same_env(a: &str, b: &str, fold: bool) -> bool {
    fold_env(a, fold) == fold_env(b, fold)
}

/// Folding keeps one character for one, so a folded prefix is a prefix of the folded name
/// exactly when the original is.
fn starts_with_env(name: &str, prefix: &str, fold: bool) -> bool {
    fold_env(name, fold).starts_with(&*fold_env(prefix, fold))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unknown(spec: &str, env: &[&str]) -> Vec<String> {
        let spec: Spec = spec.parse().unwrap();
        spec.unknown_env
            .as_ref()
            .expect("the spec declares unknown_env")
            .unknown(&spec, env.iter().map(OsStr::new))
    }

    #[test]
    fn the_whole_tree_and_every_kind_of_name_is_known() {
        let spec = r#"
            name "ex"
            bin "ex"
            unknown_env prefix="EX_" {
                allow "EX_LIBRARY"
            }
            flag "--port <port>" env="EX_PORT" env_fallback="EX_OLD_PORT"
            cmd "serve" {
                flag "--tls" env="EX_TLS" deprecated_env="EX_SSL"
                arg "[dir]" env="EX_DIR"
            }
            config {
                prop "jobs" env="EX_JOBS"
            }
        "#;
        let env = [
            "EX_PORT",
            "EX_OLD_PORT",
            "EX_TLS",
            "EX_SSL",
            "EX_DIR",
            "EX_JOBS",
            "EX_LIBRARY",
            "HOME",
            "EXTRA",
        ];
        assert_eq!(unknown(spec, &env), Vec::<String>::new());
        let env = ["EX_PROT", "EX_TSL", "EX_PORT", "EX_TSL"];
        assert_eq!(unknown(spec, &env), ["EX_PROT", "EX_TSL"]);
    }

    #[test]
    fn round_trips_through_kdl() {
        let spec: Spec =
            "name \"ex\"\nunknown_env prefix=\"EX_\" {\n    allow \"EX_A\" \"EX_B\"\n}\n"
                .parse()
                .unwrap();
        let decl = spec.unknown_env.clone().unwrap();
        assert_eq!(decl.prefix, "EX_");
        assert_eq!(decl.allow, ["EX_A", "EX_B"]);
        let kdl = spec.to_string();
        assert!(kdl.contains("unknown_env prefix=EX_"), "{kdl}");
        let reparsed: Spec = kdl.parse().unwrap();
        assert_eq!(reparsed.unknown_env, Some(decl));
    }

    #[test]
    fn a_prefix_is_required() {
        let err = "name \"ex\"\nunknown_env\n".parse::<Spec>().unwrap_err();
        assert!(format!("{err:?}").contains("prefix"), "{err:?}");
        let err = "name \"ex\"\nunknown_env prefix=\"EX_\" bogus=\"x\"\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("bogus"), "{err:?}");
        let err = "name \"ex\"\nunknown_env prefix=\"EX_\" {\n    allow name=\"EX_A\"\n}\n"
            .parse::<Spec>()
            .unwrap_err();
        assert!(format!("{err:?}").contains("allow key name"), "{err:?}");
    }

    #[test]
    fn an_empty_prefix_built_in_code_owns_nothing() {
        let spec: Spec = "name \"ex\"\n".parse().unwrap();
        let decl = SpecUnknownEnv::default();
        assert!(decl.unknown(&spec, [OsStr::new("PATH")]).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_is_reported_not_matched() {
        use std::os::unix::ffi::OsStrExt;
        // A declaration containing U+FFFD, which is what the lossy spelling of the
        // invalid byte becomes.
        let spec: Spec =
            "name \"ex\"\nunknown_env prefix=\"EX_\"\nflag \"--x <x>\" env=\"EX_\u{fffd}\"\n"
                .parse()
                .unwrap();
        let decl = spec.unknown_env.clone().unwrap();
        let name = OsStr::from_bytes(b"EX_\xff");
        assert_eq!(decl.unknown(&spec, [name]), ["EX_\u{fffd}"]);
    }
}
