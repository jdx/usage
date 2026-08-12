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
    /// What this type's keys are derived from.
    ///
    /// The whole item rather than its name: two same-named structs in different
    /// modules would otherwise hash alike, and a macro cannot see a module path. Two
    /// types now have to be *identical* to collide, which the duplicate-key assertion
    /// still catches.
    pub fingerprint: String,
    pub name: String,
    pub bin: Option<String>,
    pub version: Option<String>,
    /// From the struct's doc comment: first paragraph, and the whole thing.
    pub about: Option<String>,
    pub long_about: Option<String>,
    /// Whether a flag-like token that names no flag is a value or an error. Unset
    /// means the spec's default, which is `value`.
    pub unknown_flags: Option<String>,
    pub fields: Vec<Field>,
}

/// One field, resolved to the thing it declares.
pub struct Field {
    pub ident: syn::Ident,
    /// The declared type, needed so a counting field's accumulator is the same
    /// integer the struct holds rather than an inferred one.
    pub ty: Type,
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
    /// The values this may take. Checked after the parse, since a choice list is
    /// about what a value *means* rather than which token it came from.
    pub choices: Vec<String>,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// Flags this one cannot be given with. Checked after the parse: whether a flag
    /// is unwelcome depends on the whole command line, not on the token itself.
    pub conflicts: Vec<String>,
    /// Flags whose presence makes this one necessary.
    pub required_if: Vec<String>,
    /// Flags whose presence makes this one unnecessary.
    pub required_unless: Vec<String>,
    pub hide: bool,
    /// Whether the flag may be given more than once, taking one value each time.
    ///
    /// Distinct from [`Kind::Flag::variadic`], which is one occurrence taking
    /// several values. Conflating the two makes a merely repeatable flag greedy
    /// enough to eat a positional — the same mistake the conformance harness made.
    pub repeatable: bool,
    pub span: Span,
}

