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
    sources: Vec<Source>,
    files: Vec<File>,
}

struct Source {
    kind: String,
    name: Option<String>,
    doc_hint: Option<String>,
    set_hint: Option<String>,
}

struct File {
    path: String,
    findup: bool,
    xdg: Option<XdgBase>,
    scope: FileScope,
    format: Option<String>,
}

#[derive(Clone, Copy)]
enum FileScope {
    Project,
    Global,
    System,
}

#[derive(Clone, Copy)]
enum XdgBase {
    Config,
    Data,
    State,
    Cache,
    Runtime,
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
    help_heading: Option<String>,
    writes_to: Option<String>,
    extensions: Vec<(String, Const)>,
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
            // A spec's `type="string"` does read a bare number as its text, and the merge
            // coerces one — but a *declaration written in Rust* has no reason to spell a
            // string as anything but a string. Holding both a default and a choice to that
            // means one spelling for both, which is what lets them be compared as written:
            // the alternative is a third implementation of "how a float is written", the
            // drift `config_value_display.rs` exists to catch.
            (
                Self::String | Self::Path | Self::Url | Self::Duration,
                Const::Bool(_) | Const::Int(_) | Const::Float(_),
            ) => false,
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
        let mut sources = Vec::new();
        let mut files = Vec::new();
        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                match ident_of(meta.path()).as_str() {
                    "prefix" => prefix = Some(string_value(&meta)?),
                    "source" => sources.push(source_decl(&meta)?),
                    "file" => files.push(file_decl(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            meta.path(),
                            format!("usage::Config does not understand `{other}` on the struct"),
                        ));
                    }
                }
            }
        }
        sources.sort_by(|a, b| a.kind.cmp(&b.kind));
        for pair in sources.windows(2) {
            if pair[0].kind == pair[1].kind {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    format!(
                        "config source `{}` is declared more than once; combine its metadata \
                         into one `source(...)`",
                        pair[0].kind
                    ),
                ));
            }
        }

        let mut fields = Vec::new();
        for field in &named.named {
            fields.push(Field::from_field(field, prefix.as_deref())?);
        }

        // A duplicate within one struct is visible here, so it is refused here, with spans.
        // Duplicates across flattened structs are refused by `concat_props` at compile time.
        //
        // Aliases are names too: `Registry::lookup` checks keys and aliases together and takes
        // the first match, so an alias that collides with another setting's key — or with its
        // alias — makes one of the two unreachable by that name, silently. Every name a
        // setting answers to therefore has to be unique among all of them, not just the keys.
        let mut names: Vec<(&str, &syn::Ident, bool)> = Vec::new();
        for field in &fields {
            let Field::Prop(prop) = field else { continue };
            for (name, is_alias) in std::iter::once((prop.key.as_str(), false))
                .chain(prop.aliases.iter().map(|alias| (alias.as_str(), true)))
            {
                if let Some((_, first, first_alias)) =
                    names.iter().find(|(taken, _, _)| *taken == name)
                {
                    let what = |alias: bool| match alias {
                        true => "an alias",
                        false => "the key",
                    };
                    // Three different mistakes, and the message has to name the right one:
                    // one field colliding with itself is not two settings fighting over a
                    // name, and which of its own names came first says which mistake it is.
                    let same_field = std::ptr::eq(*first, &prop.ident);
                    return Err(syn::Error::new(
                        prop.ident.span(),
                        match (same_field, *first_alias) {
                            // Its own key, which is not a collision between two settings but
                            // an alias that can never be reached: the key is found first.
                            (true, false) => format!(
                                "`{}` lists `{name}` as an alias of its own key, which \
                                 nothing would ever reach",
                                prop.ident
                            ),
                            (true, true) => {
                                format!("`{}` lists the alias `{name}` twice", prop.ident)
                            }
                            (false, _) => format!(
                                "`{name}` is {} of `{}` and {} of `{first}`, and a lookup \
                                 takes the first of them: one of the two could never be \
                                 reached by that name",
                                what(is_alias),
                                prop.ident,
                                what(*first_alias),
                            ),
                        },
                    ));
                }
                names.push((name, &prop.ident, is_alias));
            }
        }

        Ok(Self {
            ident: input.ident.clone(),
            fields,
            sources,
            files,
        })
    }
}

fn nested_meta(meta: &Meta) -> syn::Result<Vec<Meta>> {
    let Meta::List(list) = meta else {
        return Ok(Vec::new());
    };
    let parsed = list
        .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)?;
    Ok(parsed.into_iter().collect())
}

