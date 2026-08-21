//! `#[derive(usage::Config)]`: a settings struct as the declaration of its own registry.
//!
//! The fleet's pattern — a `settings.toml` registry, a `build.rs` generator, and a settings
//! struct the generator emits — keeps three descriptions of every setting in step by hand.
//! This derive collapses them to one: the author writes the struct the CLI already holds its
//! settings in, and the derive generates the `usage_config` registry, the reader that fills
//! the struct from a resolution, and the spec `config` block that documents it. There is no
//! second declaration left to drift from.
//!
//! ```ignore
//! /// How this tool behaves, resolved from flags, the environment, and files.
//! #[derive(usage::Config)]
//! struct Settings {
//!     /// How many jobs to run at once
//!     #[usage(env = "EX_JOBS", default = 4, cli("--jobs", "-j"))]
//!     jobs: u64,
//!
//!     /// Paths to leave alone
//!     #[usage(env = "EX_EXCLUDE", merge = "union", parse = "list_by_comma")]
//!     exclude: Option<Vec<String>>,
//!
//!     /// Where the cache lives
//!     #[usage(env = "EX_CACHE_DIR", default_fn = default_cache_dir,
//!             default_note = "under the user cache directory")]
//!     cache_dir: std::path::PathBuf,
//!
//!     #[usage(flatten)]
//!     task: TaskSettings,
//! }
//!
//! /// The `task.*` settings.
//! #[derive(usage::Config)]
//! #[usage(prefix = "task")]
//! struct TaskSettings {
//!     /// How task output is interleaved
//!     #[usage(default = "prefix", choices("prefix", "interleave"))]
//!     output: String,
//! }
//! ```
//!
//! The struct's field types are the declaration's types: `bool`, integers, `String`,
//! `PathBuf`, `Vec<T>`, `BTreeMap<String, T>`, and `Option<T>` for a setting that may be
//! absent. A field is read with `usage_config::FromValue`, so a type this table does not
//! name can still be a field by saying what the spec should call it: `ty = "duration"` on a
//! `String` field holds a span of time the way the registry declares one.
//!
//! Nesting composes through `usage_config::Props`, the way `usage::Cli` composes flattened
//! groups: a child declares its own props (usually under a `prefix`), and the parent joins
//! the slices at compile time. Two groups declaring the same key are refused at compile
//! time by the join.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, UnOp};

use crate::crate_name::{crate_name, FoundCrate};
use crate::model::{attrs, doc_comment, flag_value, ident_of, nested, string_value};

