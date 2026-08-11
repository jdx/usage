//! Reading a Rust type into a description of a CLI.
//!
//! Everything here is about *understanding* the input. Nothing is emitted until
//! [`crate::codegen`], which keeps the error messages — the part an author
//! actually interacts with — in one place.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, Type};

/// A CLI, as declared by a struct.
pub struct Cli {
    pub ident: syn::Ident,
    pub name: String,
    pub bin: Option<String>,
    pub version: Option<String>,
    /// From the struct's doc comment: first paragraph, and the whole thing.
    pub about: Option<String>,
    pub long_about: Option<String>,
    pub fields: Vec<Field>,
}

/// One field, resolved to the thing it declares.
pub struct Field {
    pub ident: syn::Ident,
    /// The spec name, which is the field name with underscores turned into
    /// dashes, unless `name` says otherwise.
    pub name: String,
    pub kind: Kind,
    pub shape: Shape,
    pub help: Option<String>,
    pub long_help: Option<String>,
    pub env: Option<String>,
    pub default: Option<String>,
    pub help_heading: Option<String>,
    pub hide: bool,
    pub span: Span,
}

/// Whether a field is a flag or a positional, and how it is addressed.
pub enum Kind {
    Flag {
        longs: Vec<String>,
        shorts: Vec<char>,
        negate: Option<String>,
        global: bool,
        /// One occurrence takes several values.
        variadic: bool,
    },
    Arg {
        /// A `--` is required before this argument's value.
        double_dash_required: bool,
    },
}

/// What the field's Rust type says about how many values it holds.
///
/// Deliberately small. This version handles values as they arrive — as text —
/// because converting them is where required-ness, `choices`, and defaults live,
/// and that layer does not exist yet. A field of any other type is a compile
/// error rather than a surprise at runtime.
#[derive(PartialEq, Eq)]
pub enum Shape {
    /// `bool`: set or not.
    Bool,
    /// An integer counting occurrences, as in `-vvv`.
    Count,
    /// `Option<String>`: a value, or nothing.
    Optional,
    /// `String`: a value.
    Required,
    /// `Vec<String>`: every value given.
    Many,
}

impl Cli {
    pub fn from_input(input: &DeriveInput) -> syn::Result<Self> {
        let Data::Struct(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "usage::Cli describes a command's flags and arguments, so it needs a \
                 struct with named fields",
            ));
        };
        let Fields::Named(named) = &data.fields else {
            return Err(syn::Error::new_spanned(
                &data.fields,
                "usage::Cli needs named fields: the field name is what a flag or \
                 argument is called",
            ));
        };

        let (about, long_about) = doc_comment(&input.attrs)?;
        let mut cli = Cli {
            ident: input.ident.clone(),
            name: to_kebab(&input.ident.to_string()),
            bin: None,
            version: None,
            about,
            long_about,
            fields: Vec::new(),
        };

        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => cli.name = string_value(&meta)?,
                    "bin" => cli.bin = Some(string_value(&meta)?),
                    "version" => cli.version = Some(string_value(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a struct; usage::Cli takes \
                                 `name`, `bin`, and `version` here, and the description \
                                 comes from the doc comment"
                            ),
                        ));
                    }
                }
            }
        }

        for field in &named.named {
            cli.fields.push(Field::from_field(field)?);
        }
        cli.check()?;
        Ok(cli)
    }

    /// Reject declarations that would compile into a CLI nobody could use.
    fn check(&self) -> syn::Result<()> {
        let mut seen_long: Vec<(&str, Span)> = Vec::new();
        let mut seen_short: Vec<(char, Span)> = Vec::new();
        let mut variadic_arg: Option<Span> = None;

        for field in &self.fields {
            match &field.kind {
                Kind::Flag { longs, shorts, .. } => {
                    for long in longs {
                        if let Some((_, first)) = seen_long.iter().find(|(l, _)| l == long) {
                            return Err(dup(
                                field.span,
                                *first,
                                &format!("--{long} is declared twice"),
                            ));
                        }
                        seen_long.push((long, field.span));
                    }
                    for short in shorts {
                        if let Some((_, first)) = seen_short.iter().find(|(s, _)| s == short) {
                            return Err(dup(
                                field.span,
                                *first,
                                &format!("-{short} is declared twice"),
                            ));
                        }
                        seen_short.push((*short, field.span));
                    }
                }
                Kind::Arg { .. } => {
                    // A variadic takes everything left, so anything after it can
                    // never be filled.
                    if let Some(first) = variadic_arg {
                        return Err(dup(
                            field.span,
                            first,
                            "an argument after a variadic one can never be filled, \
                             because the variadic takes every remaining word",
                        ));
                    }
                    if field.shape == Shape::Many {
                        variadic_arg = Some(field.span);
                    }
                }
            }
        }
        Ok(())
    }
}

