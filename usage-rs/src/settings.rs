//! Flag-declared settings as registry metadata, for the code `#[derive(Cli)]` generates.
//!
//! A field with `setting = "key"` already says what the setting is: its type, its environment
//! names, its help, and the flags that set it. Each command describes that in
//! `usage_argv::spec::SettingDecl`, which knows nothing of `usage-config`, and the root hands the
//! joined table here to become `usage_config::PropMeta`. This crate is the only one that sees both
//! vocabularies, which is why the conversion lives in it rather than in either of them.
//!
//! Everything is a `const fn`, because a registry is a `static` with no initializer to run.

use usage_argv::spec::{SettingDecl, SettingSplit, SettingTy};
use usage_config::{Parser, PropMeta, Ty};

/// How many settings `decls` declares that `explicit` does not.
///
/// A key declared by several flags is one setting. A key a `usage::Config` struct also declares is
/// that struct's: the explicit declaration can say what a flag cannot — a default, a merge, a
/// scope — so it wins, and a CLI adopting flag declarations keeps compiling unchanged.
pub const fn prop_count(decls: &[SettingDecl], explicit: &[PropMeta]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < decls.len() {
        if owns(decls, i, explicit) {
            count += 1;
        }
        i += 1;
    }
    count
}

/// How many distinct flags set the settings [`prop_count`] counts.
pub const fn flag_count(
    decls: &[SettingDecl],
    bindings: &[(&'static str, &'static str)],
    explicit: &[PropMeta],
) -> usize {
    let mut count = 0;
    let mut b = 0;
    while b < bindings.len() {
        // Only a binding a declaration backs: a hand-written command may bind a key it leaves
        // to a registry of its own, and `flags` writes nothing for it.
        if !declares(explicit, bindings[b].1)
            && decl_for(decls, bindings[b].1)
            && first_binding(bindings, b)
        {
            count += 1;
        }
        b += 1;
    }
    count
}

/// Every flag counted by [`flag_count`], grouped by setting in [`props`] order.
///
/// So each setting's `cli` is a run of this array — a slice of a constant, which a `const fn`
/// can hand out where it could not build a new one per setting.
pub const fn flags<const F: usize>(
    decls: &[SettingDecl],
    bindings: &[(&'static str, &'static str)],
    explicit: &[PropMeta],
) -> [&'static str; F] {
    let mut out = [""; F];
    let mut at = 0;
    let mut i = 0;
    while i < decls.len() {
        if owns(decls, i, explicit) {
            let mut b = 0;
            while b < bindings.len() {
                // The key first: `first_binding` rescans what came before, and doing that for
                // every binding of every setting is quadratic in a CLI's whole binding table —
                // enough, in a mise-sized one, to hit rustc's limit on const evaluation.
                if str_eq(bindings[b].1, decls[i].key) && first_binding(bindings, b) {
                    out[at] = bindings[b].0;
                    at += 1;
                }
                b += 1;
            }
        }
        i += 1;
    }
    assert!(at == F, "`F` must be the number of distinct bound flags");
    out
}

/// The settings `decls` declares that `explicit` does not, with `flags` from [`flags`].
pub const fn props<const P: usize>(
    decls: &[SettingDecl],
    bindings: &[(&'static str, &'static str)],
    explicit: &[PropMeta],
    flags: &'static [&'static str],
) -> [PropMeta; P] {
    let mut out = [PropMeta::new("", Ty::Any); P];
    let mut at = 0;
    let mut flag_at = 0;
    let mut i = 0;
    while i < decls.len() {
        if owns(decls, i, explicit) {
            let decl = merged(decls, i);
            let mut flag_len = 0;
            let mut b = 0;
            while b < bindings.len() {
                if str_eq(bindings[b].1, decl.key) && first_binding(bindings, b) {
                    flag_len += 1;
                }
                b += 1;
            }
            let (_, rest) = flags.split_at(flag_at);
            let (cli, _) = rest.split_at(flag_len);
            flag_at += flag_len;
            out[at] = prop(&decl, cli);
            at += 1;
        }
        i += 1;
    }
    assert!(at == P, "`P` must be the number of flag-declared settings");
    out
}