fn source_decl(meta: &Meta) -> syn::Result<Source> {
    let Meta::List(_) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "a config source is `source(kind = \"git\", name = \"git config\", ...)`",
        ));
    };
    let mut kind = None;
    let mut name = None;
    let mut doc_hint = None;
    let mut set_hint = None;
    for item in nested_meta(meta)? {
        let slot = match ident_of(item.path()).as_str() {
            "kind" => &mut kind,
            "name" => &mut name,
            "doc_hint" => &mut doc_hint,
            "set_hint" => &mut set_hint,
            other => {
                return Err(syn::Error::new_spanned(
                    item.path(),
                    format!(
                        "a config source does not understand `{other}`; use `kind`, `name`, \
                         `doc_hint`, or `set_hint`"
                    ),
                ))
            }
        };
        if slot.is_some() {
            return Err(syn::Error::new_spanned(
                &item,
                format!(
                    "`{}` is given twice in this config source",
                    ident_of(item.path())
                ),
            ));
        }
        *slot = Some(string_value(&item)?);
    }
    let kind = kind
        .ok_or_else(|| syn::Error::new_spanned(meta, "a config source needs `kind = \"...\"`"))?;
    if kind.is_empty() {
        return Err(syn::Error::new_spanned(
            meta,
            "a config source kind cannot be empty",
        ));
    }
    Ok(Source {
        kind,
        name,
        doc_hint,
        set_hint,
    })
}

fn file_decl(meta: &Meta) -> syn::Result<File> {
    let Meta::List(_) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "a config file is `file(path = \"ex.toml\", findup, scope = \"project\")`",
        ));
    };
    let mut path = None;
    let mut findup = false;
    let mut saw_findup = false;
    let mut xdg = None;
    let mut scope = FileScope::Project;
    let mut saw_scope = false;
    let mut format = None;
    for item in nested_meta(meta)? {
        match ident_of(item.path()).as_str() {
            "path" => {
                if path.is_some() {
                    return Err(syn::Error::new_spanned(&item, "`path` is given twice"));
                }
                path = Some(string_value(&item)?);
            }
            "findup" => {
                if saw_findup {
                    return Err(syn::Error::new_spanned(&item, "`findup` is given twice"));
                }
                findup = flag_value(&item)?;
                saw_findup = true;
            }
            "xdg" => {
                if xdg.is_some() {
                    return Err(syn::Error::new_spanned(&item, "`xdg` is given twice"));
                }
                let value = string_value(&item)?;
                xdg = Some(match value.as_str() {
                    "config" => XdgBase::Config,
                    "data" => XdgBase::Data,
                    "state" => XdgBase::State,
                    "cache" => XdgBase::Cache,
                    "runtime" => XdgBase::Runtime,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &item,
                            format!(
                                "`{value}` is not an XDG base; use `config`, `data`, `state`, \
                                 `cache`, or `runtime`"
                            ),
                        ))
                    }
                });
            }
            "scope" => {
                if saw_scope {
                    return Err(syn::Error::new_spanned(&item, "`scope` is given twice"));
                }
                let value = string_value(&item)?;
                scope = match value.as_str() {
                    "project" => FileScope::Project,
                    "global" => FileScope::Global,
                    "system" => FileScope::System,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &item,
                            format!(
                                "`{value}` is not a config file scope; use `project`, `global`, \
                                 or `system`"
                            ),
                        ))
                    }
                };
                saw_scope = true;
            }
            "format" => {
                if format.is_some() {
                    return Err(syn::Error::new_spanned(&item, "`format` is given twice"));
                }
                format = Some(string_value(&item)?);
            }
            other => {
                return Err(syn::Error::new_spanned(
                    item.path(),
                    format!(
                        "a config file does not understand `{other}`; use `path`, `findup`, \
                         `xdg`, `scope`, or `format`"
                    ),
                ))
            }
        }
    }
    let path =
        path.ok_or_else(|| syn::Error::new_spanned(meta, "a config file needs `path = \"...\"`"))?;
    if path.is_empty() {
        return Err(syn::Error::new_spanned(
            meta,
            "a config file path cannot be empty",
        ));
    }
    if xdg.is_some() && (saw_findup || saw_scope) {
        return Err(syn::Error::new_spanned(
            meta,
            "an XDG file's base decides its scope; do not combine `xdg` with `findup` or \
             `scope`",
        ));
    }
    if xdg.is_some() && std::path::Path::new(&path).is_absolute() {
        return Err(syn::Error::new_spanned(
            meta,
            "an XDG config file path must be relative to the XDG config directories",
        ));
    }
    Ok(File {
        path,
        findup,
        xdg,
        scope,
        format,
    })
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
            help_heading: None,
            writes_to: None,
            extensions: Vec::new(),
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
                    "x" | "extension" => prop.extensions.push(extension(&meta)?),
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
                    "help_heading" => prop.help_heading = Some(string_value(&meta)?),
                    "writes_to" => prop.writes_to = Some(string_value(&meta)?),
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
                (prop.help_heading.is_some(), "help_heading"),
                (prop.writes_to.is_some(), "writes_to"),
                (!prop.extensions.is_empty(), "x"),
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
            Some((name, span)) => {
                let declared = parse_ty_name(&name)
                    .ok_or_else(|| syn::Error::new(span, format!("`{name}` is not a spec type")))?;
                // A spec type the field could never read. The merge coerces to the *declared*
                // type, so the shape it hands over is decided here and not by what a layer
                // supplied — `ty = "uint"` on a `String` field therefore failed every single
                // `read`, whatever anyone configured. Only an always-broken pairing is refused:
                // `ty = "int"` on a `u8` is a widening the author may mean, and it reads
                // whenever the value fits.
                if reads(&inner, &declared) == Some(false) {
                    return Err(syn::Error::new(
                        span,
                        format!(
                            "a `{name}` setting cannot be read into `{}`: `ty` renames what \
                             the spec calls a setting, and cannot change what the field holds",
                            rust_type_name(&inner),
                        ),
                    ));
                }
                declared
            }
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
            // And what the *field* can hold, which the spec type does not say: `uint` covers
            // every unsigned width, so 256 passed the check above and then failed every
            // `read` on a `u8`.
            if !field_holds(&prop.read_ty, default, Position::Default) {
                return Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "the default does not fit `{}`, which is what this field holds",
                        rust_type_name(&prop.read_ty)
                    ),
                ));
            }
            // Which values have to be one of the choices depends on what the choices name.
            // On a list setting they name what one *item* may be — that is how the registry
            // compares a resolved value against them — so it is the items that are checked.
            // Comparing the whole list refused `default("a")` beside `choices("a", "b")`, a
            // declaration whose every value is one of them.
            if !prop.choices.is_empty() {
                let declared: Vec<&Const> = match (&prop.ty, default) {
                    (Ty::List(_) | Ty::Set(_), Const::List(items)) => items.iter().collect(),
                    _ => vec![default],
                };
                if !declared
                    .iter()
                    .all(|value| prop.choices.iter().any(|choice| choice == *value))
                {
                    return Err(syn::Error::new(
                        ident.span(),
                        match declared.len() > 1 {
                            true => {
                                "the default has a value that is not one of the declared \
                                     choices"
                            }
                            false => "the default is not one of the declared choices",
                        },
                    ));
                }
            }
        }
        if !prop.choices.is_empty() && prop.ty == Ty::Bool {
            return Err(syn::Error::new(
                ident.span(),
                "choices on a `bool` cannot say anything its two values do not already",
            ));
        }
        for choice in &prop.choices {
            // The spec type first, then the field's own — the same order the default checks
            // run in. A plain `u64` should hear that `uint` cannot hold a negative, which is
            // the contract the registry will enforce; the field-level complaint is for when
            // the spec type is fine and a `ty` override moved the problem.
            if !prop.ty.admits(choice, Position::Choice) {
                return Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "a choice is not a value `{}` can hold. Write it as the type the \
                         setting is — a `string` setting's choices are strings, quoted",
                        type_name(&prop.ty)
                    ),
                ));
            }
            if !field_holds(&prop.read_ty, choice, Position::Choice) {
                return Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "a choice does not fit `{}`, so nothing could ever supply it",
                        rust_type_name(&prop.read_ty)
                    ),
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
/// The `Value` variant the merge hands a setting of a given spec type.
///
/// `Ty::coerce` decides the shape from the *declared* type, not from what a layer supplied, so
/// this is what the field's `FromValue` will actually be given.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Bool,
    Int,
    Float,
    Str,
    List,
    Map,
}