/// The config crate as the adopter depended on it.
///
/// The same resolution the other generated paths use: a direct `usage-config` dependency
/// wins, otherwise the `usage-rs` facade provides it as `usage::config`, and the bare name
/// is left to produce the useful compiler error when neither was declared.
pub(crate) fn config_path() -> TokenStream {
    match crate_name("usage-config") {
        Ok(FoundCrate::Itself) => quote!(::usage_config),
        Ok(FoundCrate::Name(name)) => {
            let config = format_ident!("{}", name.replace('-', "_"));
            quote!(::#config)
        }
        _ => match crate_name("usage-rs") {
            Ok(FoundCrate::Itself) => quote!(::usage_rs::config),
            Ok(FoundCrate::Name(name)) => {
                let facade = format_ident!("{}", name.replace('-', "_"));
                quote!(::#facade::config)
            }
            _ => quote!(::usage_config),
        },
    }
}

/// The declaration, read from the struct.
pub struct Config {
    ident: syn::Ident,
    fields: Vec<Field>,
}

enum Field {
    Prop(Box<Prop>),
    /// Another `Config` struct whose props follow this position in the joined registry.
    Flatten {
        ident: syn::Ident,
        ty: Box<syn::Type>,
    },
}

struct Prop {
    ident: syn::Ident,
    key: String,
    ty: Ty,
    /// The type the field holds and the fold reads, `Option` peeled.
    read_ty: syn::Type,
    /// Whether the field is an `Option<T>` — absence is a legitimate state.
    optional_field: bool,
    default: Option<Const>,
    /// A runtime default: `fn() -> T`, applied after the fold when no layer supplied one.
    default_fn: Option<Expr>,
    default_note: Option<String>,
    envs: Vec<String>,
    deprecated_envs: Vec<String>,
    cli: Vec<String>,
    aliases: Vec<String>,
    /// `(kind, key)` pairs, in declaration order.
    bindings: Vec<(String, String)>,
    choices: Vec<Const>,
    merge: Option<Merge>,
    scope: Option<Scope>,
    parse: Option<String>,
    hide: bool,
    deprecated: Option<String>,
    deprecated_warn_at: Option<String>,
    deprecated_remove_at: Option<String>,
    since: Option<String>,
    examples: Vec<String>,
    help: Option<String>,
    long_help: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Merge {
    Union,
    Deep,
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Global,
    Env,
}

/// The declared type, mirroring `usage_config::Ty`.
#[derive(Clone, PartialEq)]
enum Ty {
    Bool,
    Int,
    Uint,
    Float,
    String,
    Path,
    Url,
    Duration,
    Object,
    Any,
    List(Box<Ty>),
    Set(Box<Ty>),
    Map(Box<Ty>),
}

impl Ty {
    fn tokens(&self, config: &TokenStream) -> TokenStream {
        match self {
            Self::Bool => quote!(#config::Ty::Bool),
            Self::Int => quote!(#config::Ty::Int),
            Self::Uint => quote!(#config::Ty::Uint),
            Self::Float => quote!(#config::Ty::Float),
            Self::String => quote!(#config::Ty::String),
            Self::Path => quote!(#config::Ty::Path),
            Self::Url => quote!(#config::Ty::Url),
            Self::Duration => quote!(#config::Ty::Duration),
            Self::Object => quote!(#config::Ty::Object),
            Self::Any => quote!(#config::Ty::Any),
            Self::List(inner) => {
                let inner = inner.tokens(config);
                quote!(#config::Ty::List(&#inner))
            }
            Self::Set(inner) => {
                let inner = inner.tokens(config);
                quote!(#config::Ty::Set(&#inner))
            }
            Self::Map(inner) => {
                let inner = inner.tokens(config);
                quote!(#config::Ty::Map(&#inner))
            }
        }
    }

    /// Whether values of this type are a collection, which is what `merge` policies and
    /// named parsers are about.
    fn is_collection(&self) -> bool {
        matches!(
            self,
            Self::List(_) | Self::Set(_) | Self::Map(_) | Self::Object
        )
    }

    /// Whether `value` could be a value of this type, where the declaration puts it.
    ///
    /// Permissive exactly where something coerces, which is why the position matters: the
    /// merge coerces what a *layer* supplies, and `Registry`'s choice comparison coerces
    /// before it compares — but nothing coerces a declared default.
    fn admits(&self, value: &Const, position: Position) -> bool {
        let coerced = position == Position::Choice;
        match (self, value) {
            (Self::Any, _) => true,
            (Self::Bool, Const::Bool(_)) => true,
            (Self::Int | Self::Float, Const::Int(_)) => true,
            // A `uint` names a non-negative number, so `default = -1` on a `u64` field
            // compiled and then failed every `read` with a type error the author could do
            // nothing about. The span is here, so refuse it here.
            (Self::Uint, Const::Int(i)) => *i >= 0,
            (Self::Float, Const::Float(_)) => true,
            (Self::String | Self::Path | Self::Url | Self::Duration, Const::Str(_)) => true,
            // A string type reads a bare number or boolean as its text — where something
            // reads it. A default is handed to the field as `Value::Int(1)`, and `String`
            // refuses that, so `default = 1` on a `String` field is a mistake and not a
            // shorthand.
            (
                Self::String | Self::Path | Self::Url | Self::Duration,
                Const::Bool(_) | Const::Int(_) | Const::Float(_),
            ) => coerced,
            (Self::List(item) | Self::Set(item), Const::List(items)) => {
                items.iter().all(|value| item.admits(value, position))
            }
            // A choice on a list setting names what one *item* may be, which is how the
            // registry compares it. A default is the whole value, and `default(80)` is how
            // the attribute already spells a list of one — so a bare `default = 80` on a
            // `Vec<u64>` is refused rather than quietly meaning something else.
            (Self::List(item) | Self::Set(item), scalar) => {
                coerced && item.admits(scalar, position)
            }
            _ => false,
        }
    }
}

/// Where in a declaration a constant stands, which decides how strictly it is read.
#[derive(Clone, Copy, PartialEq)]
enum Position {
    /// A declared default. The resolver seeds it with `Const::to_value` and hands it to the
    /// field as it stands, so it has to already be a value of the field's type.
    Default,
    /// One of the values a setting allows, compared against a resolved value *after* the
    /// merge has coerced both.
    Choice,
}

/// A literal a registry can hold as a `const`.
#[derive(Clone, PartialEq)]
enum Const {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Const>),
}

impl Const {
    fn tokens(&self, config: &TokenStream) -> TokenStream {
        match self {
            Self::Bool(b) => quote!(#config::Const::Bool(#b)),
            Self::Int(i) => quote!(#config::Const::Int(#i)),
            Self::Float(f) => quote!(#config::Const::Float(#f)),
            Self::Str(s) => quote!(#config::Const::Str(#s)),
            Self::List(items) => {
                let items = items.iter().map(|item| item.tokens(config));
                quote!(#config::Const::List(&[#(#items),*]))
            }
        }
    }
}

impl Config {
    pub fn from_input(input: &DeriveInput) -> syn::Result<Self> {
        let Data::Struct(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "usage::Config describes a settings struct, so it needs a struct with named \
                 fields",
            ));
        };
        let Fields::Named(named) = &data.fields else {
            return Err(syn::Error::new_spanned(
                &data.fields,
                "usage::Config reads settings into named fields; a tuple or unit struct has \
                 nowhere to put them",
            ));
        };
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "usage::Config does not support generic parameters: the generated registry \
                 is `const` and every field has to be a concrete type a value can be read \
                 into",
            ));
        }

        let mut prefix = None;
        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                match ident_of(meta.path()).as_str() {
                    "prefix" => prefix = Some(string_value(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            meta.path(),
                            format!("usage::Config does not understand `{other}` on the struct"),
                        ));
                    }
                }
            }
        }

        let mut fields = Vec::new();
        for field in &named.named {
            fields.push(Field::from_field(field, prefix.as_deref())?);
        }

        // A duplicate within one struct is visible here, so it is refused here, with spans.
        // Duplicates across flattened structs are refused by `concat_props` at compile time.
        let mut keys: Vec<(&String, &syn::Ident)> = Vec::new();
        for field in &fields {
            if let Field::Prop(prop) = field {
                if let Some((_, first)) = keys.iter().find(|(key, _)| **key == prop.key) {
                    return Err(syn::Error::new(
                        prop.ident.span(),
                        format!(
                            "`{}` and `{first}` declare the same setting key `{}`",
                            prop.ident, prop.key
                        ),
                    ));
                }
                keys.push((&prop.key, &prop.ident));
            }
        }

        Ok(Self {
            ident: input.ident.clone(),
            fields,
        })
    }
}

impl Field {
    fn from_field(field: &syn::Field, prefix: Option<&str>) -> syn::Result<Self> {
        let ident = field.ident.clone().expect("named fields only");

        let mut flatten = false;
        let mut key_attr = None;
        let mut ty_attr = None;
        let mut explicit_optional = None;
        let mut prop = Prop {
            ident: ident.clone(),
            key: String::new(),
            ty: Ty::Any,
            read_ty: field.ty.clone(),
            optional_field: false,
            default: None,
            default_fn: None,
            default_note: None,
            envs: Vec::new(),
            deprecated_envs: Vec::new(),
            cli: Vec::new(),
            aliases: Vec::new(),
            bindings: Vec::new(),
            choices: Vec::new(),
            merge: None,
            scope: None,
            parse: None,
            hide: false,
            deprecated: None,
            deprecated_warn_at: None,
            deprecated_remove_at: None,
            since: None,
            examples: Vec::new(),
            help: None,
            long_help: None,
        };
        (prop.help, prop.long_help) = doc_comment(&field.attrs, false)?;

        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                let name = ident_of(meta.path());
                match name.as_str() {
                    "flatten" => flatten = flag_value(&meta)?,
                    "key" => key_attr = Some(string_value(&meta)?),
                    "ty" => ty_attr = Some((string_value(&meta)?, meta.path().span())),
                    "env" => prop.envs.extend(strings(&meta)?),
                    "deprecated_env" => prop.deprecated_envs.extend(strings(&meta)?),
                    "cli" => prop.cli.extend(strings(&meta)?),
                    "alias" | "aliases" => prop.aliases.extend(strings(&meta)?),
                    "example" | "examples" => prop.examples.extend(strings(&meta)?),
                    "source" => {
                        let mut words = strings(&meta)?;
                        if words.len() < 2 {
                            return Err(syn::Error::new_spanned(
                                &meta,
                                "`source` takes a kind and at least one key, as in \
                                 `source(\"git\", \"hk.jobs\")`",
                            ));
                        }
                        let kind = words.remove(0);
                        prop.bindings
                            .extend(words.into_iter().map(|key| (kind.clone(), key)));
                    }
                    "default" => prop.default = Some(const_value(&meta)?),
                    "default_fn" => {
                        prop.default_fn = Some(meta.require_name_value()?.value.clone());
                    }
                    "default_note" => prop.default_note = Some(string_value(&meta)?),
                    "choices" => prop.choices = consts(&meta)?,
                    "merge" => {
                        prop.merge = Some(match string_value(&meta)?.as_str() {
                            "union" => Merge::Union,
                            "deep" => Merge::Deep,
                            "replace" => {
                                return Err(syn::Error::new_spanned(
                                    &meta,
                                    "`replace` is the default; say nothing instead",
                                ))
                            }
                            other => {
                                return Err(syn::Error::new_spanned(
                                    &meta,
                                    format!("`merge` is `union` or `deep`, not `{other}`"),
                                ))
                            }
                        });
                    }
                    "scope" => {
                        prop.scope = Some(match string_value(&meta)?.as_str() {
                            "global" => Scope::Global,
                            "env" => Scope::Env,
                            other => {
                                return Err(syn::Error::new_spanned(
                                    &meta,
                                    format!("`scope` is `global` or `env`, not `{other}`"),
                                ))
                            }
                        });
                    }
                    "parse" => {
                        let parse = string_value(&meta)?;
                        if !matches!(
                            parse.as_str(),
                            "list_by_comma"
                                | "list_by_colon"
                                | "list_by_os_path_separator"
                                | "set_by_comma"
                        ) {
                            return Err(syn::Error::new_spanned(
                                &meta,
                                format!("`{parse}` is not a parser the spec names"),
                            ));
                        }
                        prop.parse = Some(parse);
                    }
                    "hide" => prop.hide = flag_value(&meta)?,
                    "optional" => explicit_optional = Some(flag_value(&meta)?),
                    "deprecated" => prop.deprecated = Some(string_value(&meta)?),
                    "deprecated_warn_at" => prop.deprecated_warn_at = Some(string_value(&meta)?),
                    "deprecated_remove_at" => {
                        prop.deprecated_remove_at = Some(string_value(&meta)?)
                    }
                    "since" => prop.since = Some(string_value(&meta)?),
                    "help" => prop.help = Some(string_value(&meta)?),
                    "long_help" => prop.long_help = Some(string_value(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            meta.path(),
                            format!("usage::Config does not understand `{other}`"),
                        ));
                    }
                }
            }
        }

        if flatten {
            // A flattened group is another declaration, not a prop: everything about its
            // settings belongs on its own fields.
            let described = [
                (!prop.envs.is_empty(), "env"),
                (prop.default.is_some(), "default"),
                (prop.default_fn.is_some(), "default_fn"),
                (!prop.cli.is_empty(), "cli"),
                (!prop.bindings.is_empty(), "source"),
                (!prop.choices.is_empty(), "choices"),
                (prop.merge.is_some(), "merge"),
                (prop.scope.is_some(), "scope"),
                (prop.parse.is_some(), "parse"),
                (prop.hide, "hide"),
                (key_attr.is_some(), "key"),
                (ty_attr.is_some(), "ty"),
                (!prop.deprecated_envs.is_empty(), "deprecated_env"),
                (!prop.aliases.is_empty(), "alias"),
                (!prop.examples.is_empty(), "example"),
                (prop.default_note.is_some(), "default_note"),
                (explicit_optional.is_some(), "optional"),
                (prop.deprecated.is_some(), "deprecated"),
                (prop.deprecated_warn_at.is_some(), "deprecated_warn_at"),
                (prop.deprecated_remove_at.is_some(), "deprecated_remove_at"),
                (prop.since.is_some(), "since"),
            ];
            if let Some((_, what)) = described.iter().find(|(given, _)| *given) {
                return Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "`{what}` describes a setting, and `flatten` says this field is a \
                             group of them: put it on the group's own fields"
                    ),
                ));
            }
            return Ok(Self::Flatten {
                ident,
                ty: Box::new(field.ty.clone()),
            });
        }

        let unraw = ident.to_string();
        let unraw = unraw.strip_prefix("r#").unwrap_or(&unraw);
        prop.key = match key_attr {
            Some(key) => key,
            None => unraw.to_string(),
        };
        if let Some(prefix) = prefix {
            prop.key = format!("{prefix}.{}", prop.key);
        }

        // The field's type is the declaration's type. `Option<T>` peels to T and marks the
        // setting optional; a `ty` override renames what the spec calls it without changing
        // what the field holds.
        let (optional_field, inner) = peel_option(&field.ty);
        prop.optional_field = optional_field;
        prop.read_ty = inner.clone();
        prop.ty = match ty_attr {
            Some((name, span)) => parse_ty_name(&name)
                .ok_or_else(|| syn::Error::new(span, format!("`{name}` is not a spec type")))?,
            None => infer_ty(&inner).ok_or_else(|| {
                syn::Error::new(
                    inner.span(),
                    "this type does not name a spec type on its own; say what the spec \
                     should call it, as in `ty = \"duration\"` on a String field",
                )
            })?,
        };

        if let Some(optional) = explicit_optional {
            if optional != prop.optional_field {
                return Err(syn::Error::new(
                    ident.span(),
                    "`optional` and the field's type disagree: `Option<T>` is how a field \
                     says a setting may be absent",
                ));
            }
        }
        if prop.optional_field && prop.default.is_some() {
            return Err(syn::Error::new(
                ident.span(),
                "a setting with a `default` always has a value, so the field cannot be an \
                 `Option`: give it the inner type",
            ));
        }
        if prop.default.is_some() && prop.default_fn.is_some() {
            return Err(syn::Error::new(
                ident.span(),
                "`default` and `default_fn` are two answers to the same question; declare one",
            ));
        }
        if prop.optional_field && prop.default_fn.is_some() {
            return Err(syn::Error::new(
                ident.span(),
                "a setting with a `default_fn` always has a value, so the field cannot be an \
                 `Option`: give it the inner type",
            ));
        }
        if let Some(default) = &prop.default {
            if !prop.ty.admits(default, Position::Default) {
                return Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "the default is not a value `{}` can hold. Nothing coerces a \
                         default — the resolver seeds it as written — so write it as the \
                         type the field holds",
                        type_name(&prop.ty)
                    ),
                ));
            }
            if !prop.choices.is_empty() && !prop.choices.iter().any(|choice| choice == default) {
                return Err(syn::Error::new(
                    ident.span(),
                    "the default is not one of the declared choices",
                ));
            }
        }
        if !prop.choices.is_empty() && prop.ty == Ty::Bool {
            return Err(syn::Error::new(
                ident.span(),
                "choices on a `bool` cannot say anything its two values do not already",
            ));
        }
        for choice in &prop.choices {
            if !prop.ty.admits(choice, Position::Choice) {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("a choice is not a value `{}` can hold", type_name(&prop.ty)),
                ));
            }
        }
        if prop.merge.is_some() && !prop.ty.is_collection() {
            return Err(syn::Error::new(
                ident.span(),
                "`merge` says how a collection combines across layers, and this is not one",
            ));
        }
        if prop.parse.is_some() && !prop.ty.is_collection() {
            return Err(syn::Error::new(
                ident.span(),
                "`parse` splits one string into several values, and this field holds one",
            ));
        }

        Ok(Self::Prop(Box::new(prop)))
    }
}

fn type_name(ty: &Ty) -> String {
    match ty {
        Ty::Bool => "bool".into(),
        Ty::Int => "int".into(),
        Ty::Uint => "uint".into(),
        Ty::Float => "float".into(),
        Ty::String => "string".into(),
        Ty::Path => "path".into(),
        Ty::Url => "url".into(),
        Ty::Duration => "duration".into(),
        Ty::Object => "object".into(),
        Ty::Any => "any".into(),
        Ty::List(inner) => format!("list<{}>", type_name(inner)),
        Ty::Set(inner) => format!("set<{}>", type_name(inner)),
        Ty::Map(inner) => format!("map<string, {}>", type_name(inner)),
    }
}

/// `Option<T>` peeled to `T`, and whether there was one to peel.
fn peel_option(ty: &syn::Type) -> (bool, syn::Type) {
    if let syn::Type::Path(path) = ty {
        if let Some(last) = path.path.segments.last() {
            if last.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return (true, inner.clone());
                    }
                }
            }
        }
    }
    (false, ty.clone())
}