fn dup(span: Span, first: Span, message: &str) -> syn::Error {
    let mut err = syn::Error::new(span, message);
    err.combine(syn::Error::new(first, "first declared here"));
    err
}

impl Field {
    fn from_field(field: &syn::Field) -> syn::Result<Self> {
        let ident = field
            .ident
            .clone()
            .expect("named fields were checked by the caller");
        let span = field.span();
        let (help, long_help) = doc_comment(&field.attrs)?;

        let mut name = to_kebab(&ident.to_string());
        let mut longs: Vec<String> = Vec::new();
        let mut shorts: Vec<char> = Vec::new();
        let mut negate = None;
        let mut global = false;
        let mut variadic = false;
        let mut count = false;
        let mut double_dash_required = false;
        let mut env = None;
        let mut default = None;
        let mut help_heading = None;
        let mut hide = false;
        let mut is_arg = false;
        let mut explicit_long = false;

        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => name = string_value(&meta)?,
                    // Bare `long` takes the field name; `long = "x"` overrides it.
                    "long" => match &meta {
                        Meta::Path(_) => longs.push(name.clone()),
                        _ => {
                            longs.push(string_value(&meta)?);
                            explicit_long = true;
                        }
                    },
                    "short" => shorts.push(match &meta {
                        Meta::Path(_) => name.chars().next().ok_or_else(|| {
                            syn::Error::new_spanned(&path, "the field name is empty")
                        })?,
                        _ => char_value(&meta)?,
                    }),
                    "negate" => negate = Some(strip_dashes(&string_value(&meta)?)),
                    "global" => global = flag_value(&meta)?,
                    "var" => variadic = flag_value(&meta)?,
                    "count" => count = flag_value(&meta)?,
                    "hide" => hide = flag_value(&meta)?,
                    "arg" => is_arg = flag_value(&meta)?,
                    "env" => env = Some(string_value(&meta)?),
                    "default" => default = Some(string_value(&meta)?),
                    "help_heading" => help_heading = Some(string_value(&meta)?),
                    "double_dash" => {
                        let mode = string_value(&meta)?;
                        match mode.as_str() {
                            "required" => double_dash_required = true,
                            other => {
                                return Err(syn::Error::new_spanned(
                                    path,
                                    format!(
                                        "`double_dash = \"{other}\"` is not supported yet; \
                                         only \"required\" is"
                                    ),
                                ));
                            }
                        }
                    }
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}`; a field takes `name`, `long`, \
                                 `short`, `negate`, `global`, `var`, `count`, `hide`, \
                                 `arg`, `env`, `default`, `help_heading`, and \
                                 `double_dash`"
                            ),
                        ));
                    }
                }
            }
        }

        // A bare `long` written before `name` would have captured the field name
        // rather than the renamed one, so resolve it once everything is read.
        if !explicit_long {
            for long in &mut longs {
                *long = name.clone();
            }
        }

        let shape = Shape::from_type(&field.ty, count, span)?;
        let is_flag = !longs.is_empty() || !shorts.is_empty();
        if is_flag && is_arg {
            return Err(syn::Error::new(
                span,
                "a field is either a flag or a positional argument, not both: it has \
                 `long` or `short` as well as `arg`",
            ));
        }
        if !is_flag && count {
            return Err(syn::Error::new(
                span,
                "`count` counts how many times a flag was given, so the field needs a \
                 `long` or a `short`",
            ));
        }
        if !is_flag && negate.is_some() {
            return Err(syn::Error::new(
                span,
                "`negate` names a second long form, so the field needs a `long`",
            ));
        }

        let kind = if is_flag {
            if negate.is_some() && shape != Shape::Bool {
                return Err(syn::Error::new(
                    span,
                    "a negatable flag is on or off, so the field has to be `bool`",
                ));
            }
            Kind::Flag {
                longs,
                shorts,
                negate,
                global,
                variadic,
            }
        } else {
            if global {
                return Err(syn::Error::new(
                    span,
                    "`global` describes a flag that subcommands inherit; a positional \
                     argument belongs to one command",
                ));
            }
            Kind::Arg {
                double_dash_required,
            }
        };

        Ok(Field {
            ident,
            name,
            kind,
            shape,
            help,
            long_help,
            env,
            default,
            help_heading,
            hide,
            span,
        })
    }

    /// Whether this field's flag takes a value.
    pub fn takes_value(&self) -> bool {
        !matches!(self.shape, Shape::Bool | Shape::Count)
    }
}

impl Shape {
    fn from_type(ty: &Type, count: bool, span: Span) -> syn::Result<Self> {
        let name = type_name(ty);
        if count {
            return match name.as_str() {
                "u8" | "u16" | "u32" | "u64" | "usize" => Ok(Shape::Count),
                other => Err(syn::Error::new(
                    span,
                    format!(
                        "a `count` field holds how many times the flag was given, so it \
                         has to be an unsigned integer, not `{other}`"
                    ),
                )),
            };
        }
        match name.as_str() {
            "bool" => Ok(Shape::Bool),
            "String" => Ok(Shape::Required),
            "Option<String>" => Ok(Shape::Optional),
            "Vec<String>" => Ok(Shape::Many),
            other => Err(syn::Error::new(
                span,
                format!(
                    "`{other}` is not supported yet. This version binds values as the \
                     text they arrive as, so a field is `bool`, `String`, \
                     `Option<String>`, `Vec<String>`, or an unsigned integer with \
                     `count`. Parsing into other types arrives with the layer that also \
                     applies defaults and validates choices"
                ),
            )),
        }
    }
}

/// A type as a comparable string, with paths reduced to their last segment so
/// `std::option::Option<String>` reads as `Option<String>`.
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(path) => {
            let Some(last) = path.path.segments.last() else {
                return String::new();
            };
            let base = last.ident.to_string();
            match &last.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    let inner: Vec<String> = args
                        .args
                        .iter()
                        .map(|arg| match arg {
                            syn::GenericArgument::Type(t) => type_name(t),
                            _ => String::new(),
                        })
                        .collect();
                    format!("{base}<{}>", inner.join(", "))
                }
                _ => base,
            }
        }
        other => other.span().source_text().unwrap_or_default(),
    }
}

/// The `#[usage(...)]` attributes on an item.
fn attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|a| a.path().is_ident("usage"))
}