impl Ty {
    /// `None` for `any`, which is the one type the merge does not coerce: it hands over
    /// whatever arrived, so no shape can be promised or refused.
    fn shape(&self) -> Option<Shape> {
        Some(match self {
            Self::Any => return None,
            Self::Bool => Shape::Bool,
            // A `uint` is an `int` the merge additionally refuses when negative. Same shape.
            Self::Int | Self::Uint => Shape::Int,
            Self::Float => Shape::Float,
            Self::String | Self::Path | Self::Url | Self::Duration => Shape::Str,
            Self::Object | Self::Map(_) => Shape::Map,
            Self::List(_) | Self::Set(_) => Shape::List,
        })
    }
}

/// Whether the field's `FromValue` can read what a setting of type `declared` is handed.
///
/// Structural, not a top-level shape comparison: the merge coerces a list's *items* to the
/// declared item type, so `ty = "list<uint>"` on a `Vec<String>` is handed a list of integers
/// and fails on the first one. Comparing only the outer kind called that a match.
///
/// `None` where the answer is not knowable — a type alias, a type whose `FromValue` an adopter
/// wrote, or a declared type that promises no shape — because a `ty` override exists for
/// exactly the types this cannot measure, and refusing what it cannot see would make the
/// escape hatch useless.
fn reads(field: &syn::Type, declared: &Ty) -> Option<bool> {
    // `any` is the one type the merge does not coerce: it hands over whatever arrived, so
    // there is no shape to promise or refuse.
    let shape = declared.shape()?;
    let syn::Type::Path(path) = field else {
        return None;
    };
    let last = path.path.segments.last()?;
    match last.ident.to_string().as_str() {
        // `Value` is the escape hatch for `any`: it reads whatever it is handed.
        "Value" => Some(true),
        "bool" => Some(shape == Shape::Bool),
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" => {
            Some(shape == Shape::Int)
        }
        // A whole number is a perfectly good float, which is the rule `FromValue for f64`
        // states and this has to agree with.
        "f32" | "f64" => Some(shape == Shape::Float || shape == Shape::Int),
        "String" | "PathBuf" => Some(shape == Shape::Str),
        "Vec" => match declared {
            Ty::List(item) | Ty::Set(item) => {
                generic_arg(last, 0).and_then(|inner| reads(&inner, item))
            }
            _ => Some(false),
        },
        "BTreeMap" => match declared {
            Ty::Map(value) => generic_arg(last, 1).and_then(|inner| reads(&inner, value)),
            // `object` is a table whose value types the spec deliberately does not describe,
            // so there is nothing to hold the field's own to.
            Ty::Object => None,
            _ => Some(false),
        },
        _ => None,
    }
}