/// The spec type a Rust type names on its own, or `None` for one that needs `ty = "..."`.
fn infer_ty(ty: &syn::Type) -> Option<Ty> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    let name = last.ident.to_string();
    let generic = |index: usize| -> Option<syn::Type> {
        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
            args.args
                .iter()
                .filter_map(|arg| match arg {
                    syn::GenericArgument::Type(inner) => Some(inner.clone()),
                    _ => None,
                })
                .nth(index)
        } else {
            None
        }
    };
    Some(match name.as_str() {
        "bool" => Ty::Bool,
        "u8" | "u16" | "u32" | "u64" | "usize" => Ty::Uint,
        "i8" | "i16" | "i32" | "i64" | "isize" => Ty::Int,
        "f32" | "f64" => Ty::Float,
        "String" => Ty::String,
        "PathBuf" => Ty::Path,
        "Value" => Ty::Any,
        "Vec" => Ty::List(Box::new(infer_ty(&generic(0)?)?)),
        "BTreeMap" => {
            // The key is part of the shape: a spec map is keyed by strings, and a map keyed
            // by anything else has no spelling a config file could write.
            let key = generic(0)?;
            if !matches!(infer_ty(&key), Some(Ty::String)) {
                return None;
            }
            Ty::Map(Box::new(infer_ty(&generic(1)?)?))
        }
        _ => return None,
    })
}