/// One setting's metadata.
///
/// Optional always: a flag-declared setting has no registry default — the field's own default is
/// text the parser applies, not a typed value a `const` can hold — so absence is a state the
/// reader has to handle. A setting that needs a registry default declares it on a `usage::Config`
/// field, which takes the key over.
const fn prop(decl: &SettingDecl, cli: &'static [&'static str]) -> PropMeta {
    let parse = match (decl.list, decl.split) {
        (true, Some(SettingSplit::Comma)) => Some(Parser::ListByComma),
        (true, Some(SettingSplit::Colon)) => Some(Parser::ListByColon),
        (true, Some(SettingSplit::Other)) => panic!(
            "a flag-declared list setting splits on `,` or `:`, which is what a registry can \
             split the environment on: use one of those as the `delimiter`, or declare the \
             setting on a `usage::Config` field"
        ),
        _ => None,
    };
    PropMeta {
        parse,
        envs: decl.envs,
        deprecated_envs: decl.deprecated_envs,
        cli,
        optional: Some(true),
        help: decl.help,
        long_help: decl.long_help,
        ..PropMeta::new(decl.key, ty(decl.ty, decl.list))
    }
}

const fn ty(ty: SettingTy, list: bool) -> Ty {
    match (ty, list) {
        (SettingTy::Bool, false) => Ty::Bool,
        (SettingTy::Int, false) => Ty::Int,
        (SettingTy::Uint, false) => Ty::Uint,
        (SettingTy::Float, false) => Ty::Float,
        (SettingTy::String, false) => Ty::String,
        (SettingTy::Path, false) => Ty::Path,
        (SettingTy::Bool, true) => Ty::List(&Ty::Bool),
        (SettingTy::Int, true) => Ty::List(&Ty::Int),
        (SettingTy::Uint, true) => Ty::List(&Ty::Uint),
        (SettingTy::Float, true) => Ty::List(&Ty::Float),
        (SettingTy::String, true) => Ty::List(&Ty::String),
        (SettingTy::Path, true) => Ty::List(&Ty::Path),
    }
}

/// Declaration `i` folded with every later one for the same key.
///
/// They have to agree on what the setting holds — two flags reading one key as different types is
/// a registry that cannot describe both. Anything one of them leaves out, another may say.
const fn merged(decls: &[SettingDecl], i: usize) -> SettingDecl {
    let mut decl = decls[i];
    let mut j = i + 1;
    while j < decls.len() {
        let other = &decls[j];
        if str_eq(other.key, decl.key) {
            assert!(
                same_ty(other.ty, decl.ty) && other.list == decl.list,
                "two flags set the same setting and read it as different types: give them the \
                 same field type, or declare the setting on a `usage::Config` field"
            );
            // A `const fn` cannot build a new list from two, and keeping one would leave the
            // other command's variables unread. So two lists have to be the same list.
            assert!(
                decl.envs.is_empty() || other.envs.is_empty() || strs_eq(decl.envs, other.envs),
                "two flags set the same setting and name different environment variables: give \
                 them the same `env`, or declare the setting on a `usage::Config` field"
            );
            assert!(
                decl.deprecated_envs.is_empty()
                    || other.deprecated_envs.is_empty()
                    || strs_eq(decl.deprecated_envs, other.deprecated_envs),
                "two flags set the same setting and name different deprecated environment \
                 variables: give them the same `deprecated_env`, or declare the setting on a \
                 `usage::Config` field"
            );
            // One registry parser splits every source's text, so two list flags that split
            // differently — including one that does not split at all — would have one of them
            // read differently from the environment than from its own flag.
            assert!(
                !decl.list || same_split(decl.split, other.split),
                "two flags set the same setting and split it differently: give them the same \
                 `delimiter`, or declare the setting on a `usage::Config` field"
            );
            if decl.envs.is_empty() {
                decl.envs = other.envs;
            }
            if decl.deprecated_envs.is_empty() {
                decl.deprecated_envs = other.deprecated_envs;
            }
            if decl.split.is_none() {
                decl.split = other.split;
            }
            if decl.help.is_none() {
                decl.help = other.help;
            }
            if decl.long_help.is_none() {
                decl.long_help = other.long_help;
            }
        }
        j += 1;
    }
    decl
}