/// A field's Rust type as written, for a message that has to name it.
fn rust_type_name(ty: &syn::Type) -> String {
    quote::ToTokens::to_token_stream(ty)
        .to_string()
        .replace(" ", "")
}

/// The `index`th type argument of a path segment, as in the `u64` of `Vec<u64>`.
fn generic_arg(segment: &syn::PathSegment, index: usize) -> Option<syn::Type> {
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(inner) => Some(inner.clone()),
            _ => None,
        })
        .nth(index)
}

/// Whether the field's own Rust type can hold `value`.
///
/// A narrower check than [`Ty::admits`], and it has to be separate: `infer_ty` collapses every
/// unsigned width to `uint`, so the spec-level type cannot see that a `u8` refuses 256. The
/// resolver seeds a default uncoerced and `FromValue` is strict about width, so a default the
/// field cannot hold fails every `read` — the same trap as a negative `uint`, one level down.
///
/// Takes the `position` for the same reason [`Ty::admits`] does: on a list or set a *choice*
/// names one item rather than the whole value, so it is the item type that has to hold it.
/// Measuring a scalar choice against the container measured nothing at all.
///
/// `true` for anything this does not recognize: the spec-level check has already had its say,
/// and a type it allows that this cannot measure is not this function's to refuse.
fn field_holds(ty: &syn::Type, value: &Const, position: Position) -> bool {
    let syn::Type::Path(path) = ty else {
        return true;
    };
    let Some(last) = path.path.segments.last() else {
        return true;
    };
    macro_rules! fits {
        ($int:ty) => {
            match value {
                Const::Int(i) => <$int>::try_from(*i).is_ok(),
                _ => true,
            }
        };
    }
    match last.ident.to_string().as_str() {
        // Every integer the reader has a `FromValue` for, including the two whose check is a
        // formality: `i64` holds anything a `Const::Int` can be, and `u64` refuses only a
        // negative. `u64` was left out on the reasoning that `Ty::Uint` already refuses those
        // — which a `ty = "int"` or `ty = "any"` override replaces, so the reasoning did not
        // hold and the list is exhaustive rather than curated now.
        "u8" => fits!(u8),
        "u16" => fits!(u16),
        "u32" => fits!(u32),
        "u64" => fits!(u64),
        "usize" => fits!(usize),
        "i8" => fits!(i8),
        "i16" => fits!(i16),
        "i32" => fits!(i32),
        "i64" => fits!(i64),
        "isize" => fits!(isize),
        // The rule `FromValue for f32` follows: rounding a value that fits is ordinary
        // precision loss, and turning a finite one into an infinity is not.
        "f32" => match value {
            Const::Float(f) => !(*f as f32).is_infinite() || f.is_infinite(),
            _ => true,
        },
        "Vec" | "BTreeSet" | "HashSet" => match generic_arg(last, 0) {
            None => true,
            Some(inner) => match value {
                Const::List(items) => items.iter().all(|item| field_holds(&inner, item, position)),
                // One value where the container is: a choice naming an item, which the item
                // type is the thing that has to hold it. A default is not that — `admits` has
                // already refused a bare scalar there — so it does not reach this arm.
                _ => position == Position::Choice && field_holds(&inner, value, position),
            },
        },
        _ => true,
    }
}

fn infer_ty(ty: &syn::Type) -> Option<Ty> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    let name = last.ident.to_string();
    let generic = |index: usize| -> Option<syn::Type> { generic_arg(last, index) };
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