/// A spec type by the name a spec writes: `uint`, `list<string>`, `map<string, path>`.
fn parse_ty_name(name: &str) -> Option<Ty> {
    let name = name.trim();
    if let Some(inner) = name.strip_prefix("list<").and_then(|s| s.strip_suffix('>')) {
        return Some(Ty::List(Box::new(parse_ty_name(inner)?)));
    }
    if let Some(inner) = name.strip_prefix("set<").and_then(|s| s.strip_suffix('>')) {
        return Some(Ty::Set(Box::new(parse_ty_name(inner)?)));
    }
    if let Some(inner) = name.strip_prefix("map<").and_then(|s| s.strip_suffix('>')) {
        let (key, value) = inner.split_once(',')?;
        if key.trim() != "string" {
            return None;
        }
        return Some(Ty::Map(Box::new(parse_ty_name(value)?)));
    }
    Some(match name {
        "bool" => Ty::Bool,
        "int" => Ty::Int,
        "uint" => Ty::Uint,
        "float" => Ty::Float,
        "string" => Ty::String,
        "path" => Ty::Path,
        "url" => Ty::Url,
        "duration" => Ty::Duration,
        "object" => Ty::Object,
        "any" => Ty::Any,
        _ => return None,
    })
}

/// `env = "X"` or `env("A", "B")`, as the strings.
fn strings(meta: &Meta) -> syn::Result<Vec<String>> {
    match meta {
        Meta::NameValue(_) => Ok(vec![string_value(meta)?]),
        Meta::List(list) => {
            let parsed = list.parse_args_with(
                syn::punctuated::Punctuated::<Lit, syn::Token![,]>::parse_terminated,
            )?;
            parsed
                .into_iter()
                .map(|lit| match lit {
                    Lit::Str(s) => Ok(s.value()),
                    other => Err(syn::Error::new_spanned(other, "expected a string")),
                })
                .collect()
        }
        Meta::Path(path) => Err(syn::Error::new_spanned(
            path,
            "expected a value, as in `env = \"EX_JOBS\"` or `env(\"A\", \"B\")`",
        )),
    }
}