fn nested(attr: &Attribute) -> syn::Result<Vec<Meta>> {
    let list = attr.meta.require_list()?;
    let parsed = list
        .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)?;
    Ok(parsed.into_iter().collect())
}

fn ident_of(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

fn string_value(meta: &Meta) -> syn::Result<String> {
    let value = &meta.require_name_value()?.value;
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => Ok(s.value()),
        other => Err(syn::Error::new_spanned(
            other,
            "expected a string, as in `long = \"jobs\"`",
        )),
    }
}

fn char_value(meta: &Meta) -> syn::Result<char> {
    let value = &meta.require_name_value()?.value;
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Char(c), ..
        }) => Ok(c.value()),
        // `short = "j"` is the mistake anyone would make, so say what to write.
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => {
            let text = s.value();
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Err(syn::Error::new_spanned(
                    value,
                    format!("a short flag is a character: write `short = '{c}'`"),
                )),
                _ => Err(syn::Error::new_spanned(
                    value,
                    "a short flag is exactly one character",
                )),
            }
        }
        other => Err(syn::Error::new_spanned(
            other,
            "expected a character, as in `short = 'j'`",
        )),
    }
}

/// A boolean option, where the bare word means true: `global` or `global = true`.
fn flag_value(meta: &Meta) -> syn::Result<bool> {
    match meta {
        Meta::Path(_) => Ok(true),
        _ => {
            let value = &meta.require_name_value()?.value;
            match value {
                Expr::Lit(ExprLit {
                    lit: Lit::Bool(b), ..
                }) => Ok(b.value()),
                other => Err(syn::Error::new_spanned(
                    other,
                    "expected `true`, `false`, or the option on its own",
                )),
            }
        }
    }
}

/// Split a doc comment into the short help and the long help.
///
/// The first paragraph is the short form, matching what every Rust CLI framework
/// does and what an author expects from writing one; the whole comment is the long
/// form, and is only reported when it says more than the short one.
fn doc_comment(attrs: &[Attribute]) -> syn::Result<(Option<String>, Option<String>)> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        if let Meta::NameValue(nv) = &attr.meta {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &nv.value
            {
                lines.push(s.value().trim().to_string());
            }
        }
    }
    if lines.is_empty() {
        return Ok((None, None));
    }

    let full = lines.join("\n").trim().to_string();
    let short = full
        .split("\n\n")
        .next()
        .unwrap_or(&full)
        .replace('\n', " ")
        .trim()
        .to_string();

    let long = if full == short { None } else { Some(full) };
    let short = if short.is_empty() { None } else { Some(short) };
    Ok((short, long))
}

fn to_kebab(s: &str) -> String {
    s.replace('_', "-")
}

fn strip_dashes(s: &str) -> String {
    s.trim_start_matches('-').to_string()
}
