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
    _decls: &[SettingDecl],
    bindings: &[(&'static str, &'static str)],
    explicit: &[PropMeta],
) -> usize {
    let mut count = 0;
    let mut b = 0;
    while b < bindings.len() {
        if first_binding(bindings, b) && !declares(explicit, bindings[b].1) {
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
                if first_binding(bindings, b) && str_eq(bindings[b].1, decls[i].key) {
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
                if first_binding(bindings, b) && str_eq(bindings[b].1, decl.key) {
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