/// `default = 4`, `default = "x"`, `default = -1`, or `default(80, 443)` for a list.
fn const_value(meta: &Meta) -> syn::Result<Const> {
    match meta {
        Meta::NameValue(nv) => const_expr(&nv.value),
        Meta::List(list) => {
            let parsed = list.parse_args_with(
                syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated,
            )?;
            Ok(Const::List(
                parsed
                    .into_iter()
                    .map(|expr| const_expr(&expr))
                    .collect::<syn::Result<_>>()?,
            ))
        }
        Meta::Path(path) => Err(syn::Error::new_spanned(
            path,
            "expected a value, as in `default = 4` or `default(80, 443)`",
        )),
    }
}

fn consts(meta: &Meta) -> syn::Result<Vec<Const>> {
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "expected a list, as in `choices(\"a\", \"b\")`",
        ));
    };
    let parsed = list
        .parse_args_with(syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated)?;
    parsed.into_iter().map(|expr| const_expr(&expr)).collect()
}

fn const_expr(expr: &Expr) -> syn::Result<Const> {
    match expr {
        Expr::Lit(ExprLit { lit, .. }) => const_lit(lit, false),
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => {
            if let Expr::Lit(ExprLit { lit, .. }) = unary.expr.as_ref() {
                return const_lit(lit, true);
            }
            Err(syn::Error::new_spanned(expr, "expected a literal"))
        }
        other => Err(syn::Error::new_spanned(
            other,
            "expected a literal the registry can hold as a const; a computed value is \
             `default_fn`",
        )),
    }
}