/// Whether a field is a flag or a positional, and how it is addressed.
pub enum Kind {
    Flag {
        longs: Vec<String>,
        shorts: Vec<char>,
        negate: Option<String>,
        global: bool,
        /// One occurrence keeps taking values, as `--include <pattern>...` does in
        /// a spec. Greedy: it stops only at a flag-like token or `--`.
        variadic: bool,
    },
    Arg {
        /// A `--` is required before this argument's value.
        double_dash_required: bool,
    },
    /// Holds the enum whose variants are this command's subcommands.
    ///
    /// The type is carried rather than resolved, because the derive cannot see the
    /// enum's definition: the generated code names it through the
    /// [`Subcommands`](usage_argv::spec::Subcommands) trait, which is also how it
    /// reaches the type that accumulates a subcommand's values.
    Subcommand {
        /// The enum's type, as written.
        ty: syn::Type,
        /// Whether the field is `Option<T>`, and so may be left unfilled. A bare `T`
        /// says a subcommand is required, which is reported once the last token has
        /// been read.
        optional: bool,
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

        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "usage::Cli does not support generic parameters: the generated tables \
                 are `static` and every field has to be a concrete type it can bind a \
                 command-line value to",
            ));
        }

        let (about, long_about) = doc_comment(&input.attrs)?;
        let mut cli = Cli {
            ident: input.ident.clone(),
            fingerprint: quote::ToTokens::to_token_stream(input).to_string(),
            name: to_kebab(&input.ident.to_string()),
            bin: None,
            version: None,
            about,
            long_about,
            unknown_flags: None,
            fields: Vec::new(),
        };

        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => cli.name = string_value(&meta)?,
                    "bin" => cli.bin = Some(string_value(&meta)?),
                    "version" => cli.version = Some(string_value(&meta)?),
                    // A Rust CLI usually owns every flag it accepts, which is the
                    // case the stricter reading is for — but it is still opt-in,
                    // since a wrapper forwarding options wants the default.
                    "unknown_flags" => {
                        let mode = string_value(&meta)?;
                        if mode != "value" && mode != "error" {
                            return Err(syn::Error::new_spanned(
                                &path,
                                format!(
                                    "`unknown_flags = \"{mode}\"` is not a mode; write \
                                     \"value\" to pass an unrecognized flag on to the \
                                     positionals, or \"error\" to refuse it"
                                ),
                            ));
                        }
                        cli.unknown_flags = Some(mode);
                    }
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a struct; usage::Cli takes \
                                 `name`, `bin`, `version`, and `unknown_flags` here, \
                                 and the description comes from the doc comment"
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

    /// The field a flag selector names, as the spec spells one: `--long` or `-s`.
    ///
    /// A negation counts, since `--no-color` is another way to name the field `--color`
    /// declared — the two share one place to record whether they were given.
    pub fn field_for_selector(&self, selector: &str) -> Option<&Field> {
        self.fields.iter().find(|field| {
            let Kind::Flag {
                longs,
                shorts,
                negate,
                ..
            } = &field.kind
            else {
                return false;
            };
            match selector.strip_prefix("--") {
                Some(long) => longs.iter().chain(negate.iter()).any(|l| l == long),
                // A short is one character; `-abc` is three flags rather than a name.
                None => selector
                    .strip_prefix('-')
                    .and_then(|rest| {
                        let mut chars = rest.chars();
                        chars.next().filter(|_| chars.next().is_none())
                    })
                    .is_some_and(|short| shorts.contains(&short)),
            }
        })
    }

    /// Reject declarations that would compile into a CLI nobody could use.
    fn check(&self) -> syn::Result<()> {
        let mut seen_long: Vec<(&str, Span)> = Vec::new();
        let mut seen_short: Vec<(char, Span)> = Vec::new();
        let mut variadic_arg: Option<Span> = None;
        let mut subcommand_field: Option<Span> = None;

        for field in &self.fields {
            match &field.kind {
                Kind::Flag {
                    longs,
                    shorts,
                    negate,
                    ..
                } => {
                    // A negation is another long form, so it collides like one. Left
                    // unchecked, two flags could answer to the same token and only
                    // the first would ever be reached.
                    for long in longs.iter().chain(negate.iter()) {
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
                Kind::Subcommand { .. } => {
                    // At most one: two would each claim the word that selects a
                    // command, and only the first could ever be filled.
                    if let Some(first) = subcommand_field {
                        return Err(dup(
                            field.span,
                            first,
                            "a command has one set of subcommands, so only one field \
                             can hold them",
                        ));
                    }
                    subcommand_field = Some(field.span);
                }
            }
        }

        // Every relationship names a flag that exists. Resolving these at compile time
        // is the advantage of declaring them in code: a spec written by hand can only
        // find a typo'd selector at parse time, or never, since a selector naming
        // nothing quietly holds no relationship at all.
        for field in &self.fields {
            for (option, selectors) in [
                ("conflicts", &field.conflicts),
                ("required_if", &field.required_if),
                ("required_unless", &field.required_unless),
            ] {
                for selector in selectors {
                    let Some(target) = self.field_for_selector(selector) else {
                        return Err(syn::Error::new(
                            field.span,
                            format!(
                                "`{option} = \"{selector}\"` names no flag on this \
                                 command; write it as the spec does, `--long` or `-s`"
                            ),
                        ));
                    };
                    if target.ident == field.ident {
                        return Err(syn::Error::new(
                            field.span,
                            format!("`{option} = \"{selector}\"` names its own field"),
                        ));
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
    /// A field marked `#[usage(subcommand)]`, if this is one.
    fn subcommand(
        field: &syn::Field,
        ident: &syn::Ident,
        span: proc_macro2::Span,
    ) -> syn::Result<Option<Self>> {
        let mut is_subcommand = false;
        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                if ident_of(&meta.path().clone()) == "subcommand" {
                    if !matches!(meta, Meta::Path(_)) {
                        return Err(syn::Error::new_spanned(
                            meta.path(),
                            "`subcommand` takes no value: the enum it holds is the \
                             field's type",
                        ));
                    }
                    is_subcommand = true;
                }
            }
        }
        if !is_subcommand {
            return Ok(None);
        }

        // `Option<T>` says the subcommand may be left out; a bare `T` requires one,
        // reported once the last token has been read.
        let name = type_name(&field.ty);
        let (ty, optional) = match name
            .strip_prefix("Option<")
            .and_then(|rest| rest.strip_suffix('>'))
        {
            Some(inner) => (syn::parse_str::<Type>(inner)?, true),
            None => (field.ty.clone(), false),
        };

        Ok(Some(Field {
            ident: ident.clone(),
            ty: field.ty.clone(),
            name: to_kebab(&ident.to_string()),
            kind: Kind::Subcommand { ty, optional },
            // A subcommand field holds a command, not a value, so none of what
            // describes a value applies to it.
            shape: Shape::Bool,
            help: None,
            long_help: None,
            env: None,
            default: None,
            help_heading: None,
            choices: Vec::new(),
            var_min: None,
            var_max: None,
            conflicts: Vec::new(),
            required_if: Vec::new(),
            required_unless: Vec::new(),
            hide: false,
            repeatable: false,
            span,
        }))
    }

    fn from_field(field: &syn::Field) -> syn::Result<Self> {
        let ident = field
            .ident
            .clone()
            .expect("named fields were checked by the caller");
        let span = field.span();
        let (help, long_help) = doc_comment(&field.attrs)?;

        // A subcommand field is neither a flag nor an argument, and shares none of
        // their options, so it is recognized before any of them are read.
        if let Some(subcommand) = Self::subcommand(field, &ident, span)? {
            return Ok(subcommand);
        }

        let mut name = to_kebab(&ident.to_string());
        let mut longs: Vec<String> = Vec::new();
        let mut bare_longs = 0usize;
        let mut shorts: Vec<char> = Vec::new();
        let mut bare_shorts = 0usize;
        let mut negate = None;
        let mut global = false;
        let mut repeatable = false;
        let mut variadic = false;
        let mut count = false;
        let mut double_dash_required = false;
        let mut env = None;
        let mut default = None;
        let mut help_heading = None;
        let mut hide = false;
        let mut is_arg = false;
        let mut choices: Vec<String> = Vec::new();
        let mut var_min: Option<usize> = None;
        let mut var_max: Option<usize> = None;
        let mut conflicts: Vec<String> = Vec::new();
        let mut required_if: Vec<String> = Vec::new();
        let mut required_unless: Vec<String> = Vec::new();

        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    // Stripped, so a dashed spelling cannot leak into the spec name
                    // or into a long form derived from it.
                    "name" => name = strip_dashes(&string_value(&meta)?),
                    // Bare `long` takes the field name; `long = "x"` overrides it.
                    // A bare `long` is counted and resolved after the loop, so it
                    // picks up a `name` written later. Explicit ones are stored
                    // without dashes, because that is what a token is matched against
                    // once its `--` has been taken off: left verbatim,
                    // `long = "--no-color"` would be unreachable.
                    "long" => match &meta {
                        Meta::Path(_) => bare_longs += 1,
                        _ => longs.push(strip_dashes(&string_value(&meta)?)),
                    },
                    // Resolved after the loop, for the same reason a bare `long` is:
                    // `short` written before `name = "…"` would otherwise take the
                    // field's first letter rather than the renamed one's.
                    "short" => match &meta {
                        Meta::Path(_) => bare_shorts += 1,
                        _ => shorts.push(char_value(&meta)?),
                    },
                    "negate" => negate = Some(strip_dashes(&string_value(&meta)?)),
                    "global" => global = flag_value(&meta)?,
                    // Two different things, deliberately spelled the way a spec
                    // spells them: `var` is a flag that may be repeated, `variadic`
                    // is one occurrence that keeps taking values.
                    "var" => repeatable = flag_value(&meta)?,
                    "variadic" => variadic = flag_value(&meta)?,
                    "count" => count = flag_value(&meta)?,
                    "hide" => hide = flag_value(&meta)?,
                    "arg" => is_arg = flag_value(&meta)?,
                    "env" => env = Some(string_value(&meta)?),
                    // `choices("a", "b")` rather than one comma-joined string, so a
                    // value containing a comma is expressible.
                    "choices" => {
                        let Meta::List(list) = &meta else {
                            return Err(syn::Error::new_spanned(
                                meta.path(),
                                "`choices` takes a list, as in `choices(\"bash\", \"zsh\")`",
                            ));
                        };
                        choices = list
                            .parse_args_with(
                                syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
                            )?
                            .into_iter()
                            .map(|lit| lit.value())
                            .collect();
                        if choices.is_empty() {
                            return Err(syn::Error::new_spanned(
                                meta.path(),
                                "`choices` with nothing in it would accept nothing",
                            ));
                        }
                    }
                    // Both spellings the spec has: one target as a value, several as a
                    // list. A flag selector never contains a comma, so unlike `choices`
                    // there is nothing to lose by accepting the shorter form.
                    "conflicts" => conflicts = selectors(&meta)?,
                    "required_if" => required_if = selectors(&meta)?,
                    "required_unless" => required_unless = selectors(&meta)?,
                    "var_min" => var_min = Some(int_value(&meta)?),
                    "var_max" => var_max = Some(int_value(&meta)?),
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
                                 `short`, `negate`, `global`, `var`, `variadic`, \
                                 `count`, `hide`, `arg`, `env`, `default`, `choices`, \
                                 `var_min`, `var_max`, `conflicts`, `required_if`, \
                                 `required_unless`, `help_heading`, and `double_dash`"
                            ),
                        ));
                    }
                }
            }
        }

        // A bare `long` or `short` written before `name` would have captured the
        // field name rather than the renamed one, so resolve both once everything
        // has been read. Counted rather than rewritten, so a field carrying both a
        // bare and an explicit form keeps each.
        for _ in 0..bare_longs {
            longs.insert(0, name.clone());
        }
        for _ in 0..bare_shorts {
            let first = name.chars().next().ok_or_else(|| {
                syn::Error::new(span, "`short` needs a name to take its first letter from")
            })?;
            shorts.insert(0, first);
        }

        if name.is_empty() {
            return Err(syn::Error::new(
                span,
                "`name` is empty once its dashes are removed, and a flag needs \
                 something to be called",
            ));
        }
        if let Some(empty) = longs.iter().find(|l| l.is_empty()) {
            let _ = empty;
            return Err(syn::Error::new(
                span,
                "a `long` of only dashes leaves nothing to match: `--` ends flag \
                 parsing, so it can never name a flag",
            ));
        }

        // A short form is matched as a single byte, so a multi-byte character
        // could never be recognized. Better to say so than to truncate it.
        if let Some(short) = shorts.iter().find(|c| !c.is_ascii()) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`short = '{short}'` is not ASCII, and a short flag is matched as \
                     one byte. Use an ASCII letter, and give the long form the \
                     descriptive name"
                ),
            ));
        }
        // Some ASCII characters cannot reach a flag at all, because the grammar
        // spends them on something else first.
        if let Some(short) = shorts.iter().find(|c| c.is_ascii_digit()) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`short = '{short}'` can never be given: `-{short}` reads as a \
                     negative number, which the grammar treats as a value so that \
                     `--offset -1` works"
                ),
            ));
        }
        // Whitespace and control characters cannot survive the round trip either:
        // the spec writes a flag's forms as a space-delimited string, so `-\t`
        // becomes an unreadable declaration rather than a flag.
        if let Some(short) = shorts.iter().find(|c| c.is_whitespace() || c.is_control()) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`short = {short:?}` cannot be written: a spec spells a flag's \
                     forms as a space-delimited string, so whitespace and control \
                     characters have nowhere to go"
                ),
            ));
        }
        if let Some(short) = shorts.iter().find(|c| matches!(c, '-' | '=')) {
            let why = if *short == '-' {
                "`--` is the separator that ends flag parsing"
            } else {
                "`=` separates a short flag from its value, as in `-j=8`"
            };
            return Err(syn::Error::new(
                span,
                format!("`short = '{short}'` can never be given: {why}"),
            ));
        }

        let shape = Shape::from_type(&field.ty, count, span)?;
        // The spec records a default and the generated code applies it; anything it
        // cannot apply would be documented and then ignored.
        if let Some(value) = &default {
            match shape {
                Shape::Bool if value != "true" && value != "false" => {
                    return Err(syn::Error::new(
                        span,
                        format!(
                            "a `bool` field is on or off, so `default = \"{value}\"` \
                             cannot be applied; write \"true\" or \"false\""
                        ),
                    ));
                }
                Shape::Count => {
                    return Err(syn::Error::new(
                        span,
                        "a `count` field starts at zero, so a default has nothing to \
                         say",
                    ));
                }
                Shape::Many => {
                    return Err(syn::Error::new(
                        span,
                        "a default for a collecting field is not applied yet, so it \
                         would be documented and then ignored",
                    ));
                }
                _ => {}
            }
        }
        if !choices.is_empty() && matches!(shape, Shape::Bool | Shape::Count) {
            return Err(syn::Error::new(
                span,
                "a `bool` or counting field has no value to check against `choices`",
            ));
        }
        if let (Some(min), Some(max)) = (var_min, var_max) {
            if min > max {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`var_min = {min}` is more than `var_max = {max}`, so nothing \
                             could satisfy both"
                    ),
                ));
            }
        }
        if (var_min.is_some() || var_max.is_some()) && shape != Shape::Many {
            return Err(syn::Error::new(
                span,
                "`var_min` and `var_max` count values, so the field has to be a `Vec`",
            ));
        }
        if let Some(default) = &default {
            if !choices.is_empty() && !choices.iter().any(|c| c == default) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "the default `{default}` is not one of this field's choices, so \
                         it could never be valid"
                    ),
                ));
            }
        }

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
        if !is_flag && repeatable {
            return Err(syn::Error::new(
                span,
                "`var` describes a flag that may be repeated; a positional argument \
                 that takes several values is a `Vec` field",
            ));
        }
        if !is_flag && variadic {
            return Err(syn::Error::new(
                span,
                "`variadic` describes a flag whose one occurrence keeps taking values; \
                 a positional argument that takes several values is a `Vec` field",
            ));
        }
        if !is_flag && negate.is_some() {
            return Err(syn::Error::new(
                span,
                "`negate` names a second long form, so the field needs a `long`",
            ));
        }
        // Relationships hold between flags. The spec records them on a flag and has no
        // place for them on an argument, so accepting one here would enforce something
        // the emitted spec cannot say — and docs and completions would describe a
        // different CLI from the one that runs.
        for (option, selectors) in [
            ("conflicts", &conflicts),
            ("required_if", &required_if),
            ("required_unless", &required_unless),
        ] {
            if !selectors.is_empty() && !is_flag {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`{option}` describes a relationship between flags, so the field \
                         needs a `long` or a `short`"
                    ),
                ));
            }
        }
        // `required_unless` says the field may be absent when another flag stands in for
        // it. A bare `String` has nowhere to put absent, so its type would keep claiming
        // the value is mandatory and the exception could never take effect.
        if !required_unless.is_empty() && shape == Shape::Required {
            return Err(syn::Error::new(
                span,
                "`required_unless` says this may be left out, so the field needs \
                 somewhere to put \"absent\": make it an `Option`",
            ));
        }

        // A `Vec` flag collects, so it is repeatable whether or not it says so —
        // unless it is `variadic`, which is the other way of collecting. Emitting
        // both would claim a flag is repeatable *and* that its argument is variadic,
        // which the grammar treats as two different things.
        let repeatable = repeatable || (is_flag && shape == Shape::Many && !variadic);

        let kind = if is_flag {
            if variadic && repeatable {
                return Err(syn::Error::new(
                    span,
                    "`var` and `variadic` are two different ways to collect values — \
                     repeated occurrences, or one occurrence taking several — so a \
                     flag declares one or the other",
                ));
            }
            if variadic && shape != Shape::Many {
                return Err(syn::Error::new(
                    span,
                    "a `variadic` flag takes several values from one occurrence, so the \
                     field has to be a `Vec`; anything else would keep only the last",
                ));
            }
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
            if shape == Shape::Bool {
                return Err(syn::Error::new(
                    span,
                    "a positional argument holds the word it is given, so a `bool` \
                     field has nowhere to put it; add a `long` to make it a flag, or \
                     use `String`",
                ));
            }
            Kind::Arg {
                double_dash_required,
            }
        };

        Ok(Field {
            ident,
            ty: field.ty.clone(),
            name,
            kind,
            shape,
            help,
            long_help,
            env,
            default,
            help_heading,
            choices,
            var_min,
            var_max,
            conflicts,
            required_if,
            required_unless,
            hide,
            repeatable,
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

/// One flag selector as a value, or several as a list.
///
/// Selectors are written the way the spec writes them — `"--stdin"`, `"-s"` — rather
/// than as field names, so a declaration reads the same in Rust as it does in KDL.
/// Which flag each one names is resolved in [`Cli::check`], where every field is in
/// view.
fn selectors(meta: &Meta) -> syn::Result<Vec<String>> {
    let Meta::List(list) = meta else {
        return Ok(vec![string_value(meta)?]);
    };
    let found: Vec<String> = list
        .parse_args_with(
            syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
        )?
        .into_iter()
        .map(|lit| lit.value())
        .collect();
    // An empty list compiles into no relationship at all, which is a declaration that
    // reads as though it does something.
    if found.is_empty() {
        return Err(syn::Error::new_spanned(
            meta.path(),
            format!(
                "`{}` needs at least one flag, as in `{}(\"--other\")`",
                ident_of(meta.path()),
                ident_of(meta.path())
            ),
        ));
    }
    Ok(found)
}

fn int_value(meta: &Meta) -> syn::Result<usize> {
    let value = &meta.require_name_value()?.value;
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Int(i), ..
        }) => i.base10_parse(),
        other => Err(syn::Error::new_spanned(
            other,
            "expected a whole number, as in `var_min = 1`",
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

/// `my_flag` and `MyCli` both become `my-cli`-shaped names.
fn to_kebab(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch == '_' {
            out.push('-');
        } else if ch.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_dashes(s: &str) -> String {
    s.trim_start_matches('-').to_string()
}

/// An enum whose variants are a command's subcommands.
pub struct Subcommands {
    pub ident: syn::Ident,
    pub variants: Vec<Variant>,
}

/// One variant: a command name and the struct holding its flags and arguments.
pub struct Variant {
    pub ident: syn::Ident,
    /// The command name, which is the variant name in kebab-case unless `name`
    /// says otherwise.
    pub name: String,
    /// The struct the variant wraps.
    pub ty: Type,
    pub help: Option<String>,
    pub long_help: Option<String>,
}

impl Subcommands {
    pub fn from_input(input: &DeriveInput) -> syn::Result<Self> {
        let Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "usage::Subcommands describes a set of subcommands, so it needs an enum",
            ));
        };
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "usage::Subcommands does not support generic parameters: the generated \
                 tables are `static`",
            ));
        }

        let variants = data
            .variants
            .iter()
            .map(Variant::from_variant)
            .collect::<syn::Result<Vec<_>>>()?;

        let mut seen: Vec<(&str, Span)> = Vec::new();
        for variant in &variants {
            if let Some((_, first)) = seen.iter().find(|(n, _)| *n == variant.name) {
                return Err(dup(
                    variant.ty.span(),
                    *first,
                    &format!("two variants are both called `{}`", variant.name),
                ));
            }
            seen.push((&variant.name, variant.ty.span()));
        }

        // Two variants cannot wrap the same struct. A command's values are collected
        // into the struct that declares them, and its fields' keys come from that
        // struct — so two commands sharing one would collect into whichever was
        // reached first, and choosing between them would be a coin toss. Rejected
        // rather than silently misbound.
        //
        // Compared as whole paths: `type_name` renders only the last segment, so
        // `add::Op` and `remove::Op` looked identical and two perfectly good commands
        // were refused.
        let mut types: Vec<(String, Span)> = Vec::new();
        for variant in &variants {
            let rendered = quote::ToTokens::to_token_stream(&variant.ty)
                .to_string()
                .replace(' ', "");
            if let Some((_, first)) = types.iter().find(|(t, _)| *t == rendered) {
                return Err(dup(
                    variant.ty.span(),
                    *first,
                    &format!(
                        "two variants both wrap `{rendered}`, and a command collects \
                         into the struct that declares it — so give each command its \
                         own struct, even if the fields are identical"
                    ),
                ));
            }
            types.push((rendered, variant.ty.span()));
        }

        Ok(Subcommands {
            ident: input.ident.clone(),
            variants,
        })
    }
}