/// Whether declaration `i` is the first for its key, and not a key `explicit` declares.
const fn owns(decls: &[SettingDecl], i: usize, explicit: &[PropMeta]) -> bool {
    if declares(explicit, decls[i].key) {
        return false;
    }
    let mut j = 0;
    while j < i {
        if str_eq(decls[j].key, decls[i].key) {
            return false;
        }
        j += 1;
    }
    true
}

/// Whether binding `b` is the first of its flag and key — the same flag on two subcommands is one
/// spelling of the setting.
const fn first_binding(bindings: &[(&'static str, &'static str)], b: usize) -> bool {
    let mut j = 0;
    while j < b {
        if str_eq(bindings[j].0, bindings[b].0) && str_eq(bindings[j].1, bindings[b].1) {
            return false;
        }
        j += 1;
    }
    true
}

const fn declares(explicit: &[PropMeta], key: &str) -> bool {
    let mut i = 0;
    while i < explicit.len() {
        if str_eq(explicit[i].key, key) {
            return true;
        }
        i += 1;
    }
    false
}

const fn decl_for(decls: &[SettingDecl], key: &str) -> bool {
    let mut i = 0;
    while i < decls.len() {
        if str_eq(decls[i].key, key) {
            return true;
        }
        i += 1;
    }
    false
}

const fn strs_eq(a: &[&str], b: &[&str]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if !str_eq(a[i], b[i]) {
            return false;
        }
        i += 1;
    }
    true
}

const fn same_split(a: Option<SettingSplit>, b: Option<SettingSplit>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a as u8 == b as u8,
        (None, None) => true,
        _ => false,
    }
}

const fn same_ty(a: SettingTy, b: SettingTy) -> bool {
    a as u8 == b as u8
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOBS: SettingDecl = SettingDecl {
        envs: &["EX_JOBS"],
        ..SettingDecl::new("jobs", SettingTy::Uint)
    };

    #[test]
    fn a_binding_no_declaration_backs_is_left_to_its_own_registry() {
        // A hand-written command can bind a key and inherit the empty `SETTING_DECLS`; counting
        // its flag would make `flags` come up short and the root fail to compile.
        let bindings = [("--jobs", "jobs"), ("--theme", "theme")];
        assert_eq!(flag_count(&[JOBS], &bindings, &[]), 1);
        assert_eq!(flags::<1>(&[JOBS], &bindings, &[]), ["--jobs"]);
    }

    #[test]
    fn the_same_environment_names_fold_into_one_setting() {
        let other = SettingDecl {
            help: Some("jobs"),
            ..JOBS
        };
        let decl = merged(&[JOBS, other], 0);
        assert_eq!(decl.envs, ["EX_JOBS"]);
        assert_eq!(decl.help, Some("jobs"));
    }

    #[test]
    #[should_panic(expected = "split it differently")]
    fn a_list_that_splits_beside_one_that_does_not_is_refused() {
        let comma = SettingDecl {
            list: true,
            split: Some(SettingSplit::Comma),
            ..JOBS
        };
        let whole = SettingDecl {
            split: None,
            ..comma
        };
        merged(&[comma, whole], 0);
    }

    #[test]
    #[should_panic(expected = "split it differently")]
    fn different_delimiters_for_one_setting_are_refused() {
        let comma = SettingDecl {
            list: true,
            split: Some(SettingSplit::Comma),
            ..JOBS
        };
        let colon = SettingDecl {
            split: Some(SettingSplit::Colon),
            ..comma
        };
        merged(&[comma, colon], 0);
    }

    #[test]
    #[should_panic(expected = "splits on `,` or `:`")]
    fn a_delimiter_no_registry_parser_splits_on_is_refused() {
        let semicolon = SettingDecl {
            list: true,
            split: Some(SettingSplit::Other),
            ..JOBS
        };
        props::<1>(&[semicolon], &[("--jobs", "jobs")], &[], &["--jobs"]);
    }

    #[test]
    #[should_panic(expected = "name different environment variables")]
    fn different_environment_names_for_one_setting_are_refused() {
        let other = SettingDecl {
            envs: &["EX_CHECK_JOBS"],
            ..JOBS
        };
        merged(&[JOBS, other], 0);
    }
}