fn const_lit(lit: &Lit, negated: bool) -> syn::Result<Const> {
    let value = match lit {
        Lit::Bool(b) => Const::Bool(b.value()),
        Lit::Int(i) => Const::Int(i.base10_parse::<i64>()?),
        Lit::Float(f) => Const::Float(f.base10_parse::<f64>()?),
        Lit::Str(s) => Const::Str(s.value()),
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "expected a boolean, number, or string",
            ))
        }
    };
    Ok(match (negated, value) {
        (false, value) => value,
        (true, Const::Int(i)) => Const::Int(-i),
        (true, Const::Float(f)) => Const::Float(-f),
        (true, _) => {
            return Err(syn::Error::new_spanned(
                lit,
                "only a number can be negative",
            ))
        }
    })
}

pub fn emit(config: &Config) -> TokenStream {
    let ident = &config.ident;
    let cfg = config_path();

    let metas: Vec<TokenStream> = config
        .fields
        .iter()
        .filter_map(|field| match field {
            Field::Prop(prop) => Some(prop_meta(prop, &cfg)),
            Field::Flatten { .. } => None,
        })
        .collect();

    let has_flatten = config
        .fields
        .iter()
        .any(|field| matches!(field, Field::Flatten { .. }));

    // Without flattening the props are one literal slice. With it, the slices join at
    // compile time in field order, so an id is a position in the joined table — the same
    // arrangement `usage::Cli` uses for a flattened group's flags.
    let metas_len = metas.len();
    let props_decl = if has_flatten {
        let parts: Vec<TokenStream> = config
            .fields
            .iter()
            .map(|field| match field {
                Field::Prop(prop) => {
                    let meta = prop_meta(prop, &cfg);
                    quote!(&[#meta])
                }
                Field::Flatten { ty, .. } => quote!(<#ty as #cfg::Props>::PROPS),
            })
            .collect();
        quote! {
            const __USAGE_PARTS: &[&[#cfg::PropMeta]] = &[#(#parts),*];
            const __USAGE_LEN: usize = {
                let mut total = 0;
                let mut i = 0;
                while i < __USAGE_PARTS.len() {
                    total += __USAGE_PARTS[i].len();
                    i += 1;
                }
                total
            };
            static __USAGE_PROPS: [#cfg::PropMeta; __USAGE_LEN] =
                #cfg::concat_props(__USAGE_PARTS);
        }
    } else {
        quote! {
            static __USAGE_PROPS: [#cfg::PropMeta; #metas_len] = [#(#metas),*];
        }
    };

    // Reads in declaration order, advancing a cursor: one id per own prop, a group's length
    // for a flattened child. Everything is read before anything is judged, so the fold holds
    // every error rather than the first one.
    let reads: Vec<TokenStream> = config
        .fields
        .iter()
        .map(|field| match field {
            Field::Prop(prop) => {
                let local = read_local(&prop.ident);
                let read_ty = &prop.read_ty;
                let method = if prop.optional_field || prop.default_fn.is_some() {
                    quote!(optional)
                } else {
                    quote!(required)
                };
                quote! {
                    let #local: ::std::option::Option<#read_ty> =
                        __usage_fold.#method(#cfg::PropId(__usage_at));
                    __usage_at += 1;
                }
            }
            Field::Flatten { ident, ty } => {
                let local = read_local(ident);
                quote! {
                    let #local: ::std::option::Option<#ty> =
                        <#ty as #cfg::Props>::read_at(__usage_fold, __usage_at);
                    __usage_at += <#ty as #cfg::Props>::PROPS.len() as u16;
                }
            }
        })
        .collect();

    let builds: Vec<TokenStream> = config
        .fields
        .iter()
        .map(|field| match field {
            Field::Prop(prop) => {
                let name = &prop.ident;
                let local = read_local(name);
                if let Some(default_fn) = &prop.default_fn {
                    quote!(#name: #local.unwrap_or_else(#default_fn))
                } else if prop.optional_field {
                    quote!(#name: #local)
                } else {
                    quote!(#name: #local?)
                }
            }
            Field::Flatten { ident, .. } => {
                let local = read_local(ident);
                quote!(#ident: #local?)
            }
        })
        .collect();

    quote! {
        const _: () = {
            #props_decl

            impl #cfg::Props for #ident {
                const PROPS: &'static [#cfg::PropMeta] = &__USAGE_PROPS;

                fn read_at(
                    __usage_fold: &mut #cfg::Fold<'_>,
                    __usage_base: u16,
                ) -> ::std::option::Option<Self> {
                    let mut __usage_at: u16 = __usage_base;
                    #(#reads)*
                    let _ = __usage_at;
                    ::std::option::Option::Some(Self {
                        #(#builds),*
                    })
                }
            }

            impl #ident {
                /// Every setting this struct declares, one entry per field, flattened groups
                /// included. The registry a `build.rs` used to generate, generated from the
                /// struct instead — there is no second declaration to keep in step.
                pub const SETTINGS_PROPS: &'static [#cfg::PropMeta] =
                    <Self as #cfg::Props>::PROPS;

                /// The registry over [`Self::SETTINGS_PROPS`], for `resolve`, `drift`, and
                /// the layers.
                pub const SETTINGS_REGISTRY: #cfg::Registry =
                    #cfg::Registry::new(Self::SETTINGS_PROPS);

                /// This resolution's values, as the struct.
                ///
                /// Every field is read before anything is returned, so the error is the whole
                /// list of what is wrong rather than the first thing found.
                pub fn read(
                    __usage_resolved: &#cfg::Resolved,
                ) -> ::std::result::Result<Self, #cfg::ReadErrors> {
                    let mut __usage_fold = __usage_resolved.fold();
                    let __usage_read = <Self as #cfg::Props>::read_at(&mut __usage_fold, 0);
                    __usage_fold.finish()?;
                    ::std::result::Result::Ok(__usage_read.expect(
                        "the fold reported nothing, so every field was read",
                    ))
                }

                /// The spec `config` block for these settings, as KDL.
                ///
                /// What documents, JSON schema and completions read. A CLI deriving
                /// `usage::Cli` names this type in `#[usage(config = ...)]` instead of
                /// calling this, and its `to_kdl` carries the block.
                pub fn spec_kdl() -> ::std::string::String {
                    #cfg::spec_kdl(Self::SETTINGS_PROPS)
                }
            }
        };
    }
}

/// The local a field is read into, unrawed: `r#match` reads into `__usage_read_match`.
fn read_local(ident: &syn::Ident) -> syn::Ident {
    let name = ident.to_string();
    let name = name.strip_prefix("r#").unwrap_or(&name);
    format_ident!("__usage_read_{name}")
}

/// One prop as registry metadata, in struct-update form over `PropMeta::new`.
fn prop_meta(prop: &Prop, cfg: &TokenStream) -> TokenStream {
    let key = &prop.key;
    let ty = if prop.optional_field {
        let inner = prop.ty.tokens(cfg);
        quote!(#cfg::Ty::Option(&#inner))
    } else {
        prop.ty.tokens(cfg)
    };
    let mut fields = Vec::new();
    if let Some(default) = &prop.default {
        let default = default.tokens(cfg);
        fields.push(quote!(default: ::std::option::Option::Some(#default)));
    }
    if let Some(merge) = prop.merge {
        let merge = match merge {
            Merge::Union => quote!(#cfg::Merge::Union),
            Merge::Deep => quote!(#cfg::Merge::Deep),
        };
        fields.push(quote!(merge: #merge));
    }
    if let Some(scope) = prop.scope {
        let scope = match scope {
            Scope::Global => quote!(#cfg::Scope::Global),
            Scope::Env => quote!(#cfg::Scope::Env),
        };
        fields.push(quote!(scope: #scope));
    }
    if let Some(parse) = &prop.parse {
        let parser = match parse.as_str() {
            "list_by_comma" => quote!(#cfg::Parser::ListByComma),
            "list_by_colon" => quote!(#cfg::Parser::ListByColon),
            "list_by_os_path_separator" => quote!(#cfg::Parser::ListByOsPathSeparator),
            _ => quote!(#cfg::Parser::SetByComma),
        };
        fields.push(quote!(parse: ::std::option::Option::Some(#parser)));
    }
    if !prop.envs.is_empty() {
        let envs = &prop.envs;
        fields.push(quote!(envs: &[#(#envs),*]));
    }
    if !prop.deprecated_envs.is_empty() {
        let envs = &prop.deprecated_envs;
        fields.push(quote!(deprecated_envs: &[#(#envs),*]));
    }
    if !prop.cli.is_empty() {
        let cli = &prop.cli;
        fields.push(quote!(cli: &[#(#cli),*]));
    }
    if !prop.bindings.is_empty() {
        let pairs = prop
            .bindings
            .iter()
            .map(|(kind, key)| quote!((#kind, #key)));
        fields.push(quote!(bindings: &[#(#pairs),*]));
    }
    if !prop.choices.is_empty() {
        let choices = prop.choices.iter().map(|choice| choice.tokens(cfg));
        fields.push(quote!(choices: &[#(#choices),*]));
    }
    if prop.hide {
        fields.push(quote!(hide: true));
    }
    if let Some(deprecated) = &prop.deprecated {
        fields.push(quote!(deprecated: ::std::option::Option::Some(#deprecated)));
    }
    if !prop.aliases.is_empty() {
        let aliases = &prop.aliases;
        fields.push(quote!(aliases: &[#(#aliases),*]));
    }
    // The optionality contract, stated rather than inferred. A registry that leaves this
    // unset invites the reader's inference — "no default means optional" — and a plain
    // non-`Option` field with no default is the one case where that inference disagrees with
    // `read`, which reports the key as missing. Docs, the JSON schema and the completers all
    // read the registry, so they have to be told what `read` will do.
    let optional = if prop.optional_field || prop.default_fn.is_some() {
        Some(true)
    } else if prop.default.is_none() {
        Some(false)
    } else {
        // A declared default always resolves, and inference already agrees with that.
        None
    };
    if let Some(optional) = optional {
        fields.push(quote!(optional: ::std::option::Option::Some(#optional)));
    }
    if let Some(help) = &prop.help {
        fields.push(quote!(help: ::std::option::Option::Some(#help)));
    }
    if let Some(long_help) = &prop.long_help {
        fields.push(quote!(long_help: ::std::option::Option::Some(#long_help)));
    }
    if let Some(note) = &prop.default_note {
        fields.push(quote!(default_note: ::std::option::Option::Some(#note)));
    }
    if let Some(since) = &prop.since {
        fields.push(quote!(since: ::std::option::Option::Some(#since)));
    }
    if let Some(at) = &prop.deprecated_warn_at {
        fields.push(quote!(deprecated_warn_at: ::std::option::Option::Some(#at)));
    }
    if let Some(at) = &prop.deprecated_remove_at {
        fields.push(quote!(deprecated_remove_at: ::std::option::Option::Some(#at)));
    }
    if !prop.examples.is_empty() {
        let examples = &prop.examples;
        fields.push(quote!(examples: &[#(#examples),*]));
    }
    quote! {
        #cfg::PropMeta {
            #(#fields,)*
            ..#cfg::PropMeta::new(#key, #ty)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    /// The message a bad declaration produces, which is the part worth asserting on: it is
    /// what the author sees, and the point of checking here is that they see it *here* rather
    /// than at the first `read` in production.
    fn rejection(body: &str) -> String {
        match Config::from_input(&syn::parse_str::<syn::DeriveInput>(body).expect("valid Rust")) {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        }
    }

    fn accepted(body: &str) {
        Config::from_input(&syn::parse_str::<syn::DeriveInput>(body).expect("valid Rust"))
            .unwrap_or_else(|e| panic!("should have compiled: {e}"));
    }

    #[test]
    fn a_uint_refuses_a_negative_default_or_choice() {
        // The resolver seeds a declared default with no coercion, and `u64::from_value`
        // refuses a negative value — so this compiled and then failed every `read` with a type
        // error the author could do nothing about at run time. A choice is the same shape of
        // mistake: a value nothing can ever supply.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(default = -1)]
                jobs: u64,
            }
        "#,
        );
        assert!(
            err.contains("the default is not a value `uint` can hold"),
            "unhelpful: {err}"
        );

        let err = rejection(
            r#"
            struct Settings {
                #[usage(choices(1, -1))]
                jobs: u64,
            }
        "#,
        );
        assert!(
            err.contains("a choice is not a value `uint` can hold"),
            "unhelpful: {err}"
        );

        // A signed field still takes one, which is the whole difference.
        accepted(
            r#"
            struct Settings {
                #[usage(default = -1)]
                offset: i64,
            }
        "#,
        );
    }

    #[test]
    fn a_default_is_refused_unless_it_is_already_the_field_s_type() {
        // Nothing coerces a declared default: the resolver seeds it with `Const::to_value`
        // and the reader is handed it as it stands. So the permissiveness the merge has —
        // a string type reading a bare number as its text, a scalar standing in for a list
        // of one — is not permissiveness a default gets. Each of these compiled and then
        // failed *every* `Settings::read` with a type error nothing at run time could fix.
        for (field, default) in [
            ("name: String", "default = 1"),
            ("name: String", "default = true"),
            ("home: std::path::PathBuf", "default = 1"),
            ("ports: Vec<u64>", "default = 80"),
        ] {
            let err = rejection(&format!(
                r#"
                struct Settings {{
                    #[usage({default})]
                    {field},
                }}
            "#
            ));
            assert!(
                err.contains("the default is not a value"),
                "`{default}` was accepted on `{field}`: {err}"
            );
            // And the message says why, because "not a value `string` can hold" reads like a
            // lie next to a spec's `default=1`, which the merge does coerce.
            assert!(
                err.contains("Nothing coerces a default"),
                "unhelpful for `{default}` on `{field}`: {err}"
            );
        }

        // Written as the type the field holds, each is fine — including a list of one, which
        // the attribute already spells apart from a bare scalar.
        for (field, default) in [
            ("name: String", r#"default = "1""#),
            ("home: std::path::PathBuf", r#"default = "/tmp""#),
            ("ports: Vec<u64>", "default(80)"),
            ("ports: Vec<u64>", "default(80, 443)"),
        ] {
            accepted(&format!(
                r#"
                struct Settings {{
                    #[usage({default})]
                    {field},
                }}
            "#
            ));
        }
    }

    #[test]
    fn a_choice_still_reads_the_way_the_merge_reads_one() {
        // The other side of the same rule. A choice is compared against a resolved value
        // *after* coercion, so a `list<string>` setting's choices name what one item may be,
        // and a string setting may name a bare number. Tightening the default check must not
        // tighten this one: the registry's own comparison coerces before it compares.
        accepted(
            r#"
            struct Settings {
                #[usage(choices("a", "b"))]
                tags: Vec<String>,
            }
        "#,
        );
        accepted(
            r#"
            struct Settings {
                #[usage(choices(1, 2))]
                level: String,
            }
        "#,
        );
    }

    #[test]
    fn every_setting_attribute_on_a_flattened_field_is_refused() {
        // `flatten` says the field is a group of settings, so anything describing *one*
        // setting was parsed into a prop that the flatten branch then dropped. The checked
        // list had grown stale: `#[usage(flatten, alias = "task")]` compiled, and the alias
        // simply did not exist.
        for attribute in [
            r#"env = "EX_X""#,
            r#"deprecated_env = "EX_OLD""#,
            r#"alias = "other""#,
            r#"example = "1""#,
            r#"default_note = "note""#,
            "optional = true",
            r#"deprecated = "gone""#,
            r#"deprecated_warn_at = "6.0.0""#,
            r#"deprecated_remove_at = "7.0.0""#,
            r#"since = "5.2.0""#,
        ] {
            let err = rejection(&format!(
                r#"
                struct Settings {{
                    #[usage(flatten, {attribute})]
                    task: TaskSettings,
                }}
            "#
            ));
            assert!(
                err.contains("describes a setting, and `flatten` says this field is a group"),
                "`{attribute}` was accepted on a flattened field: {err}"
            );
        }

        // A doc comment is not one of them: it describes the group, and `help` is how the
        // derive carries a doc comment.
        accepted(
            r#"
            struct Settings {
                /// The task settings
                #[usage(flatten)]
                task: TaskSettings,
            }
        "#,
        );
    }
}