/// `x("mise.rust_type", "Duration")`: one tool-private key and one scalar value.
fn extension(meta: &Meta) -> syn::Result<(String, Const)> {
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "an extension is `x(\"tool.key\", value)`",
        ));
    };
    let values = list
        .parse_args_with(syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated)?;
    if values.len() != 2 {
        return Err(syn::Error::new_spanned(
            meta,
            "an extension takes exactly a string key and one scalar value, as in \
             `x(\"tool.key\", \"value\")`",
        ));
    }
    let mut values = values.into_iter();
    let key_expr = values.next().expect("length checked");
    let Expr::Lit(ExprLit {
        lit: Lit::Str(key), ..
    }) = key_expr
    else {
        return Err(syn::Error::new_spanned(
            key_expr,
            "an extension key must be a string",
        ));
    };
    if key.value().is_empty() {
        return Err(syn::Error::new_spanned(
            key,
            "an extension key cannot be empty",
        ));
    }
    let value_expr = values.next().expect("length checked");
    let value = const_expr(&value_expr)?;
    if matches!(value, Const::List(_)) {
        return Err(syn::Error::new_spanned(
            value_expr,
            "an extension value is one scalar, not a list",
        ));
    }
    Ok((key.value(), value))
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
    let prop_specs: Vec<TokenStream> = config
        .fields
        .iter()
        .filter_map(|field| match field {
            Field::Prop(prop) => Some(prop_spec(prop, &cfg)),
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
        let spec_parts: Vec<TokenStream> = config
            .fields
            .iter()
            .map(|field| match field {
                Field::Prop(prop) => {
                    let spec = prop_spec(prop, &cfg);
                    quote!(&[#spec])
                }
                Field::Flatten { ty, .. } => quote!(<#ty as #cfg::Props>::PROP_SPECS),
            })
            .collect();
        quote! {
            const __USAGE_PARTS: &[&[#cfg::PropMeta]] = &[#(#parts),*];
            const __USAGE_SPEC_PARTS: &[&[#cfg::PropSpec]] = &[#(#spec_parts),*];
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
            static __USAGE_PROP_SPECS: [#cfg::PropSpec; __USAGE_LEN] =
                #cfg::concat_prop_specs(__USAGE_SPEC_PARTS);
        }
    } else {
        quote! {
            static __USAGE_PROPS: [#cfg::PropMeta; #metas_len] = [#(#metas),*];
            static __USAGE_PROP_SPECS: [#cfg::PropSpec; #metas_len] = [#(#prop_specs),*];
        }
    };

    let sources = config.sources.iter().map(|source| {
        let kind = &source.kind;
        let name = option_str(&source.name);
        let doc_hint = option_str(&source.doc_hint);
        let set_hint = option_str(&source.set_hint);
        quote!(#cfg::SpecSource {
            kind: #kind,
            name: #name,
            doc_hint: #doc_hint,
            set_hint: #set_hint,
        })
    });
    let files = config.files.iter().flat_map(|file| {
        let path = &file.path;
        let format = option_str(&file.format);
        if let Some(base) = file.xdg {
            let (dirs, home) = match base {
                XdgBase::Config => (Some("$XDG_CONFIG_DIRS"), "$XDG_CONFIG_HOME"),
                XdgBase::Data => (Some("$XDG_DATA_DIRS"), "$XDG_DATA_HOME"),
                XdgBase::State => (None, "$XDG_STATE_HOME"),
                XdgBase::Cache => (None, "$XDG_CACHE_HOME"),
                XdgBase::Runtime => (None, "$XDG_RUNTIME_DIR"),
            };
            let global = format!("{home}/{path}");
            let mut expanded = Vec::new();
            if let Some(dirs) = dirs {
                let system = format!("{dirs}/{path}");
                expanded.push(quote!(#cfg::SpecFile {
                    path: #system,
                    findup: false,
                    scope: #cfg::FileScope::System,
                    format: #format,
                }));
            }
            expanded.push(quote!(#cfg::SpecFile {
                path: #global,
                findup: false,
                scope: #cfg::FileScope::Global,
                format: #format,
            }));
            expanded
        } else {
            let findup = file.findup;
            let scope = match file.scope {
                FileScope::Project => quote!(#cfg::FileScope::Project),
                FileScope::Global => quote!(#cfg::FileScope::Global),
                FileScope::System => quote!(#cfg::FileScope::System),
            };
            vec![quote!(#cfg::SpecFile {
                path: #path,
                findup: #findup,
                scope: #scope,
                format: #format,
            })]
        }
    });

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
                const PROP_SPECS: &'static [#cfg::PropSpec] = &__USAGE_PROP_SPECS;

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

                /// Metadata used only when lowering this declaration into a usage spec.
                pub const SETTINGS_SPEC: #cfg::ConfigSpec = #cfg::ConfigSpec::new(
                    <Self as #cfg::Props>::PROP_SPECS,
                    &[#(#sources),*],
                    &[#(#files),*],
                );

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

                /// This resolution's values, keeping every setting that reads.
                ///
                /// [`Self::read`] is all or nothing, which leaves a CLI two moves when one
                /// field is bad: refuse to start, or fall back to a struct of declared
                /// defaults and lose the environment and every config file along with the
                /// offending value. Neither is a choice this crate should be making.
                ///
                /// So: a field that will not read falls back to its own declared default and
                /// the rest keep what the merge gave them, with every failure returned
                /// alongside for the CLI to raise, log, or ignore as it sees fit. The errors
                /// are the same [`::usage_config::ReadError`]s [`Self::read`] returns, so a
                /// caller that decides a bad value *is* fatal has lost nothing by asking.
                ///
                /// `None` only where a setting has no value and no declared default — a hole
                /// in the declaration rather than a bad value, and nothing to fall back to.
                pub fn read_lossy(
                    __usage_resolved: &#cfg::Resolved,
                ) -> (::std::option::Option<Self>, #cfg::ReadErrors) {
                    let mut __usage_fold = __usage_resolved.fold_lossy();
                    let __usage_read = <Self as #cfg::Props>::read_at(&mut __usage_fold, 0);
                    (__usage_read, __usage_fold.into_errors())
                }

                /// The spec `config` block for these settings, as KDL.
                ///
                /// What documents, JSON schema and completions read. A CLI deriving
                /// `usage::Cli` names this type in `#[usage(config = ...)]` instead of
                /// calling this, and its `to_kdl` carries the block.
                pub fn spec_kdl() -> ::std::string::String {
                    #cfg::spec_kdl_with(Self::SETTINGS_PROPS, Self::SETTINGS_SPEC)
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

fn option_str(value: &Option<String>) -> TokenStream {
    match value {
        Some(value) => quote!(::std::option::Option::Some(#value)),
        None => quote!(::std::option::Option::None),
    }
}

fn prop_spec(prop: &Prop, cfg: &TokenStream) -> TokenStream {
    let help_heading = option_str(&prop.help_heading);
    let writes_to = option_str(&prop.writes_to);
    let extensions = prop.extensions.iter().map(|(key, value)| {
        let value = value.tokens(cfg);
        quote!((#key, #value))
    });
    quote! {
        #cfg::PropSpec {
            help_heading: #help_heading,
            writes_to: #writes_to,
            extensions: &[#(#extensions),*],
        }
    }
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
    fn a_list_default_is_held_against_the_choices_item_by_item() {
        // The choices on a list setting name what one item may be, so a default made of them
        // is a default made of declared values. Comparing the whole list against each choice
        // refused this outright — the opposite failure to the two above, and the one that
        // would have hit hk first: its list settings are exactly this shape.
        accepted(
            r#"
            struct Settings {
                #[usage(default("a"), choices("a", "b"))]
                tags: Vec<String>,
            }
        "#,
        );
        accepted(
            r#"
            struct Settings {
                #[usage(default("a", "b"), choices("a", "b"))]
                tags: Vec<String>,
            }
        "#,
        );

        // An item nothing declared is still refused, which is the point of checking at all.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(default("a", "z"), choices("a", "b"))]
                tags: Vec<String>,
            }
        "#,
        );
        assert!(
            err.contains("not one of the declared choices"),
            "unhelpful: {err}"
        );

        // And a scalar setting still compares whole, which is all it can do.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(default = "z", choices("a", "b"))]
                mode: String,
            }
        "#,
        );
        assert!(
            err.contains("the default is not one of the declared choices"),
            "unhelpful: {err}"
        );
    }

    #[test]
    fn a_choice_on_a_list_setting_names_one_item() {
        // A choice is compared against a resolved value *after* coercion, and a `list<string>`
        // setting's choices name what one item may be — that is what the registry compares.
        // Tightening the default check must not tighten this one.
        accepted(
            r#"
            struct Settings {
                #[usage(choices("a", "b"))]
                tags: Vec<String>,
            }
        "#,
        );
    }

    #[test]
    fn a_string_setting_spells_its_values_as_strings() {
        // A spec's `type="string"` does read a bare number as its text, and the merge coerces
        // one — so the registry would accept `choice 1` here. A declaration written in Rust
        // still has no reason to spell a string as anything else, and requiring the quotes is
        // what lets a default and a choice be compared as written. The alternative is a third
        // implementation of how a float is written, which is the drift
        // `conformance/tests/config_value_display.rs` exists to catch.
        for attribute in ["choices(1, 2)", "default = 1", "default = true"] {
            let err = rejection(&format!(
                r#"
                struct Settings {{
                    #[usage({attribute})]
                    level: String,
                }}
            "#
            ));
            assert!(
                err.contains("`string` can hold"),
                "`{attribute}` was accepted on a String field: {err}"
            );
        }

        // Quoted, they are the same declaration and they compile — including the pairing that
        // the literal comparison would otherwise have refused.
        accepted(
            r#"
            struct Settings {
                #[usage(default = "1", choices("1", "2"))]
                level: String,
            }
        "#,
        );
    }

    #[test]
    fn a_default_that_does_not_fit_the_field_is_refused() {
        // `infer_ty` collapses every unsigned width to `uint`, so the spec-level check cannot
        // see that a `u8` refuses 256. Uncaught, this is the negative-`uint` trap one level
        // down: seeded uncoerced, refused by `FromValue`, and so a type error on every
        // `Settings::read` that nothing at run time could fix.
        for (field, value) in [
            ("small: u8", "256"),
            ("small: u8", "-1"),
            ("signed: i8", "128"),
            ("medium: u16", "70000"),
            ("wide: u32", "5000000000"),
            ("ratio: f32", "1e300"),
        ] {
            let err = rejection(&format!(
                r#"
                struct Settings {{
                    #[usage(default = {value})]
                    {field},
                }}
            "#
            ));
            assert!(
                err.contains("does not fit") || err.contains("can hold"),
                "`default = {value}` was accepted on `{field}`: {err}"
            );
        }

        // A choice nothing could supply is the same mistake, and the items of a list are
        // measured against the item type rather than the list.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(choices(1, 256))]
                small: u8,
            }
        "#,
        );
        assert!(err.contains("does not fit"), "unhelpful: {err}");

        // A `ty` override can replace the spec type that was doing the refusing, so the
        // field's own type has to be the thing checked. `Ty::Uint` refuses a negative, but
        // `ty = "int"` and `ty = "any"` do not — and a `u64` still cannot hold one, so every
        // read failed on a default the derive had accepted.
        for ty in ["int", "any"] {
            let err = rejection(&format!(
                r#"
                struct Settings {{
                    #[usage(ty = "{ty}", default = -1)]
                    jobs: u64,
                }}
            "#
            ));
            assert!(
                err.contains("does not fit `u64`"),
                "`ty = \"{ty}\"` let a negative default past: {err}"
            );
        }
        // Through a list too, where the item is what cannot hold it.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(ty = "list<int>", choices(1, -1))]
                ports: Vec<u64>,
            }
        "#,
        );
        assert!(err.contains("does not fit"), "unhelpful: {err}");

        // A choice on a *list* setting names one item, so it is the item type that has to
        // hold it. Measured against `Vec<u16>` instead, a bare `70000` reached the container
        // arm and was called fitting — and `Ty::admits` allows a scalar as an item, so this
        // declared a choice no `u16` could ever be.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(choices(1, 70000))]
                ports: Vec<u16>,
            }
        "#,
        );
        assert!(err.contains("does not fit"), "unhelpful: {err}");
        accepted(
            r#"
            struct Settings {
                #[usage(choices(1, 65535))]
                ports: Vec<u16>,
            }
        "#,
        );
        let err = rejection(
            r#"
            struct Settings {
                #[usage(default(80, 70000))]
                ports: Vec<u16>,
            }
        "#,
        );
        assert!(err.contains("does not fit"), "unhelpful: {err}");

        // What does fit still compiles, at every width and through a list.
        for (field, value) in [
            ("small: u8", "255"),
            ("signed: i8", "-128"),
            ("medium: u16", "65535"),
            ("ratio: f32", "1.5"),
            ("jobs: u64", "4"),
        ] {
            accepted(&format!(
                r#"
                struct Settings {{
                    #[usage(default = {value})]
                    {field},
                }}
            "#
            ));
        }
        accepted(
            r#"
            struct Settings {
                #[usage(default(80, 443))]
                ports: Vec<u16>,
            }
        "#,
        );
    }

    #[test]
    fn a_ty_the_field_could_never_read_is_refused() {
        // The merge coerces to the *declared* type, so the shape the field is handed is
        // decided by `ty` and not by what a layer supplied. A pairing whose shapes disagree
        // therefore fails every `read` for every input — the same "accepted declaration,
        // broken at every read" trap as a default the field cannot hold.
        for (field, ty) in [
            ("name: String", "uint"),
            ("name: String", "bool"),
            ("jobs: u64", "string"),
            ("jobs: u64", "duration"),
            ("flag: bool", "string"),
            // A container declared over a scalar field, and the reverse.
            ("jobs: u64", "list<uint>"),
            ("ports: Vec<u64>", "uint"),
            (
                "table: std::collections::BTreeMap<String, String>",
                "string",
            ),
            // And the *item* types, not only the outer kind: the merge coerces a list's items
            // to the declared item type, so a list of the wrong thing fails on the first one.
            ("names: Vec<String>", "list<uint>"),
            ("ports: Vec<u64>", "list<string>"),
            ("ports: Vec<u64>", "set<string>"),
            ("nested: Vec<Vec<u64>>", "list<list<string>>"),
            (
                "table: std::collections::BTreeMap<String, String>",
                "map<string, bool>",
            ),
            // A list declared where a map is held, and the reverse.
            (
                "table: std::collections::BTreeMap<String, String>",
                "list<string>",
            ),
            ("names: Vec<String>", "map<string, string>"),
        ] {
            let err = rejection(&format!(
                r#"
                struct Settings {{
                    #[usage(ty = "{ty}")]
                    {field},
                }}
            "#
            ));
            assert!(
                err.contains("cannot be read into"),
                "`ty = \"{ty}\"` was accepted on `{field}`: {err}"
            );
        }

        // The pairings that do read. `duration` on a `String` is the reason the attribute
        // exists — a span of time is carried as its text, and the crate that owns the duration
        // type owns its spelling.
        for (field, ty) in [
            ("timeout: String", "duration"),
            ("home: std::path::PathBuf", "path"),
            ("home: std::path::PathBuf", "url"),
            // An integer is a perfectly good float, which `FromValue for f64` states.
            ("ratio: f64", "int"),
            ("ratio: f32", "float"),
            // `any` promises no shape, so it refuses none.
            ("whatever: String", "any"),
            // A widening the author may mean: it reads whenever the value fits, so it is
            // theirs to make rather than this check's to refuse.
            ("small: u8", "int"),
            // Item types that agree, at depth, and through a set — which the merge hands over
            // as a list like any other.
            ("names: Vec<String>", "list<string>"),
            ("paths: Vec<std::path::PathBuf>", "set<path>"),
            ("spans: Vec<String>", "list<duration>"),
            ("nested: Vec<Vec<u64>>", "list<list<uint>>"),
            (
                "table: std::collections::BTreeMap<String, String>",
                "map<string, string>",
            ),
            // `object` is a table whose value types the spec deliberately does not describe,
            // so there is nothing to hold the field's own to.
            (
                "table: std::collections::BTreeMap<String, String>",
                "object",
            ),
            // An item type this cannot measure keeps the escape hatch open at depth too.
            ("items: Vec<Custom>", "list<string>"),
        ] {
            accepted(&format!(
                r#"
                struct Settings {{
                    #[usage(ty = "{ty}")]
                    {field},
                }}
            "#
            ));
        }
    }

    #[test]
    fn a_ty_override_does_not_widen_what_the_field_holds() {
        // `ty` renames what the *spec* calls a setting; it does not change what the struct
        // holds, so it must not be a way around the width check. The two checks are separate
        // functions for exactly this reason — one reads `prop.ty`, the other `prop.read_ty`.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(ty = "int", default = 256)]
                small: u8,
            }
        "#,
        );
        assert!(err.contains("does not fit `u8`"), "unhelpful: {err}");

        // And a type this cannot measure stays permissive rather than refused: an alias for a
        // container is the ordinary reason `ty` is written at all, and refusing what it cannot
        // see would make the escape hatch useless.
        accepted(
            r#"
            struct Settings {
                #[usage(ty = "list<uint>", default(80, 443))]
                ports: Ports,
            }
        "#,
        );
    }

    #[test]
    fn two_settings_cannot_answer_to_one_name() {
        // `Registry::lookup` checks keys and aliases together and takes the first match, so a
        // collision does not fail — it makes one of the two settings unreachable by that name,
        // which is the quietest possible way to lose a setting. Every name has to be unique
        // among all of them, not just the keys among the keys.
        let cases = [
            // An alias over another setting's key.
            r#"
            struct Settings {
                #[usage(alias("other"))]
                jobs: u64,
                other: u64,
            }
            "#,
            // The same alias twice.
            r#"
            struct Settings {
                #[usage(alias("shared"))]
                jobs: u64,
                #[usage(alias("shared"))]
                threads: u64,
            }
            "#,
            // A key over an earlier setting's alias, which is the same collision found in the
            // other order.
            r#"
            struct Settings {
                #[usage(alias("threads"))]
                jobs: u64,
                threads: u64,
            }
            "#,
        ];
        for body in cases {
            let err = rejection(body);
            assert!(
                err.contains("could never be reached"),
                "a colliding name was accepted: {err}"
            );
        }

        // One field colliding with itself is not two settings fighting over a name, and
        // which of its own names came first says which mistake it is — so each gets its own
        // message rather than the two-settings one, which would name `jobs` twice and read
        // like nonsense.
        let err = rejection(
            r#"
            struct Settings {
                #[usage(alias("jobs"))]
                jobs: u64,
            }
        "#,
        );
        assert!(err.contains("alias of its own key"), "unhelpful: {err}");

        let err = rejection(
            r#"
            struct Settings {
                #[usage(alias("concurrency", "concurrency"))]
                jobs: u64,
            }
        "#,
        );
        assert!(
            err.contains("lists the alias `concurrency` twice"),
            "unhelpful: {err}"
        );

        // Distinct names are fine, including an alias that looks like a prefix of another key.
        accepted(
            r#"
            struct Settings {
                #[usage(alias("concurrency", "parallelism"))]
                jobs: u64,
                #[usage(alias("task-jobs"))]
                threads: u64,
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
            r#"help_heading = "Performance""#,
            r#"writes_to = "git""#,
            r#"x("tool.key", true)"#,
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

    #[test]
    fn spec_only_field_metadata_is_accepted() {
        accepted(
            r#"
            struct Settings {
                #[usage(
                    help_heading = "Performance",
                    writes_to = "git",
                    x("ex.rust_type", "u64"),
                    x("ex.restart_required", true)
                )]
                jobs: u64,
            }
        "#,
        );
    }

    #[test]
    fn a_config_source_and_file_are_struct_declarations() {
        accepted(
            r#"
            #[usage(source(kind = "git", name = "git config"))]
            #[usage(file(path = "ex.toml", findup, scope = "project"))]
            #[usage(file(path = "ex/config.toml", xdg = "config", format = "toml"))]
            struct Settings {
                jobs: u64,
            }
        "#,
        );

        let err = rejection(
            r#"
            #[usage(source(kind = "git"))]
            #[usage(source(kind = "git", name = "git"))]
            struct Settings {
                jobs: u64,
            }
        "#,
        );
        assert!(
            err.contains("source `git` is declared more than once"),
            "unhelpful: {err}"
        );

        let err = rejection(
            r#"
            #[usage(source = "git")]
            struct Settings {
                jobs: u64,
            }
        "#,
        );
        assert!(err.contains("source(kind ="), "unhelpful: {err}");

        let err = rejection(
            r#"
            #[usage(file(findup))]
            struct Settings {
                jobs: u64,
            }
        "#,
        );
        assert!(err.contains("needs `path"), "unhelpful: {err}");

        let err = rejection(
            r#"
            #[usage(file(path = "ex.toml", scope = "trusted"))]
            struct Settings {
                jobs: u64,
            }
        "#,
        );
        assert!(err.contains("not a config file scope"), "unhelpful: {err}");

        for declaration in [
            r#"file(path = "ex/config.toml", xdg = "config", findup)"#,
            r#"file(path = "ex/config.toml", xdg = "cache", scope = "global")"#,
        ] {
            let err = rejection(&format!(
                r#"
                #[usage({declaration})]
                struct Settings {{
                    jobs: u64,
                }}
                "#
            ));
            assert!(err.contains("base decides its scope"), "{err}");
        }

        let err = rejection(
            r#"
            #[usage(file(path = "ex/value", xdg = "local"))]
            struct Settings {
                jobs: u64,
            }
            "#,
        );
        assert!(err.contains("not an XDG base"), "{err}");

        let err = rejection(
            r#"
            struct Settings {
                #[usage(x("key"))]
                jobs: u64,
            }
        "#,
        );
        assert!(
            err.contains("exactly a string key and one scalar value"),
            "unhelpful: {err}"
        );
    }
}