impl Variant {
    fn from_variant(variant: &syn::Variant) -> syn::Result<Self> {
        let (help, long_help) = doc_comment(&variant.attrs)?;
        let mut name = to_kebab(&variant.ident.to_string());

        for attr in attrs(&variant.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => name = strip_dashes(&string_value(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a variant; a subcommand \
                                 variant takes `name` here, and its description comes \
                                 from the doc comment"
                            ),
                        ));
                    }
                }
            }
        }

        // One unnamed field, holding the struct that declares the command's flags
        // and arguments. A variant with no fields would have nothing to parse into,
        // and named fields would make the variant itself the struct — which is a
        // second way to say the same thing.
        let ty = match &variant.fields {
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => unnamed.unnamed[0].ty.clone(),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "a subcommand variant wraps exactly one struct, as in \
                     `Install(Install)`: that struct is where its flags and arguments \
                     are declared",
                ));
            }
        };

        Ok(Variant {
            ident: variant.ident.clone(),
            name,
            ty,
            help,
            long_help,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;

    fn cli(body: &str) -> syn::Result<Cli> {
        Cli::from_input(&syn::parse_str::<syn::DeriveInput>(body).expect("valid Rust"))
    }

    /// The message a bad declaration produces, which is the part worth asserting on:
    /// `Cli` is not `Debug`, and the error is what the user sees.
    fn rejection(body: &str) -> String {
        match cli(body) {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn a_selector_resolves_by_long_short_or_negation() {
        let cli = cli(r#"
            struct Ex {
                #[usage(long, negate = "no-color")]
                color: bool,
                #[usage(short = 'f', long)]
                force: bool,
            }
        "#)
        .expect("should compile");

        let named = |selector: &str| {
            cli.field_for_selector(selector)
                .map(|f| f.ident.to_string())
        };
        assert_eq!(named("--color").as_deref(), Some("color"));
        assert_eq!(named("--no-color").as_deref(), Some("color"));
        assert_eq!(named("-f").as_deref(), Some("force"));
        assert_eq!(named("--force").as_deref(), Some("force"));
        assert_eq!(named("--nope"), None);
        // Not a name: `-fx` is two flags bundled, and a bare word is not a selector.
        assert_eq!(named("-fx"), None);
        assert_eq!(named("force"), None);
    }

    #[test]
    fn a_selector_naming_nothing_is_a_compile_error() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, conflicts = "--stdout")]
                out: Option<String>,
            }
        "#,
        );
        assert!(err.contains("names no flag"), "unhelpful message: {err}");
    }

    #[test]
    fn a_relationship_needs_a_flag_to_hold_between() {
        // The spec records these on a flag and has nowhere to put them on an argument,
        // so enforcing one here would describe a CLI the emitted spec does not.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long)]
                force: bool,
                #[usage(conflicts = "--force")]
                file: String,
            }
        "#,
        );
        assert!(
            err.contains("relationship between flags"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn required_unless_needs_somewhere_to_put_absent() {
        // A bare `String` is always filled, so the exception could never take effect:
        // the shape says mandatory and the attribute says conditional.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long)]
                url: Option<String>,
                #[usage(long, required_unless = "--url")]
                file: String,
            }
        "#,
        );
        assert!(
            err.contains("make it an `Option`"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn an_empty_relationship_list_is_a_compile_error() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, conflicts())]
                file: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("needs at least one flag"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn a_selector_naming_its_own_field_is_a_compile_error() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, required_unless = "--out")]
                out: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("names its own field"),
            "unhelpful message: {err}"
        );
    }
}
