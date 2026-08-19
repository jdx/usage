//! Reading a Rust type into a description of a CLI.
//!
//! Everything here is about *understanding* the input. Nothing is emitted until
//! [`crate::codegen`], which keeps the error messages — the part an author
//! actually interacts with — in one place.

use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro2::Span;
use syn::ext::IdentExt as _;
use syn::parse::Parser as _;
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, Type};

/// A CLI, as declared by a struct.
pub struct Cli {
    pub ident: syn::Ident,
    /// Whether this argument struct may be flattened into another command.
    pub composable: bool,
    /// Whether the declaration is a unit struct (`struct Command;`).
    ///
    /// Unit structs have the same empty command metadata as `{}` structs, but
    /// code generation must construct them without braces.
    pub unit: bool,
    /// What this type's keys are derived from.
    ///
    /// The whole item rather than its name: two same-named structs in different
    /// modules would otherwise hash alike, and a macro cannot see a module path. Two
    /// types now have to be *identical* to collide, which the duplicate-key assertion
    /// still catches.
    pub fingerprint: String,
    pub name: String,
    pub bin: Option<String>,
    /// Runtime program identity used by process help, version, diagnostics, and completions.
    /// The portable spec keeps the corresponding literal `name` / `bin` value.
    pub runtime_name: Option<proc_macro2::TokenStream>,
    pub runtime_bin: Option<proc_macro2::TokenStream>,
    /// The version the CLI reports, as the tokens for an `Option<&str>`.
    ///
    /// Tokens rather than a string because bare `version` means "the package's", and
    /// `env!("CARGO_PKG_VERSION")` has to be expanded in the *adopter's* crate — this one has
    /// its own version and it is not the answer. clap's bare `#[command(version)]` reads it the
    /// same way.
    pub version: Option<proc_macro2::TokenStream>,
    /// The expression printed for `--version` when the portable spec uses an
    /// explicitly supplied literal.
    pub runtime_version: Option<proc_macro2::TokenStream>,
    /// Whether this CLI answers a completion request.
    ///
    /// Opt in rather than supplied like `--help`: it is a hidden command a binary carries and a
    /// protocol a generated script depends on, so a CLI says when it wants both. The script
    /// generator is emitted under the same flag, which makes a script that calls a command the
    /// binary lacks a compile error rather than a puzzle at the prompt.
    pub completion: bool,
    /// Whether this CLI resolves settings, when nothing on the root itself says so.
    ///
    /// Only needed when every bound flag lives in a flattened group: the root cannot see another
    /// struct's fields, and generating the settings entry points unconditionally would make every
    /// CLI with a subcommand depend on `usage-config`. A root that binds a setting of its own has
    /// already said it, and does not need this.
    pub settings: bool,
    /// The oldest `usage` that can read the emitted spec, when the CLI says.
    ///
    /// Declared rather than computed. Working it out would mean a table from every property to
    /// the version that introduced it, kept in step by hand — and a table that silently rots
    /// produces a spec claiming to be readable by a `usage` that chokes on it, which is worse
    /// than saying nothing. The CLI knows which consumers it has to keep working.
    pub min_usage_version: Option<String>,
    /// An exact usage synopsis for shapes with explicit alternatives.
    pub usage: Option<String>,
    /// What running this command does to the world, when it says.
    ///
    /// Held as the tokens for an `Option<Effect>`, since the only thing it becomes is a field of
    /// a generated `static` — a second enum here would be a copy of the spec's to keep in step.
    pub effect: Option<proc_macro2::TokenStream>,
    /// Other names this command answers to.
    ///
    /// Declared on an `Args` struct so clap command attributes can migrate in place. A
    /// `Subcommands` variant may still add aliases for the particular route mounting it.
    pub aliases: Vec<String>,
    pub hidden_aliases: Vec<String>,
    /// Default casing for inferred flag and positional names.
    rename_all: Option<CasingStyle>,
    /// Default casing for environment names inferred by bare `env`.
    rename_all_env: CasingStyle,
    /// Where `#[usage(...)]` was written on the struct, when it was.
    ///
    /// Every position rule in [`Cli::check_position`] is about an attribute in the wrong place,
    /// so every one of them should point at the attribute. Spanning the struct's *name* put the
    /// underline one line below the thing that was wrong.
    pub attr_span: Option<proc_macro2::Span>,
    /// From the struct's doc comment: first paragraph, and the whole thing.
    pub about: Option<String>,
    pub long_about: Option<String>,
    /// Whether a flag-like token that names no flag is a value or an error. Unset
    /// means the spec's default, which is `value`.
    pub unknown_flags: Option<String>,
    /// Accept unambiguous prefixes of subcommands, inherited by children.
    pub infer_subcommands: bool,
    /// Accept unambiguous prefixes of long flags, inherited by children.
    pub infer_long_args: bool,
    /// The command a bare invocation means: `mise build` is `mise run build`.
    ///
    /// Only the root has one, and it is what mise sets by hand on the emitted spec today.
    pub default_subcommand: Option<String>,
    /// Whether argv[0]'s basename selects a subcommand (busybox-style applets).
    ///
    /// clap's `multicall`. Only the root has one: a spec declares it once, for the
    /// whole program. `parse()` rewrites the process's argv[0]; `parse_from` is
    /// unchanged, because the caller already decided the words.
    pub multicall: bool,
    /// Whether clap-shaped `try_parse_from` input omits argv0.
    pub no_binary_name: bool,
    /// Show help when no argv token follows this command's name.
    pub arg_required_else_help: bool,
    /// Declared descriptions, for the case a doc comment cannot express: a long form that does
    /// not contain the short one.
    pub about_attr: Option<proc_macro2::TokenStream>,
    pub long_about_attr: Option<proc_macro2::TokenStream>,
    /// Text around the rest of the help page. mise puts an Examples section in
    /// `after_long_help` on 115 commands, and a page without it is missing what a reader came
    /// for. Nothing derives these from the code, so they are declared.
    pub before_help: Option<proc_macro2::TokenStream>,
    /// Default help section for fields declared by this argument struct.
    pub next_help_heading: Option<String>,
    pub before_long_help: Option<proc_macro2::TokenStream>,
    pub after_help: Option<proc_macro2::TokenStream>,
    pub after_long_help: Option<proc_macro2::TokenStream>,
    /// The word that starts another invocation of the same command: mise's `:::`.
    pub restart_token: Option<String>,
    /// A command to run for subcommands discovered at completion time.
    ///
    /// Carried into the spec and nowhere else. The parser never runs it: a mount costs a
    /// subprocess, and completions are the cold path where that is affordable.
    pub mount: Option<String>,
    /// Groups declared on this command, with their properties.
    ///
    /// Membership is on the field — `#[usage(group = "input")]` — and only the two
    /// properties live here, because a group that says nothing but "these three are
    /// exclusive" should not need declaring twice.
    pub groups: Vec<GroupDecl>,
    pub fields: Vec<Field>,
}

/// A `#[usage(group("input", required))]` on the struct.
pub struct GroupDecl {
    pub name: String,
    pub required: bool,
    pub multiple: bool,
    pub span: Span,
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
    /// What one value converts into, absent for a switch or a count.
    ///
    /// The partial holds text either way — binding decides where a word lands, not what it
    /// means — and this is what `build` converts it with.
    pub value_ty: Option<Type>,
    /// Written as `Option<Vec<T>>`, so "never given" and "given nothing" differ.
    pub optional_collection: bool,
    /// Whether the words come from the type, via [`ValueEnum`].
    ///
    /// The alternative is `choices("a", "b")` written on the field, which is the same list
    /// kept in a second place. Both end up in the spec identically.
    pub value_enum: bool,
    pub help: Option<String>,
    pub long_help: Option<String>,
    pub env: Option<String>,
    /// The setting this flag sets, when the CLI resolves configuration.
    ///
    /// `#[usage(setting = "jobs")]`. The spec's `cli "--jobs"` node is the *documented* binding and
    /// this is the executable one; `Registry::drift` compares them, which is what hk's eighteen
    /// declared and five read `sources.cli` lines needed and never had.
    pub setting: Option<String>,
    /// The values the field takes when the command line supplies none.
    ///
    /// A list, because the spec's is: a collecting field can be given several, each `default`
    /// one item, in the order they are written. Every other shape holds at most one, which the
    /// model checks — a `String` has one place to put a value.
    pub default: Vec<String>,
    /// A typed Rust default evaluated when parsing starts. `default` remains
    /// the explicit portable spelling emitted in static metadata.
    pub default_value_t: Option<proc_macro2::TokenStream>,
    pub help_heading: Option<String>,
    /// What supplying this flag does to the world, when it says.
    ///
    /// A flag can only *raise* what its command does — `--dry-run` does not make a writing
    /// command read-only — which is the spec's rule and not this crate's to enforce.
    pub effect: Option<proc_macro2::TokenStream>,
    /// Whether a collecting argument needs at least one value.
    ///
    /// Required-ness is normally the type's to say: a bare `String` has nowhere to put
    /// "absent" and an `Option` does. A `Vec` has neither shape, so `<TARGET>…` — a spec's
    /// way of saying "one or more" — could not be declared at all, and came back as
    /// `[TARGET]…`. This is the one place it has to be stated rather than inferred.
    pub required_collection: bool,
    /// Whether the flag's value may be left off: `[BUMP]` rather than `<BUMP>`.
    ///
    /// Help and the emitted spec only. usage-lib's parser refuses a bare `--bump` exactly as it
    /// refuses a bare `--port`, so this binds nothing differently — which is why it is stated
    /// here and not inferred from the type, where `Option<String>` already means the *flag* is
    /// optional and says nothing about its value.
    pub value_optional: bool,
    /// The placeholder for a flag's value in help and in the emitted spec: `n` in
    /// `--jobs <n>`.
    ///
    /// Only meaningful on a flag that takes one. Without it the flag's own name stands in,
    /// which reads oddly when the two differ in case or shape — a spec saying
    /// `--tool <TOOL>` came back as `--tool <tool>`, because the name was all there was.
    pub value_name: Option<String>,
    /// The values this may take. Checked after the parse, since a choice list is
    /// about what a value *means* rather than which token it came from.
    pub choices: Vec<String>,
    /// Portable expr expression evaluated for each raw value after binding.
    pub validate: Option<String>,
    /// Message reported when `validate` returns false.
    pub validate_error: Option<String>,
    /// A Rust function that answers for this value when a shell asks.
    ///
    /// The counterpart of a spec's `run=`, and the source it is generated from: declaring the
    /// function is the only place a completer is said to exist.
    pub complete: Option<syn::Path>,
    /// A built-in completion class, in the spec's vocabulary (`path` or `dir`).
    pub complete_type: Option<String>,
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// Flags this one displaces. Applied while parsing rather than after it: the
    /// question is which of them came last, so the answer is decided by the token
    /// that arrives, not by the state it leaves behind.
    pub overrides: Vec<String>,
    /// Flags this one cannot be given with. Checked after the parse: whether a flag
    /// is unwelcome depends on the whole command line, not on the token itself.
    pub conflicts: Vec<String>,
    /// Flags that must also be given when this one is. Checked after the parse for the
    /// same reason `conflicts` is: the flag that satisfies the requirement may still be
    /// ahead of the one that imposes it.
    ///
    /// The same rule `required_if` states from the other end, and worth having both: this
    /// one lives on the flag the rule is about, which is where clap puts it and where a
    /// reader looks for it.
    pub requires: Vec<String>,
    /// Requirements activated by one of this flag's explicit values.
    pub requires_if: Vec<ConditionalRequirement>,
    /// Defaults that apply when another flag is given. First match wins.
    pub default_if: Vec<ConditionalDefault>,
    /// The character a value is split on, making one word several values.
    ///
    /// Only where several can land, so the field has to be a collection — checked at
    /// compile time, since a delimiter on a single-value field would drop everything
    /// after the first separator.
    pub delimiter: Option<char>,
    /// Whether a detached value may itself look like a flag.
    ///
    /// clap's `allow_hyphen_values`, and the spec's property of the same name. Only
    /// a flag that takes a value can declare it: there is nothing to take otherwise.
    pub allow_hyphen_values: bool,
    /// Whether the value must be attached with `=`. clap's `require_equals`.
    pub require_equals: bool,
    /// Value used when the flag is present but no value is given.
    ///
    /// clap's `default_missing_value`. `--color` binds this string; `--color=never`
    /// binds `never`. The field has to take a value.
    pub default_missing: Option<String>,
    /// Whether this flag must be given on its own — clap's `exclusive`.
    pub exclusive: bool,
    /// The group this flag belongs to, if any. Properties live on the group's own
    /// declaration; membership lives here, because a field is where a reader looks to
    /// see what a flag is part of.
    pub group: Option<String>,
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

#[derive(Debug, Clone)]
pub struct ConditionalRequirement {
    pub value: String,
    pub requires: String,
}

/// A default that applies when another flag is given.
///
/// Two arguments are clap's `ArgPredicate::IsPresent`; three are `Equals`.
#[derive(Debug, Clone)]
pub struct ConditionalDefault {
    pub selector: String,
    pub when: Option<String>,
    pub value: String,
}

/// How a positional argument relates to the `--` separator.
///
/// The spec's four modes, spelled as the spec spells them. Mirrors
/// [`usage_argv::DoubleDash`] rather than being it, because the derive builds its model
/// before it names the runtime — but the two must stay in step, which the round-trip test
/// in `codegen` checks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DoubleDash {
    /// Values may appear on either side of a `--`. The default.
    Optional,
    /// Values are accepted only after a `--`.
    Required,
    /// A `--` is kept as a value rather than consumed as a separator.
    Preserve,
    /// Once the argument takes a value, the rest of the command line is values.
    ///
    /// What a wrapper needs: `mise run build --watch` hands `--watch` to the task without
    /// the user typing a separator, and `mise --watch build` is still a flag mise reads.
    Automatic,
}

/// What a command or a flag does to the world, as the spec spells it.
///
/// Not something clap can express, so a CLI that wants it keeps a side table keyed by command
/// path and applies it to the generated spec afterwards — communique's `command_effects.rs` is
/// two hundred lines of exactly that. Declared beside the command instead, where the code that
/// does the thing is.
fn effect_value(meta: &Meta) -> syn::Result<proc_macro2::TokenStream> {
    let value = string_value(meta)?;
    let variant = match value.as_str() {
        "read" => quote::quote!(Read),
        "write" => quote::quote!(Write),
        "destructive" => quote::quote!(Destructive),
        other => {
            return Err(syn::Error::new_spanned(
                meta,
                format!(
                    "`effect = \"{other}\"` is not one the spec has; it takes \"read\" for \
                     something that only looks, \"write\" for something that changes what can \
                     be made again, and \"destructive\" for something that cannot be undone"
                ),
            ));
        }
    };
    Ok(quote::quote!(::core::option::Option::Some(
        usage_argv::spec::Effect::#variant
    )))
}

/// usage's shell-native `ValueHint`s lowered into the completion types the spec has.
fn value_hint(meta: &Meta) -> syn::Result<String> {
    let value = &meta.require_name_value()?.value;
    let Expr::Path(path) = value else {
        return Err(syn::Error::new_spanned(
            value,
            "`value_hint` takes a usage ValueHint variant, as in \
             `value_hint = usage::ValueHint::FilePath`",
        ));
    };
    let Some(variant) = path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            value,
            "`value_hint` needs a variant",
        ));
    };
    match variant.ident.to_string().as_str() {
        "FilePath" | "AnyPath" => Ok("path".to_string()),
        "DirPath" => Ok("dir".to_string()),
        "ExecutablePath" => Ok("executable".to_string()),
        "CommandName" | "CommandString" => Ok("command".to_string()),
        "CommandWithArguments" => Ok("command_args".to_string()),
        other => Err(syn::Error::new_spanned(
            value,
            format!(
                "`ValueHint::{other}` has no usage completion type yet; supported hints are \
                 `FilePath`, `AnyPath`, `DirPath`, `ExecutablePath`, `CommandName`, \
                 `CommandString`, and `CommandWithArguments`"
            ),
        )),
    }
}

/// Whether a field is a flag or a positional, and how it is addressed.
pub enum Kind {
    Flag {
        longs: Vec<String>,
        /// Long aliases accepted by the parser but omitted from help and completion.
        hidden_longs: Vec<String>,
        shorts: Vec<char>,
        negate: Option<String>,
        global: bool,
        /// One occurrence keeps taking values, as `--include <pattern>...` does in
        /// a spec. Greedy: it stops only at a flag-like token or `--`.
        variadic: bool,
    },
    Arg {
        /// How this argument relates to the `--` separator.
        double_dash: DoubleDash,
    },
    /// Holds the enum whose variants are this command's subcommands.
    ///
    /// The type is carried rather than resolved, because the derive cannot see the
    /// enum's definition: the generated code names it through the
    /// [`Subcommands`](usage_argv::spec::Subcommands) trait, which is also how it
    /// reaches the type that accumulates a subcommand's values.
    /// Holds a struct whose flags and arguments belong to *this* command.
    ///
    /// A Rust-side device for sharing declarations between commands — mise writes one
    /// `ConfigLs` and gives it to both `config` and `config ls`. The spec has no such idea,
    /// and does not need one: the emitted KDL lists the flags inline, exactly as a
    /// hand-written command would.
    ///
    /// The type is carried rather than resolved, as for a subcommand: the derive cannot see
    /// the other struct's fields, so everything goes through
    /// [`CommandArgs`](usage_argv::spec::CommandArgs) — including the tables, which are
    /// joined into the parent's at compile time.
    Flatten {
        /// The struct's type, as written.
        ty: syn::Type,
    },
    /// A field that is not an argument at all, filled from `Default`.
    ///
    /// clap's `#[arg(skip)]`: the struct still holds the field so a rewrite can keep
    /// computed state beside parsed state, and nothing about it reaches the spec, the
    /// parse tables, or help. The type has to implement `Default`, which is also how
    /// clap fills one.
    Skip,
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
                "usage::Cli describes a command's flags and arguments, so it needs a struct",
            ));
        };
        match &data.fields {
            Fields::Named(_) | Fields::Unit => {}
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    &data.fields,
                    "usage derives do not infer whether a tuple field is a positional or a \
                     flattened Args type; rewrite it as a named field, using \
                     `#[usage(flatten)]` for an Args wrapper",
                ));
            }
        }

        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "usage::Cli does not support generic parameters: the generated tables \
                 are `static` and every field has to be a concrete type it can bind a \
                 command-line value to",
            ));
        }

        let mut name_given = false;
        let mut name_spec_given = false;
        let mut name_needs_spec = false;
        let mut bin_spec_given = false;
        let mut bin_needs_spec = false;
        let mut verbatim_doc_comment = false;
        let mut version_spec_given = false;
        let mut version_needs_spec = false;
        let mut cli = Cli {
            ident: input.ident.clone(),
            composable: false,
            unit: matches!(&data.fields, Fields::Unit),
            fingerprint: quote::ToTokens::to_token_stream(input).to_string(),
            name: to_kebab(&input.ident.to_string()),
            bin: None,
            runtime_name: None,
            runtime_bin: None,
            completion: false,
            settings: false,
            min_usage_version: None,
            usage: None,
            effect: None,
            aliases: Vec::new(),
            hidden_aliases: Vec::new(),
            rename_all: None,
            rename_all_env: CasingStyle::ScreamingSnake,
            attr_span: input
                .attrs
                .iter()
                .find(|a| a.path().is_ident("usage") || a.path().is_ident("command"))
                .map(|a| a.path().span()),
            version: None,
            runtime_version: None,
            about: None,
            long_about: None,
            unknown_flags: None,
            infer_subcommands: false,
            infer_long_args: false,
            default_subcommand: None,
            multicall: false,
            no_binary_name: false,
            arg_required_else_help: false,
            about_attr: None,
            long_about_attr: None,
            before_help: None,
            next_help_heading: None,
            before_long_help: None,
            after_help: None,
            after_long_help: None,
            restart_token: None,
            mount: None,
            groups: Vec::new(),
            fields: Vec::new(),
        };

        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => {
                        let Meta::NameValue(value) = &meta else {
                            return Err(syn::Error::new_spanned(
                                &meta,
                                "`name` takes a string literal or Rust expression",
                            ));
                        };
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(literal),
                            ..
                        }) = &value.value
                        {
                            cli.name = literal.value();
                        } else {
                            cli.runtime_name = Some(quote::ToTokens::to_token_stream(&value.value));
                            name_needs_spec = true;
                        }
                        name_given = true;
                    }
                    "name_spec" => {
                        cli.name = string_value(&meta)?;
                        name_spec_given = true;
                        name_given = true;
                    }
                    "bin" => {
                        let Meta::NameValue(value) = &meta else {
                            return Err(syn::Error::new_spanned(
                                &meta,
                                "`bin` takes a string literal or Rust expression",
                            ));
                        };
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(literal),
                            ..
                        }) = &value.value
                        {
                            cli.bin = Some(literal.value());
                        } else {
                            cli.runtime_bin = Some(quote::ToTokens::to_token_stream(&value.value));
                            bin_needs_spec = true;
                        }
                    }
                    "bin_spec" => {
                        cli.bin = Some(string_value(&meta)?);
                        bin_spec_given = true;
                    }
                    // Through the same helper as `global` and `var`, so `completion = false`
                    // means false rather than being read as the bare word with something
                    // decorative after it.
                    "completion" => cli.completion = flag_value(&meta)?,
                    "settings" => cli.settings = flag_value(&meta)?,
                    "verbatim_doc_comment" => verbatim_doc_comment = flag_value(&meta)?,
                    "effect" => cli.effect = Some(effect_value(&meta)?),
                    "alias" => cli.aliases.extend(selectors(&meta)?),
                    "alias_hidden" => cli.hidden_aliases.extend(selectors(&meta)?),
                    "min_usage_version" => cli.min_usage_version = Some(string_value(&meta)?),
                    "usage" => cli.usage = Some(string_value(&meta)?),
                    "version" => {
                        let value = match &meta {
                            // `version` on its own: whatever the adopter's package says.
                            Meta::Path(_) => quote::quote!(env!("CARGO_PKG_VERSION")),
                            Meta::NameValue(value) => {
                                quote::ToTokens::to_token_stream(&value.value)
                            }
                            Meta::List(_) => {
                                return Err(syn::Error::new_spanned(
                                    &meta,
                                    "`version` is a Rust expression, as in `version = build::VERSION`, or is written on its own for the package version",
                                ));
                            }
                        };
                        cli.runtime_version = Some(value.clone());
                        if let Meta::NameValue(value) = &meta {
                            version_needs_spec =
                                matches!(value.value, Expr::Call(_) | Expr::MethodCall(_));
                        }
                        if !version_spec_given {
                            cli.version = Some(value);
                        }
                    }
                    "version_spec" => {
                        let literal = string_value(&meta)?;
                        cli.version = Some(quote::quote!(#literal));
                        version_spec_given = true;
                    }
                    // A doc comment's long form always contains its short one — the short form
                    // *is* the comment's first paragraph. A spec keeps `about` and `about_long`
                    // independent, and mise's differ entirely: "Dev tools, env vars, and tasks
                    // in one CLI" against "mise prepares your development environment before
                    // each command runs." There is no comment that says both, so they can be
                    // declared.
                    "about" => cli.about_attr = Some(metadata_expr(&meta)?),
                    "long_about" => cli.long_about_attr = Some(metadata_expr(&meta)?),
                    "before_help" => cli.before_help = Some(metadata_expr(&meta)?),
                    "next_help_heading" => cli.next_help_heading = Some(string_value(&meta)?),
                    "before_long_help" => cli.before_long_help = Some(metadata_expr(&meta)?),
                    "after_help" => cli.after_help = Some(metadata_expr(&meta)?),
                    "after_long_help" => cli.after_long_help = Some(metadata_expr(&meta)?),
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
                    "default_subcommand" => {
                        cli.default_subcommand = Some(strip_dashes(&string_value(&meta)?))
                    }
                    "multicall" => cli.multicall = flag_value(&meta)?,
                    "no_binary_name" => cli.no_binary_name = flag_value(&meta)?,
                    "arg_required_else_help" => cli.arg_required_else_help = flag_value(&meta)?,
                    "infer_subcommands" => cli.infer_subcommands = flag_value(&meta)?,
                    "infer_long_args" => cli.infer_long_args = flag_value(&meta)?,
                    "restart_token" => cli.restart_token = Some(string_value(&meta)?),
                    "mount" => cli.mount = Some(string_value(&meta)?),
                    "group" => cli.groups.push(group_decl(&meta)?),
                    "rename_all" => cli.rename_all = Some(CasingStyle::parse(&meta)?),
                    "rename_all_env" => cli.rename_all_env = CasingStyle::parse(&meta)?,
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a struct; usage::Cli takes \
                                 `name`, `name_spec`, `bin`, `bin_spec`, `version`, `version_spec`, `usage`, `verbatim_doc_comment`, `unknown_flags`, \
                                 `default_subcommand`, `multicall`, `no_binary_name`, `arg_required_else_help`, `infer_subcommands`, \
                                 `infer_long_args`, `next_help_heading`, `restart_token`, `mount` and \
                                 `group` here, and the description comes from the doc \
                                 comment"
                            ),
                        ));
                    }
                }
            }
        }

        (cli.about, cli.long_about) = doc_comment(&input.attrs, verbatim_doc_comment)?;

        if version_needs_spec && !version_spec_given {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "a computed `version` needs `version_spec = \"...\"`: the expression is evaluated for `--version`, while the literal is emitted into the portable spec",
            ));
        }
        if name_needs_spec && !name_spec_given {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "a computed `name` needs `name_spec = \"...\"`: the expression is evaluated for process output, while the literal is emitted into the portable spec",
            ));
        }
        if bin_needs_spec && !bin_spec_given {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "a computed `bin` needs `bin_spec = \"...\"`: the expression is evaluated for process output, while the literal is emitted into the portable spec",
            ));
        }

        let alias_span = cli.attr_span.unwrap_or_else(Span::call_site);
        let mut seen_aliases: Vec<(&str, Span)> = Vec::new();
        for alias in cli.aliases.iter().chain(&cli.hidden_aliases) {
            if alias.is_empty() {
                return Err(syn::Error::new(
                    alias_span,
                    "an alias with no name would answer to nothing",
                ));
            }
            if let Some((_, first)) = seen_aliases.iter().find(|(name, _)| *name == alias) {
                return Err(dup(
                    alias_span,
                    *first,
                    &format!("`{alias}` is declared twice as an alias for this command"),
                ));
            }
            seen_aliases.push((alias, alias_span));
        }

        for field in data.fields.iter() {
            cli.fields.push(Field::from_field(
                field,
                cli.rename_all,
                cli.rename_all_env,
            )?);
        }
        if let Some(heading) = &cli.next_help_heading {
            for field in &mut cli.fields {
                if matches!(field.kind, Kind::Flag { .. } | Kind::Arg { .. })
                    && field.help_heading.is_none()
                {
                    field.help_heading = Some(heading.clone());
                }
            }
        }
        // A program is called what its binary is called, unless it says otherwise. The name
        // defaults to the struct's, and a struct is usually called `Cli` — so `bin =
        // "communique"` with no `name` bannered itself as `cli 1.3.1`, and every adopter had
        // to declare the same word twice to avoid it.
        if !name_given {
            if let Some(bin) = &cli.bin {
                cli.name = bin.clone();
            }
            cli.runtime_name = cli.runtime_bin.clone();
        }
        cli.check()?;
        Ok(cli)
    }

    /// The field a flag selector names, as the spec spells one: `--long` or `-s`.
    ///
    /// A negation counts, since `--no-color` is another way to name the field `--color`
    /// declared — the two share one place to record whether they were given.
    /// A position error, pointed at the attribute rather than at the struct's name.
    ///
    /// Every rule in [`check_position`](Self::check_position) is about an attribute written in
    /// the wrong place, so the underline belongs on the attribute. Falls back to the name for a
    /// struct that has none, which cannot reach these rules but keeps the helper total.
    fn misplaced(&self, ident: &syn::Ident, message: impl std::fmt::Display) -> syn::Error {
        match self.attr_span {
            Some(span) => syn::Error::new(span, message),
            None => syn::Error::new_spanned(ident, message),
        }
    }

    /// Check the command-level properties against where this struct sits in the tree.
    ///
    /// Both derives share this parse, so these rules cannot live inside it: the same
    /// attribute is required at the root and a mistake below it, or the reverse. Called with
    /// `is_root` from each derive.
    ///
    /// The *name* a `default_subcommand` gives is not checked here. It is resolved by
    /// `usage_argv::find_subcommand` during const evaluation, so a name no subcommand answers
    /// to fails to compile there.
    ///
    /// Every rule here exists because the *spec* cannot express the thing anywhere else.
    /// Without them a declaration is accepted, dropped on the way to the KDL, and missed —
    /// or, for a root `mount`, trips a `debug_assert!` in the writer, which is a poor way to
    /// learn that an attribute was in the wrong place.
    pub fn check_position(&self, ident: &syn::Ident, is_root: bool) -> syn::Result<()> {
        if !is_root {
            if self.runtime_name.is_some() || self.runtime_bin.is_some() {
                return Err(self.misplaced(
                    ident,
                    "computed `name` and `bin` belong on the root `#[derive(Cli)]`: nested command names are static parser routing keys",
                ));
            }
            // The completion command answers for the whole CLI, so it is declared where the
            // whole CLI is. Accepted silently on an `Args`, it generated nothing and said
            // nothing, which reads as a CLI that has completions and does not.
            if self.completion {
                return Err(self.misplaced(
                    ident,
                    "`completion` belongs on the root, where `#[derive(Cli)]` is: the hidden \
                     command it adds answers for the whole program, not for one of its commands",
                ));
            }
            // Only a root can be in the position this describes. A group is asked for its
            // settings by whatever flattens it, and answers whenever it has any — so the
            // attribute has nothing left to say here, and saying it would read as the group
            // having asked for something.
            if self.settings {
                return Err(self.misplaced(
                    ident,
                    "`settings` belongs on the root, where `#[derive(Cli)]` is: it says that \
                     this CLI resolves settings whose flags are declared elsewhere, and a group \
                     is asked for its own by whoever flattens it",
                ));
            }
            // One spec, one claim about which `usage` can read it — and only the root writes a
            // spec at all, so a command declaring it was storing a value with nowhere to go.
            if self.min_usage_version.is_some() {
                return Err(self.misplaced(
                    ident,
                    "`min_usage_version` belongs on the root, where `#[derive(Cli)]` is: it is \
                     one claim about the whole emitted spec, and only the root emits one",
                ));
            }
            // A spec declares one `default_subcommand`, at the top.
            if self.default_subcommand.is_some() {
                return Err(self.misplaced(
                    ident,
                    "`default_subcommand` belongs on the root, where `#[derive(Cli)]` is: a \
                     spec declares one for the whole program, not one per command",
                ));
            }
            if self.multicall {
                return Err(self.misplaced(
                    ident,
                    "`multicall` belongs on the root, where `#[derive(Cli)]` is: a spec \
                     declares it once for the whole program, not one per command",
                ));
            }
            if self.no_binary_name {
                return Err(self.misplaced(
                    ident,
                    "`no_binary_name` belongs on the root, where `#[derive(Cli)]` is: it \
                     selects the input contract of the whole CLI's clap-shaped parser",
                ));
            }
            return Ok(());
        }

        // `settings` says "this CLI resolves settings whose flags are declared elsewhere", so
        // there has to be an elsewhere: a field that binds one, a flattened group, or a
        // subcommand. With none of the three the attribute describes nothing, and the generated
        // layer called a `settings_given` that was never emitted — so an adopter's build failed
        // with `cannot find function settings_given` pointing at `#[derive(Cli)]`, which names
        // neither the attribute nor the mistake.
        if self.settings
            && !self.fields.iter().any(|f| {
                f.setting.is_some()
                    || matches!(f.kind, Kind::Flatten { .. } | Kind::Subcommand { .. })
            })
        {
            return Err(self.misplaced(
                ident,
                "`settings` says this CLI resolves settings whose flags are declared \
                 elsewhere, and there is no elsewhere: nothing here binds a setting, and \
                 there is no flattened group or subcommand that could. Add `setting = \"…\"` \
                 to the flag that sets one, or drop the attribute",
            ));
        }

        // `mount` and `restart_token` are written on a `cmd` node, and the root is not one.
        // Verified against usage-lib, which rejects a spec that puts either at the top.
        for (present, what) in [
            (self.mount.is_some(), "mount"),
            (self.restart_token.is_some(), "restart_token"),
            // Bare `communique` does nothing to the world; one of its commands does. The spec
            // writer asserts the root carries none, so declaring one here would trip a
            // `debug_assert!` in the writer rather than say anything.
            (self.effect.is_some(), "effect"),
            (!self.aliases.is_empty(), "alias"),
            (!self.hidden_aliases.is_empty(), "alias_hidden"),
        ] {
            if present {
                return Err(self.misplaced(
                    ident,
                    format!(
                        "`{what}` belongs on a command, not on the root: the spec accepts it \
                         only inside a `cmd` block, so declaring it here would be dropped on \
                         the way to the KDL"
                    ),
                ));
            }
        }

        if self.default_subcommand.is_some()
            && !self
                .fields
                .iter()
                .any(|f| matches!(f.kind, Kind::Subcommand { .. }))
        {
            return Err(self.misplaced(
                ident,
                "`default_subcommand` names the command a bare invocation means, and this \
                 one has no subcommands to name",
            ));
        }
        if self.multicall
            && !self
                .fields
                .iter()
                .any(|f| matches!(f.kind, Kind::Subcommand { .. }))
        {
            return Err(self.misplaced(
                ident,
                "`multicall` treats argv[0] as a subcommand, and this one has no \
                 subcommands to select",
            ));
        }
        // The subcommand enum is a separate macro expansion, so its external variants
        // are not visible here. Code generation also asserts that the enum's COMMANDS
        // table contains a named subcommand; external catch-alls are absent from it.
        Ok(())
    }

    /// How a group names one of its member fields in the emitted spec.
    ///
    /// Flags use their long form when there is one, or their short form otherwise.
    /// Positionals use their bare argument name.
    pub fn selector_for_field(field: &Field) -> Option<String> {
        match &field.kind {
            Kind::Flag { longs, shorts, .. } => longs
                .first()
                .map(|long| format!("--{long}"))
                .or_else(|| shorts.first().map(|short| format!("-{short}"))),
            Kind::Arg { .. } => Some(field.name.clone()),
            _ => None,
        }
    }

    pub fn field_for_selector(&self, selector: &str) -> Option<&Field> {
        self.fields.iter().find(|field| {
            match &field.kind {
                Kind::Flag {
                    longs,
                    shorts,
                    negate,
                    ..
                } => match selector.strip_prefix("--") {
                    Some(long) => longs.iter().chain(negate.iter()).any(|l| l == long),
                    // A short is one character; `-abc` is three flags rather than a name.
                    None => selector
                        .strip_prefix('-')
                        .and_then(|rest| {
                            let mut chars = rest.chars();
                            chars.next().filter(|_| chars.next().is_none())
                        })
                        .is_some_and(|short| shorts.contains(&short)),
                },
                Kind::Arg { .. } => {
                    selector == field.name || selector == to_kebab(&field.ident.to_string())
                }
                _ => false,
            }
        })
    }

    /// Reject declarations that would compile into a CLI nobody could use.
    fn check(&self) -> syn::Result<()> {
        let mut seen_long: Vec<(&str, Span)> = Vec::new();
        let mut seen_short: Vec<(char, Span)> = Vec::new();
        let mut variadic_arg: Option<Span> = None;
        // A variadic that only takes what follows a `--`, which is the end of the line in
        // both senses: it holds the remaining words and the separator cannot come twice.
        let mut spent_separator: Option<Span> = None;
        let mut separator_eating_variadic: Option<Span> = None;
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
                // Nothing to check here: the flattened struct's own derive checked its
                // declarations, and a collision *across* the two is invisible from either
                // side — both expansions see a type name and no fields. That case is caught
                // where the whole tree is visible, by the duplicate-form check in
                // `Spec::to_kdl`.
                Kind::Flatten { .. } => {}
                Kind::Skip => {}
                Kind::Arg { double_dash } => {
                    // A variadic takes every remaining word, so anything after it can
                    // never be filled — with two exceptions, both of which are something
                    // that stops the variadic. An argument only fillable after a `--`,
                    // because the separator ends the collecting; and any argument at all
                    // when the variadic is *bounded*, because it hands over the words past
                    // its bound. mise relies on the first on `run`, `exec` and `git`.
                    // Only `required` stops a variadic. `automatic` stops *flags*, which is a
                    // different thing entirely, and `preserve` changes what a `--` means without
                    // ending anything — an argument declaring either is as unreachable behind an
                    // unbounded variadic as a plain one.
                    let stops_the_variadic = *double_dash == DoubleDash::Required;
                    // Unless the variadic in front eats the separator. `preserve` takes the `--`
                    // as one of its own values rather than letting it end anything, so the
                    // argument that was going to be unlocked by one never is — and the exemption
                    // that makes `run`'s layout legal would make this layout compile and be
                    // unfillable, which is the exact thing the check exists to prevent.
                    if let Some(first) = separator_eating_variadic {
                        return Err(dup(
                            field.span,
                            first,
                            "nothing can follow an unbounded variadic that keeps the `--` as a \
                             value: `double_dash = \"preserve\"` means the separator never ends \
                             anything, so not even an argument that waits for one can be filled \
                             — give the variadic a `var_max`",
                        ));
                    }
                    if let Some(first) = variadic_arg.filter(|_| !stops_the_variadic) {
                        return Err(dup(
                            field.span,
                            first,
                            "an argument after an unbounded variadic can never be filled, \
                             because the variadic takes every remaining word — give the \
                             variadic a `var_max`, or make this one fillable only after a \
                             `--`, either of which stops it",
                        ));
                    }
                    // The separator comes once, so an unbounded variadic behind it holds
                    // the rest of the command line and nothing can follow.
                    if let Some(first) = spent_separator {
                        return Err(dup(
                            field.span,
                            first,
                            "nothing can follow an unbounded variadic that takes the words \
                             after a `--`: it has both the rest of the command line and \
                             the only separator there is",
                        ));
                    }
                    if field.shape == Shape::Many && field.var_max.is_none() {
                        if stops_the_variadic {
                            spent_separator = Some(field.span);
                        } else {
                            variadic_arg = Some(field.span);
                            if *double_dash == DoubleDash::Preserve {
                                separator_eating_variadic = Some(field.span);
                            }
                        }
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

        // A delimiter splits one word into several values, so the field has to be able to
        // hold several. Anything else would drop everything after the first separator —
        // silently, and only at run time, which is the worst way to find out.
        for field in &self.fields {
            if field.delimiter.is_some() && field.shape != Shape::Many {
                return Err(syn::Error::new(
                    field.span,
                    "`delimiter` makes one word several values, so the field needs to be \
                     a `Vec`",
                ));
            }
        }

        // Groups: every member is an argument, every declared group has members, and a group
        // holds at least two of them — the same floor the spec enforces, checked here so
        // it fails where it is written rather than when the spec is emitted.
        let mut group_members: Vec<(&str, Vec<&Field>)> = Vec::new();
        for field in &self.fields {
            let Some(name) = field.group.as_deref() else {
                continue;
            };
            if !matches!(field.kind, Kind::Flag { .. } | Kind::Arg { .. }) {
                return Err(syn::Error::new(
                    field.span,
                    "`group` describes a relationship between arguments, so the field \
                     must be a flag or positional",
                ));
            }
            // `group("")` on the struct is refused as nameless; two fields saying
            // `group = ""` would otherwise form the same nameless group by the back
            // door, and it would be emitted and reported with nothing to call it.
            if name.is_empty() {
                return Err(syn::Error::new(
                    field.span,
                    "a group with no name answers to nothing; give it one, as \
                     `group = \"input\"`",
                ));
            }
            match group_members.iter_mut().find(|(n, _)| *n == name) {
                Some((_, members)) => members.push(field),
                None => group_members.push((name, vec![field])),
            }
        }
        for (name, members) in &group_members {
            if members.len() < 2 {
                return Err(syn::Error::new(
                    members[0].span,
                    format!(
                        "group `{name}` has one argument in it; a rule about a single \
                         argument belongs on that argument"
                    ),
                ));
            }
        }
        for (i, decl) in self.groups.iter().enumerate() {
            // Two declarations of one group would be read first-match-wins, so the second
            // one's properties would be silently dropped — a `required` written and not
            // enforced, which is worse than not being able to write it.
            if self.groups[..i].iter().any(|d| d.name == decl.name) {
                return Err(syn::Error::new(
                    decl.span,
                    format!(
                        "group `{}` is declared twice; one declaration carries all of \
                         its properties",
                        decl.name
                    ),
                ));
            }
            if !group_members.iter().any(|(n, _)| *n == decl.name) {
                return Err(syn::Error::new(
                    decl.span,
                    format!(
                        "group `{}` is declared and no field is in it; a field joins a \
                         group with `#[usage(group = \"{}\")]`",
                        decl.name, decl.name
                    ),
                ));
            }
        }

        // Every relationship names an entry that exists. Resolving these at compile time
        // is the advantage of declaring them in code: a spec written by hand can only
        // find a typo'd selector at parse time, or never, since a selector naming
        // nothing quietly holds no relationship at all.
        let has_flatten = self
            .fields
            .iter()
            .any(|field| matches!(field.kind, Kind::Flatten { .. }));
        for field in &self.fields {
            for (option, selectors) in [
                ("overrides", &field.overrides),
                ("conflicts", &field.conflicts),
                ("requires", &field.requires),
                ("required_if", &field.required_if),
                ("required_unless", &field.required_unless),
            ] {
                for selector in selectors {
                    let Some(target) = self.field_for_selector(selector) else {
                        // Relationship lookup composes through an opaque flattened partial.
                        // Post-binding rules ask it about presence and values; binding-time
                        // overrides ask it to displace the selected field as tokens arrive.
                        if has_flatten {
                            continue;
                        }
                        return Err(syn::Error::new(
                            field.span,
                            format!(
                                "`{option} = \"{selector}\"` names no argument on this \
                                 command; use `--long` or `-s` for a flag, or the bare \
                                 name for a positional"
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
            for condition in &field.requires_if {
                let selector = &condition.requires;
                let Some(target) = self.field_for_selector(selector) else {
                    if has_flatten {
                        continue;
                    }
                    return Err(syn::Error::new(
                        field.span,
                        format!(
                            "`requires_if(_, \"{selector}\")` names no flag on this command; \
                             write it as the spec does, `--long` or `-s`"
                        ),
                    ));
                };
                if target.ident == field.ident {
                    return Err(syn::Error::new(
                        field.span,
                        format!("`requires_if(_, \"{selector}\")` names its own field"),
                    ));
                }
            }
            for condition in &field.default_if {
                let selector = &condition.selector;
                let Some(target) = self.field_for_selector(selector) else {
                    if has_flatten {
                        continue;
                    }
                    return Err(syn::Error::new(
                        field.span,
                        format!(
                            "`default_if(\"{selector}\", _)` names no flag on this command; \
                             write it as the spec does, `--long` or `-s`"
                        ),
                    ));
                };
                if target.ident == field.ident {
                    return Err(syn::Error::new(
                        field.span,
                        format!("`default_if(\"{selector}\", _)` names its own field"),
                    ));
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
    /// A field marked `#[usage(skip)]`, if this is one.
    ///
    /// clap's `#[arg(skip)]`: the field is not an argument, and is filled from `Default`
    /// when the struct is built. Recognised before flags and arguments so a combination
    /// like `#[usage(skip, long)]` is refused as a contradiction rather than parsed as a
    /// flag that also happens to say skip — there is no such thing, and accepting it would
    /// put a field in the tables that the build then ignored.
    fn skip(
        field: &syn::Field,
        ident: &syn::Ident,
        span: proc_macro2::Span,
    ) -> syn::Result<Option<Self>> {
        let mut found = false;
        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                if ident_of(&meta.path().clone()) != "skip" {
                    continue;
                }
                if !matches!(meta, Meta::Path(_)) {
                    return Err(syn::Error::new_spanned(
                        meta.path(),
                        "`skip` takes no value: the field is filled from `Default`",
                    ));
                }
                found = true;
            }
        }
        if !found {
            return Ok(None);
        }

        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                let name = ident_of(&meta.path().clone());
                if name != "skip" {
                    return Err(syn::Error::new_spanned(
                        meta.path(),
                        format!(
                            "`skip` cannot be combined with `{name}`: a skipped field is \
                             not a flag or an argument, so nothing that describes one applies"
                        ),
                    ));
                }
            }
        }

        Ok(Some(Field {
            ident: ident.clone(),
            ty: field.ty.clone(),
            name: to_kebab(&ident.to_string()),
            value_optional: false,
            kind: Kind::Skip,
            effect: None,
            complete: None,
            complete_type: None,
            shape: Shape::Bool,
            value_ty: None,
            optional_collection: false,
            help: None,
            long_help: None,
            env: None,
            setting: None,
            default: Vec::new(),
            default_value_t: None,
            help_heading: None,
            value_name: None,
            required_collection: false,
            choices: Vec::new(),
            validate: None,
            validate_error: None,
            value_enum: false,
            var_min: None,
            var_max: None,
            overrides: Vec::new(),
            conflicts: Vec::new(),
            requires: Vec::new(),
            requires_if: Vec::new(),
            default_if: Vec::new(),
            delimiter: None,
            allow_hyphen_values: false,
            require_equals: false,
            default_missing: None,
            exclusive: false,
            group: None,
            required_if: Vec::new(),
            required_unless: Vec::new(),
            hide: false,
            repeatable: false,
            span,
        }))
    }

    /// A field marked `#[usage(flatten)]`, if this is one.
    ///
    /// Recognized before flags and arguments for the same reason a subcommand is: the field
    /// holds a whole command's worth of declarations, so none of what describes a single
    /// value applies to it. A doc comment on it describes nothing that reaches the spec,
    /// since the flattened struct's own fields carry their own help.
    fn flatten(
        field: &syn::Field,
        ident: &syn::Ident,
        span: proc_macro2::Span,
    ) -> syn::Result<Option<Self>> {
        let mut found = false;
        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                if ident_of(&meta.path().clone()) != "flatten" {
                    continue;
                }
                if !matches!(meta, Meta::Path(_)) {
                    return Err(syn::Error::new_spanned(
                        meta.path(),
                        "`flatten` takes no value: the struct it holds is the field's type",
                    ));
                }
                found = true;
            }
        }
        if !found {
            return Ok(None);
        }

        // Nothing else may be declared beside it. `#[usage(flatten, long)]` reads as though
        // the flattening were also a flag, and there is no such thing.
        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                let name = ident_of(&meta.path().clone());
                if name != "flatten" {
                    return Err(syn::Error::new_spanned(
                        meta.path(),
                        format!(
                            "`flatten` cannot be combined with `{name}`: the flattened \
                             struct's own fields declare what they are"
                        ),
                    ));
                }
            }
        }

        // `Option<T>` would mean "the whole group, or nothing" — which needs a rule for what
        // makes it present, and clap's answer (any of its fields given) is not obviously the
        // right one. Refused for now rather than guessed at; nothing in the fleet uses it.
        if type_name(&field.ty).starts_with("Option<") {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "`flatten` on an `Option` is not supported: it would have to decide when the \
                 group counts as given. Hold the struct directly",
            ));
        }

        Ok(Some(Field {
            ident: ident.clone(),
            ty: field.ty.clone(),
            name: to_kebab(&ident.to_string()),
            value_optional: false,
            kind: Kind::Flatten {
                ty: field.ty.clone(),
            },
            // Neither holds a flag of its own, so neither has an effect to declare: a
            // flattened group's flags carry their own, and a subcommand field is a command's
            // place rather than a command.
            effect: None,
            complete: None,
            complete_type: None,
            // A flattened field holds declarations, not a value, so none of what describes a
            // value applies — the same as a subcommand field.
            shape: Shape::Bool,
            value_ty: None,
            optional_collection: false,
            help: None,
            long_help: None,
            env: None,
            setting: None,
            default: Vec::new(),
            default_value_t: None,
            help_heading: None,
            value_name: None,
            required_collection: false,
            choices: Vec::new(),
            validate: None,
            validate_error: None,
            value_enum: false,
            var_min: None,
            var_max: None,
            overrides: Vec::new(),
            conflicts: Vec::new(),
            requires: Vec::new(),
            requires_if: Vec::new(),
            default_if: Vec::new(),
            delimiter: None,
            allow_hyphen_values: false,
            require_equals: false,
            default_missing: None,
            exclusive: false,
            group: None,
            required_if: Vec::new(),
            required_unless: Vec::new(),
            hide: false,
            repeatable: false,
            span,
        }))
    }

    /// A field marked `#[usage(subcommand)]`, if this is one.
    fn subcommand(
        field: &syn::Field,
        ident: &syn::Ident,
        span: proc_macro2::Span,
    ) -> syn::Result<Option<Self>> {
        let mut is_subcommand = false;
        let mut others: Vec<syn::Path> = Vec::new();
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
                } else {
                    others.push(meta.path().clone());
                }
            }
        }
        // Nothing else applies here, and this branch used to *ignore* whatever else was
        // written — so `#[usage(subcommand, effect = "write")]` compiled and said nothing.
        // Refused as a class rather than one option at a time: this field holds a set of
        // commands, and everything the attribute can otherwise say describes a value or a
        // flag, which a subcommand holder is neither.
        if is_subcommand {
            if let Some(other) = others.first() {
                let what = ident_of(&other.clone());
                return Err(syn::Error::new_spanned(
                    other,
                    format!(
                        "`{what}` says nothing about a `subcommand` field, which holds a set \
                         of commands rather than a value — declare it on the command it \
                         describes, where `#[derive(Args)]` is"
                    ),
                ));
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
            value_optional: false,
            kind: Kind::Subcommand { ty, optional },
            effect: None,
            complete: None,
            complete_type: None,
            // A subcommand field holds a command, not a value, so none of what
            // describes a value applies to it.
            shape: Shape::Bool,
            value_ty: None,
            optional_collection: false,
            help: None,
            long_help: None,
            env: None,
            setting: None,
            default: Vec::new(),
            default_value_t: None,
            help_heading: None,
            value_name: None,
            required_collection: false,
            choices: Vec::new(),
            validate: None,
            validate_error: None,
            value_enum: false,
            var_min: None,
            var_max: None,
            overrides: Vec::new(),
            conflicts: Vec::new(),
            requires: Vec::new(),
            requires_if: Vec::new(),
            default_if: Vec::new(),
            delimiter: None,
            allow_hyphen_values: false,
            require_equals: false,
            default_missing: None,
            exclusive: false,
            group: None,
            required_if: Vec::new(),
            required_unless: Vec::new(),
            hide: false,
            repeatable: false,
            span,
        }))
    }

    fn from_field(
        field: &syn::Field,
        rename_all: Option<CasingStyle>,
        rename_all_env: CasingStyle,
    ) -> syn::Result<Self> {
        let ident = field
            .ident
            .clone()
            .expect("named fields were checked by the caller");
        let span = field.span();
        // A skipped field is not a flag, an argument, or a subcommand: recognised
        // first so `#[usage(skip, long)]` is refused as a combination rather than
        // parsed as a flag that also happens to say skip.
        if let Some(skipped) = Self::skip(field, &ident, span)? {
            return Ok(skipped);
        }
        // A subcommand field is neither a flag nor an argument, and shares none of
        // their options, so it is recognized before any of them are read.
        if let Some(subcommand) = Self::subcommand(field, &ident, span)? {
            return Ok(subcommand);
        }
        if let Some(flattened) = Self::flatten(field, &ident, span)? {
            return Ok(flattened);
        }

        let rust_name = ident.unraw().to_string();
        let mut name = rename_all
            .map(|style| style.apply(&rust_name))
            .unwrap_or_else(|| to_kebab(&rust_name));
        let mut name_given = false;
        let mut longs: Vec<String> = Vec::new();
        let mut visible_long_aliases: Vec<String> = Vec::new();
        let mut hidden_longs: Vec<String> = Vec::new();
        let mut bare_longs = 0usize;
        let mut shorts: Vec<char> = Vec::new();
        let mut bare_shorts = 0usize;
        let mut negate = None;
        let mut global = false;
        let mut repeatable = false;
        let mut variadic = false;
        let mut count = false;
        let mut value_optional = false;
        let mut double_dash = DoubleDash::Optional;
        let mut env = None;
        let mut setting = None;
        let mut default: Vec<String> = Vec::new();
        let mut default_value_t = None;
        let mut help_heading = None;
        let mut effect = None;
        let mut value_name = None;
        let mut required_collection = false;
        let mut help_attr: Option<String> = None;
        let mut long_help_attr: Option<String> = None;
        let mut verbatim_doc_comment = false;
        let mut hide = false;
        let mut is_arg = false;
        let mut choices: Vec<String> = Vec::new();
        let mut validate: Option<String> = None;
        let mut validate_error: Option<String> = None;
        let mut complete: Option<syn::Path> = None;
        let mut complete_type: Option<String> = None;
        let mut value_enum = false;
        let mut var_min: Option<usize> = None;
        let mut var_max: Option<usize> = None;
        let mut overrides: Vec<String> = Vec::new();
        let mut conflicts: Vec<String> = Vec::new();
        let mut requires: Vec<String> = Vec::new();
        let mut requires_if: Vec<ConditionalRequirement> = Vec::new();
        let mut default_if: Vec<ConditionalDefault> = Vec::new();
        let mut group: Option<String> = None;
        let mut exclusive = false;
        let mut delimiter: Option<char> = None;
        let mut allow_hyphen_values = false;
        let mut require_equals = false;
        let mut default_missing = None;
        let mut required_if: Vec<String> = Vec::new();
        let mut required_unless: Vec<String> = Vec::new();

        for attr in attrs(&field.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    // Stripped, so a dashed spelling cannot leak into the spec name
                    // or into a long form derived from it.
                    "name" => {
                        name = strip_dashes(&string_value(&meta)?);
                        name_given = true;
                    }
                    // clap calls the parser identity `id`; usage's field name is the
                    // same stable identity and also supplies the positional placeholder.
                    "id" => {
                        name = strip_dashes(&string_value(&meta)?);
                        name_given = true;
                    }
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
                    // clap distinguishes advertised and hidden aliases. Visible aliases
                    // are losslessly additional long forms in usage.
                    "visible_alias" | "visible_aliases" => {
                        visible_long_aliases.extend(
                            selectors(&meta)?
                                .into_iter()
                                .map(|name| strip_dashes(&name)),
                        );
                    }
                    "alias" | "aliases" => {
                        hidden_longs.extend(
                            selectors(&meta)?
                                .into_iter()
                                .map(|name| strip_dashes(&name)),
                        );
                    }
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
                    // Help only: the parser refuses a bare `--bump` either way. What this
                    // changes is the brackets, which is the whole of what a spec's
                    // `arg "[BUMP]" required=#false` says.
                    "value_optional" => value_optional = flag_value(&meta)?,
                    "hide" => hide = flag_value(&meta)?,
                    "arg" => is_arg = flag_value(&meta)?,
                    "env" => {
                        env = Some(match &meta {
                            Meta::Path(_) => rename_all_env.apply(&ident.unraw().to_string()),
                            _ => string_value(&meta)?,
                        })
                    }
                    // The *setting* this flag sets, which is a different thing from the flag's name
                    // and from the environment variable: `--jobs`, `HK_JOBS` and `jobs` are three
                    // spellings of one value, and only the last is what a config file calls it.
                    "setting" => setting = Some(string_value(&meta)?),
                    // `choices("a", "b")` rather than one comma-joined string, so a
                    // value containing a comma is expressible.
                    // A path, not a string: it names a function in the user's crate, and a
                    // name that does not resolve should be a compile error where it is written
                    // rather than a completion that silently answers nothing.
                    "complete" => {
                        let value = &meta.require_name_value()?.value;
                        let Expr::Path(path) = value else {
                            return Err(syn::Error::new_spanned(
                                value,
                                "`complete` takes a function, as in `complete = my_completer`",
                            ));
                        };
                        complete = Some(path.path.clone());
                    }
                    "value_hint" => complete_type = Some(value_hint(&meta)?),
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
                    "validate" => validate = Some(string_value(&meta)?),
                    "validate_error" => validate_error = Some(string_value(&meta)?),
                    // Both spellings the spec has: one target as a value, several as a
                    // list. A flag selector never contains a comma, so unlike `choices`
                    // there is nothing to lose by accepting the shorter form.
                    "overrides" => overrides = selectors(&meta)?,
                    "conflicts" => conflicts = selectors(&meta)?,
                    "requires" => requires = selectors(&meta)?,
                    "requires_if" => requires_if.push(requirement_if(&meta)?),
                    "requires_ifs" => requires_if.extend(requirements_if(&meta)?),
                    "default_if" => default_if.push(default_if_attr(&meta)?),
                    "default_ifs" => default_if.extend(default_ifs_attr(&meta)?),
                    "group" => group = Some(string_value(&meta)?),
                    "exclusive" => exclusive = flag_value(&meta)?,
                    "allow_hyphen_values" => allow_hyphen_values = flag_value(&meta)?,
                    "require_equals" => require_equals = flag_value(&meta)?,
                    "default_missing" => default_missing = Some(string_value(&meta)?),
                    "delimiter" => {
                        let c = char_value(&meta)?;
                        if !c.is_ascii() {
                            return Err(syn::Error::new_spanned(
                                &path,
                                format!(
                                    "`delimiter = {c:?}` is more than one byte, and a \
                                     value is split by bytes; use an ASCII separator"
                                ),
                            ));
                        }
                        delimiter = Some(c);
                    }
                    "required_if" => required_if = selectors(&meta)?,
                    "required_unless" => required_unless = selectors(&meta)?,
                    "value_enum" => value_enum = flag_value(&meta)?,
                    "var_min" => var_min = Some(int_value(&meta)?),
                    "var_max" => var_max = Some(int_value(&meta)?),
                    "default" => default.push(string_value(&meta)?),
                    "default_value_t" => {
                        default_value_t = Some(match &meta {
                            Meta::Path(_) => {
                                quote::quote!(::std::default::Default::default())
                            }
                            Meta::NameValue(value) => {
                                quote::ToTokens::to_token_stream(&value.value)
                            }
                            Meta::List(_) => {
                                return Err(syn::Error::new_spanned(
                                    &meta,
                                    "`default_value_t` is a Rust expression or is written on its own for `Default::default()`",
                                ));
                            }
                        });
                    }
                    "help_heading" => help_heading = Some(string_value(&meta)?),
                    "effect" => effect = Some(effect_value(&meta)?),
                    "value_name" => value_name = Some(string_value(&meta)?),
                    "num_args" => {
                        return Err(syn::Error::new_spanned(
                            &meta,
                            "clap's `num_args` maps to the Rust field shape plus \
                             `var_min`/`var_max`; use `Option<T>`, `Vec<T>`, and those \
                             bounds to declare the same arity",
                        ));
                    }
                    "value_parser" => {
                        return Err(syn::Error::new_spanned(
                            &meta,
                            "clap's `value_parser` is Rust-only metadata; usage converts \
                             through the field type's `FromStr`, with `value_enum`, \
                             `choices`, or portable `validate` for additional constraints",
                        ));
                    }
                    // Help text a doc comment cannot carry. A comment's first paragraph is
                    // read the way Rust reads one — line breaks inside it are spaces — so
                    // help whose breaks are meant literally has to be given directly.
                    "help" => help_attr = Some(string_value(&meta)?),
                    "long_help" => long_help_attr = Some(string_value(&meta)?),
                    "verbatim_doc_comment" => verbatim_doc_comment = flag_value(&meta)?,
                    "required" => required_collection = flag_value(&meta)?,
                    "double_dash" => {
                        let mode = string_value(&meta)?;
                        match mode.as_str() {
                            "optional" => double_dash = DoubleDash::Optional,
                            "required" => double_dash = DoubleDash::Required,
                            "preserve" => double_dash = DoubleDash::Preserve,
                            "automatic" => double_dash = DoubleDash::Automatic,
                            other => {
                                return Err(syn::Error::new_spanned(
                                    path,
                                    format!(
                                        "`double_dash = \"{other}\"` is not a mode; \
                                         the spec has \"optional\", \"required\", \
                                         \"preserve\" and \"automatic\""
                                    ),
                                ));
                            }
                        }
                    }
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}`; a field takes `name`, `id`, `long`, \
                                 `short`, `negate`, `global`, `var`, `variadic`, \
                                 `count`, `hide`, `arg`, `env`, `default`, `default_value_t`, `choices`, `validate`, \
                                 `validate_error`, \
                                 `var_min`, `var_max`, `value_enum`, `value_hint`, `overrides`, \
                                 `conflicts`, `requires`, `group`, `exclusive`, \
                                 `delimiter`, `allow_hyphen_values`, `require_equals`, \
                                 `default_missing`, `default_if`, \
                                 `required_if`, \
                                 `required_unless`, `help_heading`, `value_name`, \
                                 `verbatim_doc_comment`, \
                                 `visible_alias`, `visible_aliases`, `required`, \
                                 `double_dash`, and `skip`"
                            ),
                        ));
                    }
                }
            }
        }

        let (help, long_help) = doc_comment(&field.attrs, verbatim_doc_comment)?;

        // A bare `long` or `short` written before `name` would have captured the
        // field name rather than the renamed one, so resolve both once everything
        // has been read. Counted rather than rewritten, so a field carrying both a
        // bare and an explicit form keeps each.
        for _ in 0..bare_longs {
            longs.insert(0, name.clone());
        }
        longs.extend(visible_long_aliases);
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

        // A short form is matched as a single byte, so a multi-byte character could never be
        // recognized. Better to say so than to truncate it.
        //
        // This also keeps `usage_argv::os_string_from_bytes` sound: a cluster is walked one
        // byte at a time, and the remainder after a value-taking short becomes that value, so
        // a non-ASCII short would let a value begin inside a character. `Flag::shorts`
        // documents the requirement; this is where it is enforced for derived tables.
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

        let ValueKind {
            shape,
            ty: value_ty,
            optional_collection,
        } = ValueKind::from_type(&field.ty, count, span)?;
        // The spec records a default and the generated code applies it; anything it
        // cannot apply would be documented and then ignored.
        //
        // Several is a collection's privilege. Every other shape has one place to put a value,
        // so a second `default` is a contradiction rather than a list — and silently keeping
        // the last would be a declaration the author cannot see being ignored.
        if default.len() > 1 && shape != Shape::Many {
            return Err(syn::Error::new(
                span,
                "this field holds one value, so it takes one `default`; several is for a \
                 `Vec`, which starts out holding all of them",
            ));
        }
        if default_value_t.is_some() {
            if default.len() != 1 {
                return Err(syn::Error::new(
                    span,
                    "`default_value_t` needs exactly one `default = \"...\"` beside it: the Rust expression supplies the runtime value and the literal is emitted into the portable spec",
                ));
            }
            if matches!(shape, Shape::Bool | Shape::Count | Shape::Many) {
                return Err(syn::Error::new(
                    span,
                    "`default_value_t` is for one value-taking field; switches, counts, and collections declare their portable defaults directly",
                ));
            }
        }
        for value in &default {
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
                _ => {}
            }
        }
        if value_enum && !choices.is_empty() {
            return Err(syn::Error::new(
                span,
                "`value_enum` takes the words from the type and `choices` lists them here, \
                 so a field says one or the other — two lists is one too many to keep in \
                 step",
            ));
        }
        if value_enum && matches!(shape, Shape::Bool | Shape::Count) {
            return Err(syn::Error::new(
                span,
                "`value_enum` describes what a value may be, and this field takes no value",
            ));
        }
        if value_optional && matches!(shape, Shape::Bool | Shape::Count) {
            return Err(syn::Error::new(
                span,
                "`value_optional` describes a value that may be left off, and this field \
                 takes no value",
            ));
        }
        if !choices.is_empty() && matches!(shape, Shape::Bool | Shape::Count) {
            return Err(syn::Error::new(
                span,
                "a `bool` or counting field has no value to check against `choices`",
            ));
        }
        if validate.is_some() && matches!(shape, Shape::Bool | Shape::Count) {
            return Err(syn::Error::new(
                span,
                "a `bool` or counting field has no value to validate",
            ));
        }
        if validate_error.is_some() && validate.is_none() {
            return Err(syn::Error::new(
                span,
                "`validate_error` needs a `validate` expression to report for",
            ));
        }
        if complete_type.is_some() && matches!(shape, Shape::Bool | Shape::Count) {
            return Err(syn::Error::new(
                span,
                "`value_hint` describes a value to complete, and this field takes no value",
            ));
        }
        if complete_type.is_some() && complete.is_some() {
            return Err(syn::Error::new(
                span,
                "`value_hint` and `complete` both answer completion for this value; use one",
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
        if !choices.is_empty() {
            // Each of them, not the first: a collection's second default is as unusable as its
            // first if the choices do not allow it.
            if let Some(default) = default.iter().find(|d| !choices.contains(d)) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "the default `{default}` is not one of this field's choices, so \
                         it could never be valid"
                    ),
                ));
            }
            if let Some(missing) = default_missing
                .as_ref()
                .filter(|missing| !choices.contains(missing))
            {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "the default_missing `{missing}` is not one of this field's \
                         choices, so it could never be valid"
                    ),
                ));
            }
            if let Some(value) = default_if
                .iter()
                .map(|condition| &condition.value)
                .find(|value| !choices.contains(value))
            {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "the default_if value `{value}` is not one of this field's \
                         choices, so it could never be valid"
                    ),
                ));
            }
        }

        if longs.is_empty() && shorts.is_empty() && !hidden_longs.is_empty() {
            return Err(syn::Error::new(
                span,
                "`alias`/`aliases` add hidden spellings to a flag, so the field also needs \
                 `long` or `short` to declare the flag",
            ));
        }
        let is_flag = !longs.is_empty() || !shorts.is_empty();
        // Only a flag's value has a say in this. A positional's brackets come from its type
        // already — `Option<T>` renders `[NAME]` and `T` renders `<NAME>` — so the attribute
        // would be read by nothing, and a declaration nothing reads is worse than an error: the
        // page would keep saying the opposite of what was asked for.
        //
        // Tested against `is_flag` and not `is_arg`: `is_arg` is only true where `arg` was
        // written, and a field with no `long`, `short` or `arg` is a positional as well. That
        // gap let `#[usage(value_optional)] out: Option<String>` compile and be dropped.
        if value_optional && !is_flag {
            return Err(syn::Error::new(
                span,
                "`value_optional` is for a flag's value; a positional says this with its \
                 type, where `Option<T>` is already `[NAME]`",
            ));
        }
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
        // Most relationship families still live on flags. Pairwise conflicts also live
        // on positionals, matching clap's argument-id model and the spec's bare selector.
        for (option, selectors) in [
            ("overrides", &overrides),
            ("conflicts", &conflicts),
            ("requires", &requires),
            ("required_if", &required_if),
            ("required_unless", &required_unless),
        ] {
            if !selectors.is_empty() && !is_flag && option != "conflicts" {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`{option}` describes a relationship between flags, so the field \
                         needs a `long` or a `short`"
                    ),
                ));
            }
        }
        if !requires_if.is_empty() && !is_flag {
            return Err(syn::Error::new(
                span,
                "`requires_if` describes a relationship between flags, so the field \
                 needs a `long` or a `short`",
            ));
        }
        if !default_if.is_empty() && !is_flag {
            return Err(syn::Error::new(
                span,
                "`default_if` describes a relationship between flags, so the field \
                 needs a `long` or a `short`",
            ));
        }
        if !default_if.is_empty() && shape == Shape::Count {
            return Err(syn::Error::new(
                span,
                "`default_if` binds a value, and a `count` field holds how many times \
                 the flag was given rather than a value",
            ));
        }
        // Declared text wins over the comment, which is the point of declaring it. A comment's
        // first paragraph is read the way Rust reads one — line breaks inside it become spaces —
        // so help whose breaks are deliberate has to be given directly.
        let (help, long_help) = (help_attr.or(help), long_help_attr.or(long_help));

        // A flag is named after the form it answers to, not after the Rust field holding it.
        // usage-lib derives the name the same way, so the two agree about what a flag is
        // called — and the field name is often not a legal one: `type_` gave a flag called
        // `type-`, which help printed as `type-: -t --type` and errors reported as `type-`.
        //
        // Only where the field says nothing: an explicit `name` still wins, and a flag with no
        // long form keeps its short as the name, as usage-lib does.
        if is_flag && !name_given {
            // Only where the name is about to become something that says nothing: a flag with
            // no long form is named after its short one, and `-j <j>` is no use as a
            // placeholder. Where a long form exists it *is* the descriptive name, and keeping
            // the field ident instead would render `--type <type->` for a field called `type_`.
            if longs.is_empty()
                && value_name.is_none()
                && shape != Shape::Bool
                && shape != Shape::Count
            {
                // Shouted here too: this runs before the default below, so leaving it
                // unshouted meant `-j <jobs>` beside `--jobs <JOBS>` — one CLI printing a
                // placeholder two ways. clap prints `-j <JOBS>`.
                value_name = Some(shout(&name));
            }
            if let Some(long) = longs.first() {
                name = long.clone();
            } else if let Some(short) = shorts.first() {
                name = short.to_string();
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

        // `required` is for the one case the type cannot express. Anywhere else it either
        // repeats what the type says or contradicts it, and both are worth refusing: a
        // declaration that changes nothing is a declaration someone will trust.
        if required_collection && shape != Shape::Many {
            return Err(syn::Error::new(
                span,
                match shape {
                    Shape::Optional => {
                        "`required` contradicts `Option`, which is how a \
                                        field says a value may be left out — drop one or \
                                        the other"
                    }
                    Shape::Required => {
                        "a bare type is already required: `required` is only \
                                        for a collecting field, where the type cannot say it"
                    }
                    _ => {
                        "`required` is only for a collecting field, which is the one shape \
                          whose type cannot say whether a value is needed"
                    }
                },
            ));
        }

        // A `Vec` flag collects, so it is repeatable whether or not it says so —
        // unless it is `variadic`, which is the other way of collecting. Emitting
        // both would claim a flag is repeatable *and* that its argument is variadic,
        // which the grammar treats as two different things.
        //
        // A `count` flag is repeatable by definition: `-vvv` is three occurrences, and
        // counting them is the whole point. Left uninferred, the emitted spec said `count`
        // without `var` where mise's says both, and help rendered `-v --verbose` for a flag
        // that can be given again.
        let repeatable =
            repeatable || (is_flag && !variadic && (shape == Shape::Many || shape == Shape::Count));

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
            // Hidden aliases still belong in the parse table, after advertised forms so
            // the first long remains the canonical help spelling.
            longs.extend(hidden_longs.iter().cloned());
            Kind::Flag {
                longs,
                hidden_longs,
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
            Kind::Arg { double_dash }
        };

        if complete_type.as_deref() == Some("command_args")
            && (!matches!(kind, Kind::Arg { .. })
                || shape != Shape::Many
                || double_dash != DoubleDash::Automatic)
        {
            return Err(syn::Error::new(
                span,
                "`ValueHint::CommandWithArguments` describes a forwarded argv vector; use \
                 it on a positional `Vec` with `double_dash = \"automatic\"`",
            ));
        }

        // `required` on a collection is the only way a `Vec` can say it, and it means *always*.
        // Two declarations contradict that, and both compiled:
        //
        // A sibling `required_if` or `required_unless` says "only sometimes", and the check for
        // plain required-ness runs first and unconditionally — so the condition was accepted,
        // emitted into the spec, and never consulted. For a scalar the two cannot meet, because
        // `required_unless` needs somewhere to put "absent" and so only goes on an `Option`; a
        // collection has no such type to keep them apart, so this does.
        //
        // And an `Option<Vec<_>>` is shaped like any other collection, so `required` was
        // accepted there too — after which the field can never be `None` and the `Option` means
        // nothing at all.
        // `required` is for the one shape whose type cannot say it. Anywhere else it repeats
        // what the type says or contradicts it, and a declaration that changes nothing is one
        // someone will eventually trust. (This guard was lost while two fixes for the same
        // review finding met in the middle; the test for it is what noticed.)
        if required_collection && shape != Shape::Many {
            return Err(syn::Error::new(
                span,
                match shape {
                    Shape::Optional => {
                        "`required` contradicts `Option`, which is how a field \
                                        says a value may be left out — drop one or the other"
                    }
                    Shape::Required => {
                        "a bare type is already required: `required` is only for \
                                        a collecting field, where the type cannot say it"
                    }
                    _ => {
                        "`required` is only for a collecting field, which is the one shape \
                          whose type cannot say whether a value is needed"
                    }
                },
            ));
        }
        if required_collection {
            let contradiction = if optional_collection {
                Some("an `Option<Vec<_>>` says the whole collection may be absent")
            } else if !required_if.is_empty() {
                Some("`required_if` says it is required only sometimes")
            } else if !required_unless.is_empty() {
                Some("`required_unless` says it is required only sometimes")
            } else {
                None
            };
            if let Some(why) = contradiction {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`required` on a collection means one or more values, always, but {why}"
                    ),
                ));
            }
        }

        // `effect` says what *supplying* something does to the world, so on a field it belongs
        // to a flag. A positional is the thing being acted on rather than a choice to act, and
        // `arg_meta` has nowhere to put one — so a declaration here was stored and then
        // dropped, which reads as though it had done something.
        if effect.is_some() && !matches!(kind, Kind::Flag { .. }) {
            return Err(syn::Error::new(
                span,
                "`effect` describes what supplying a flag does to the world, and a positional \
                 argument is what is acted on rather than a choice to act — put it on the \
                 command, where `#[derive(Args)]` is, or on the flag that changes the answer",
            ));
        }

        // `exclusive` is represented by flag metadata and enforced for a flag occurrence.
        // Accepting it on a positional would make the derive enforce a rule that its emitted
        // spec and documentation silently omit.
        if exclusive && !matches!(kind, Kind::Flag { .. }) {
            return Err(syn::Error::new(
                span,
                "`exclusive` describes a flag that has to be given on its own; a positional \
                 argument cannot carry it — add `long` or `short` to make this field a flag",
            ));
        }

        // clap's `allow_hyphen_values`: a flag's detached value may look like a flag.
        // A positional that needs the same thing already has `double_dash = "automatic"`,
        // which is how trailing argv is spelled. A flag that takes no value has nothing
        // to take, matching the spec's refusal.
        if allow_hyphen_values {
            if !matches!(kind, Kind::Flag { .. }) {
                return Err(syn::Error::new(
                    span,
                    "`allow_hyphen_values` is for a flag's detached value; a positional \
                     that should take flag-like words after it declares \
                     `double_dash = \"automatic\"` instead",
                ));
            }
            if matches!(shape, Shape::Bool | Shape::Count) {
                return Err(syn::Error::new(
                    span,
                    "`allow_hyphen_values` takes the next token as a value, and this flag \
                     takes none",
                ));
            }
        }

        if require_equals {
            if !matches!(kind, Kind::Flag { .. }) {
                return Err(syn::Error::new(
                    span,
                    "`require_equals` is for a flag's value; a positional has no `=` form",
                ));
            }
            if matches!(shape, Shape::Bool | Shape::Count) {
                return Err(syn::Error::new(
                    span,
                    "`require_equals` requires `--flag=value`, and this flag takes none",
                ));
            }
        }

        if default_missing.is_some() {
            if !matches!(kind, Kind::Flag { .. }) {
                return Err(syn::Error::new(
                    span,
                    "`default_missing` is for a flag's value; a positional is filled \
                     or it is not",
                ));
            }
            if matches!(shape, Shape::Bool | Shape::Count) {
                return Err(syn::Error::new(
                    span,
                    "`default_missing` is the value used when this flag is given \
                     without one, and this flag takes none",
                ));
            }
            // Help should show the value as optional: `--color` is a complete invocation.
            value_optional = true;
        }

        // `value_name` names the placeholder a *flag's value* gets in help — `--out <FILE>`.
        // A positional argument is named by `name`, and a `bool` or `count` flag has no value
        // to put a placeholder in, so `arg_meta` never emits it and a valueless flag has nowhere
        // to show it. Accepting it there compiled and dropped it: the declaration read as though
        // it had done something.
        if value_name.is_some() {
            let unusable = match &kind {
                Kind::Arg { .. } => Some(
                    "a positional argument is named by `name`, which is the placeholder help \
                     shows",
                ),
                Kind::Flag { .. } if matches!(shape, Shape::Bool | Shape::Count) => {
                    Some("this flag takes no value, so there is no placeholder to name")
                }
                _ => None,
            };
            if let Some(why) = unusable {
                return Err(syn::Error::new(
                    span,
                    format!("`value_name` names the placeholder a flag's value gets: {why}"),
                ));
            }
        }

        // A positional is named by the same rule, and its name *is* its placeholder: clap
        // prints `<TAG> [PREV_TAG]` for `tag: String, prev_tag: Option<String>`, so a spec
        // saying `<tag> [prev-tag]` is a CLI whose help reads differently for no reason.
        // Only when the field did not name itself — `name = "…"` means exactly this.
        if matches!(kind, Kind::Arg { .. }) && !name_given {
            name = shout(&name);
        }

        // Undeclared, a flag's value is named after the flag, shouted — `--max-tokens
        // <MAX_TOKENS>`. clap's default, and the reason to match it is the same as everywhere
        // else here: an adopter's users read the placeholder in `--help`, and `<max-tokens>` is
        // a visible change for a CLI that changed nothing.
        //
        // Set here rather than left to the renderers, so the metadata says what help prints —
        // two fallbacks would be two answers, and the spec is what docs and completions read.
        if value_name.is_none()
            && matches!(kind, Kind::Flag { .. })
            && !matches!(shape, Shape::Bool | Shape::Count)
        {
            value_name = Some(shout(&name));
        }

        Ok(Field {
            ident,
            ty: field.ty.clone(),
            name,
            value_optional,
            kind,
            shape,
            value_ty,
            optional_collection,
            help,
            long_help,
            env,
            setting,
            default,
            default_value_t,
            help_heading,
            effect,
            value_name,
            required_collection,
            choices,
            validate,
            validate_error,
            complete,
            complete_type,
            value_enum,
            var_min,
            var_max,
            overrides,
            conflicts,
            requires,
            requires_if,
            default_if,
            delimiter,
            allow_hyphen_values,
            require_equals,
            default_missing,
            exclusive,
            group,
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

/// What a field's type says about the values that reach it.
pub struct ValueKind {
    pub shape: Shape,
    /// What one value converts into: the `T` in `T`, `Option<T>` or `Vec<T>`.
    ///
    /// Absent for a switch and for a count, which are decided by how many times the flag
    /// was given rather than by anything a word says.
    pub ty: Option<Type>,
    /// Whether a collection was written as `Option<Vec<T>>`.
    ///
    /// The difference from `Vec<T>` is "never given" against "given nothing", which is a
    /// distinction mise's root draws three times. Values collect the same way; only the
    /// field it is finally put into differs.
    pub optional_collection: bool,
}

impl ValueKind {
    fn from_type(ty: &Type, count: bool, span: Span) -> syn::Result<Self> {
        let plain = |shape| ValueKind {
            shape,
            ty: None,
            optional_collection: false,
        };
        let name = type_name(ty);
        if count {
            return match name.as_str() {
                "u8" | "u16" | "u32" | "u64" | "usize" => Ok(plain(Shape::Count)),
                other => Err(syn::Error::new(
                    span,
                    format!(
                        "a `count` field holds how many times the flag was given, so it \
                         has to be an unsigned integer, not `{other}`"
                    ),
                )),
            };
        }
        if name == "bool" {
            return Ok(plain(Shape::Bool));
        }
        // Peeled in this order because `Option<Vec<T>>` is both, and reading it as an
        // optional whose value is a `Vec` would ask `Vec<T>` to parse from one word.
        if let Some(inner) = peel(ty, "Option").as_ref().and_then(|o| peel(o, "Vec")) {
            return Ok(ValueKind {
                shape: Shape::Many,
                ty: Some(inner),
                optional_collection: true,
            });
        }
        for (wrapper, shape) in [("Option", Shape::Optional), ("Vec", Shape::Many)] {
            if let Some(inner) = peel(ty, wrapper) {
                return Ok(ValueKind {
                    shape,
                    ty: Some(inner),
                    optional_collection: false,
                });
            }
        }
        // Anything else is one value, converted with `FromStr` — which is where a type
        // that cannot hold a command-line word finally reports itself, naming the user's
        // type rather than ours.
        Ok(ValueKind {
            shape: Shape::Required,
            ty: Some(ty.clone()),
            optional_collection: false,
        })
    }
}

/// A type as a comparable string, with paths reduced to their last segment so
/// `std::option::Option<String>` reads as `Option<String>`.
/// The `T` in a written `Box<T>`, path and all.
///
/// Only the last segment is inspected, so `std::boxed::Box<T>` and `Box<T>` are both read,
/// and a single type argument is required: `Box<T, A>` names an allocator this cannot
/// reason about, and is left as the type it is — which then fails to implement the trait,
/// with an error naming the type the user wrote.
fn unbox(ty: &Type) -> Option<Type> {
    peel(ty, "Box")
}

/// The `T` in a written `Wrapper<T>`, path and all.
///
/// Only the last path segment is inspected, so `std::boxed::Box<T>` reads as `Box<T>` and
/// `std::option::Option<T>` as `Option<T>`. One type argument is required: `Box<T, A>`
/// names an allocator this cannot reason about, and is left as the type the user wrote so
/// the error names theirs rather than ours.
fn peel(ty: &Type, wrapper: &str) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if last.ident != wrapper {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    match args.args.iter().collect::<Vec<_>>().as_slice() {
        [syn::GenericArgument::Type(inner)] => Some(inner.clone()),
        _ => None,
    }
}

/// A type as written, path and all, with the spaces `quote` leaves out.
///
/// Distinct from [`type_name`], which keeps only the last segment: telling
/// `std::string::String` from somebody's own `my::String` needs the whole path, and
/// mistaking the two hands a `String` to a field that cannot hold one.
pub fn rendered_path(ty: &Type) -> String {
    quote::ToTokens::to_token_stream(ty)
        .to_string()
        .replace(' ', "")
}

pub fn type_name(ty: &Type) -> String {
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

/// Native `#[usage(...)]` plus clap-compatible `#[command(...)]` and
/// `#[arg(...)]` attributes. The latter appears on fields (and inline variants),
/// while the iterator is shared by all three model readers.
fn attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|a| {
        a.path().is_ident("usage") || a.path().is_ident("command") || a.path().is_ident("arg")
    })
}

/// Value metadata accepts clap's `#[value(...)]` spelling so an enum can keep its
/// existing annotations while replacing the derive. `#[usage(...)]` remains accepted
/// for code that does not need source compatibility.
fn value_attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("usage") || a.path().is_ident("value"))
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

/// A Rust expression whose result must be usable as `&'static str` in the
/// generated metadata table. Type and const checking belong to rustc; retaining
/// the tokens here is what lets constants and `env!` remain the source of truth.
fn metadata_expr(meta: &Meta) -> syn::Result<proc_macro2::TokenStream> {
    let value = &meta.require_name_value()?.value;
    Ok(quote::ToTokens::to_token_stream(value))
}

/// One flag selector as a value, or several as a list.
///
/// Selectors are written the way the spec writes them — `"--stdin"`, `"-s"` — rather
/// than as field names, so a declaration reads the same in Rust as it does in KDL.
/// Which flag each one names is resolved in [`Cli::check`], where every field is in
/// view.
/// `group("input", required, multiple)` — a name, then any of the two properties.
///
/// Hand-parsed rather than reusing [`selectors`], because the list is mixed: a string
/// literal for the name and bare idents for the properties. Spelling the properties as
/// idents rather than as `required = true` matches how `long`, `global` and `count` are
/// already written on a field — a property that is only ever on or off is said by naming
/// it.
fn group_decl(meta: &Meta) -> syn::Result<GroupDecl> {
    let span = meta.path().span();
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta.path(),
            "a group is declared as `group(\"name\")`, with `required` and `multiple` \
             after the name if it needs them",
        ));
    };
    let mut name: Option<String> = None;
    let mut decl = GroupDecl {
        name: String::new(),
        required: false,
        multiple: false,
        span,
    };
    list.parse_args_with(|input: syn::parse::ParseStream| {
        while !input.is_empty() {
            if input.peek(syn::LitStr) {
                let lit: syn::LitStr = input.parse()?;
                if name.is_some() {
                    return Err(syn::Error::new_spanned(
                        &lit,
                        "a group takes one name; its members are declared on the fields, \
                         with `#[usage(group = \"…\")]`",
                    ));
                }
                name = Some(lit.value());
            } else {
                let ident: syn::Ident = input.parse()?;
                match ident.to_string().as_str() {
                    "required" => decl.required = true,
                    "multiple" => decl.multiple = true,
                    other => {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            format!(
                                "unknown group property `{other}`; a group takes \
                                 `required` and `multiple`"
                            ),
                        ));
                    }
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<syn::Token![,]>()?;
        }
        Ok(())
    })?;
    let Some(name) = name else {
        return Err(syn::Error::new(
            span,
            "a group needs a name, as in `group(\"input\", required)`",
        ));
    };
    if name.is_empty() {
        return Err(syn::Error::new(
            span,
            "a group with no name answers to nothing",
        ));
    }
    decl.name = name;
    Ok(decl)
}

fn selectors(meta: &Meta) -> syn::Result<Vec<String>> {
    let found: Vec<String> = match meta {
        Meta::List(list) => {
            if let Ok(array) = syn::parse2::<syn::ExprArray>(list.tokens.clone()) {
                string_array(&array)?
            } else {
                list.parse_args_with(
                    syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
                )?
                .into_iter()
                .map(|lit| lit.value())
                .collect()
            }
        }
        Meta::NameValue(value) => match &value.value {
            syn::Expr::Array(array) => string_array(array)?,
            _ => vec![string_value(meta)?],
        },
        Meta::Path(_) => vec![string_value(meta)?],
    };
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

fn string_array(array: &syn::ExprArray) -> syn::Result<Vec<String>> {
    array
        .elems
        .iter()
        .map(|expr| match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => Ok(value.value()),
            _ => Err(syn::Error::new_spanned(expr, "expected a string literal")),
        })
        .collect()
}

fn requirement_if(meta: &Meta) -> syn::Result<ConditionalRequirement> {
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "`requires_if` takes a value and a flag, as in \
             `requires_if(\"json\", \"--format\")`",
        ));
    };
    let args = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
    )?;
    if args.len() != 2 {
        return Err(syn::Error::new_spanned(
            meta,
            "`requires_if` takes exactly a value and a flag, as in \
             `requires_if(\"json\", \"--format\")`",
        ));
    }
    let mut args = args.into_iter();
    Ok(ConditionalRequirement {
        value: args.next().expect("length checked").value(),
        requires: args.next().expect("length checked").value(),
    })
}

fn requirements_if(meta: &Meta) -> syn::Result<Vec<ConditionalRequirement>> {
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "`requires_ifs` takes `(value, flag)` pairs",
        ));
    };
    let pairs = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::ExprTuple, syn::Token![,]>::parse_terminated,
    )?;
    if pairs.is_empty() {
        return Err(syn::Error::new_spanned(
            meta,
            "`requires_ifs` needs at least one `(value, flag)` pair",
        ));
    }
    pairs
        .into_iter()
        .map(|pair| {
            if pair.elems.len() != 2 {
                return Err(syn::Error::new_spanned(
                    pair,
                    "each `requires_ifs` entry is `(value, flag)`",
                ));
            }
            let mut elems = pair.elems.into_iter();
            let value = string_expr(elems.next().expect("length checked"))?;
            let requires = string_expr(elems.next().expect("length checked"))?;
            Ok(ConditionalRequirement { value, requires })
        })
        .collect()
}

fn default_if_attr(meta: &Meta) -> syn::Result<ConditionalDefault> {
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "`default_if` takes a flag and a value, as in `default_if(\"--json\", \"true\")`, \
             or a flag, a value, and a default, as in `default_if(\"--output\", \"json\", \"pretty\")`",
        ));
    };
    let args: Vec<_> = list
        .parse_args_with(
            syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
        )?
        .into_iter()
        .collect();
    match args.len() {
        2 => Ok(ConditionalDefault {
            selector: args[0].value(),
            when: None,
            value: args[1].value(),
        }),
        3 => Ok(ConditionalDefault {
            selector: args[0].value(),
            when: Some(args[1].value()),
            value: args[2].value(),
        }),
        _ => Err(syn::Error::new_spanned(
            meta,
            "`default_if` takes two arguments (`--json`, `true`) or three \
             (`--output`, `json`, `pretty`)",
        )),
    }
}

fn default_ifs_attr(meta: &Meta) -> syn::Result<Vec<ConditionalDefault>> {
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "`default_ifs` takes tuples of two or three strings",
        ));
    };
    let pairs = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::ExprTuple, syn::Token![,]>::parse_terminated,
    )?;
    if pairs.is_empty() {
        return Err(syn::Error::new_spanned(
            meta,
            "`default_ifs` needs at least one tuple",
        ));
    }
    pairs
        .into_iter()
        .map(|pair| {
            let n = pair.elems.len();
            if n != 2 && n != 3 {
                return Err(syn::Error::new_spanned(
                    pair,
                    "each `default_ifs` entry is `(flag, value)` or `(flag, when, value)`",
                ));
            }
            let mut elems = pair.elems.into_iter();
            let selector = string_expr(elems.next().expect("length checked"))?;
            if n == 2 {
                let value = string_expr(elems.next().expect("length checked"))?;
                Ok(ConditionalDefault {
                    selector,
                    when: None,
                    value,
                })
            } else {
                let when = string_expr(elems.next().expect("length checked"))?;
                let value = string_expr(elems.next().expect("length checked"))?;
                Ok(ConditionalDefault {
                    selector,
                    when: Some(when),
                    value,
                })
            }
        })
        .collect()
}

fn string_expr(expr: Expr) -> syn::Result<String> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Ok(value.value()),
        other => Err(syn::Error::new_spanned(
            other,
            "a conditional requirement's value and flag must be string literals",
        )),
    }
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
/// The first paragraph is the short form; the whole comment is the long form and is only
/// reported when it says more than the short one. Prose is flowed by default, while
/// `verbatim` keeps line breaks and whitespace for tables, examples, and ASCII art.
fn doc_comment(
    attrs: &[Attribute],
    verbatim: bool,
) -> syn::Result<(Option<String>, Option<String>)> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        if let Meta::NameValue(nv) = &attr.meta {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &nv.value
            {
                // Only the one space `///` conventionally adds, and trailing space. Trimming
                // each line outright flattened every indented example in a CLI's help — and
                // mise's help is full of them, since an indented block is how a spec shows a
                // command to type.
                let raw = s.value();
                if verbatim {
                    let mut raw_lines = raw.split('\n');
                    if let Some(first) = raw_lines.next() {
                        lines.push(first.strip_prefix(' ').unwrap_or(first).to_string());
                    }
                    lines.extend(raw_lines.map(str::to_string));
                } else {
                    // Preserve the pre-verbatim behaviour for an explicitly written,
                    // multiline `#[doc = "..."]`: only `///` contributes one leading
                    // space per attribute. A newline inside one attribute does not.
                    lines.push(raw.strip_prefix(' ').unwrap_or(&raw).trim_end().to_string());
                }
            }
        }
    }
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return Ok((None, None));
    }

    if verbatim {
        let first_blank = lines.iter().position(|line| line.trim().is_empty());
        let short_lines = first_blank.map_or(lines.as_slice(), |i| &lines[..i]);
        let short = short_lines.join("\n");
        let long = first_blank.map(|_| lines.join("\n"));
        return Ok(((!short.is_empty()).then_some(short), long));
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
/// A flag's name as its value placeholder: `max-tokens` becomes `MAX_TOKENS`.
///
/// Taken from the *form* rather than the field, so `#[usage(long = "type")] type_` renders
/// `--type <TYPE>` and not `<TYPE_>`, and shouted with underscores because that is what clap
/// prints. All three measured from clap 4 rather than assumed:
///
/// ```text
///       --type <TYPE>
///       --max-tokens <MAX_TOKENS>
///   -c, --config <CONFIG>
/// ```
///
/// One flag written two ways on purpose: kebab to type, shouted snake to read.
fn shout(form: &str) -> String {
    form.to_uppercase().replace('-', "_")
}

pub(crate) fn to_kebab(s: &str) -> String {
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

/// The name of the struct a bare variant implies.
///
/// The enum's name is in it because two enums in one module may each declare a `Sponsors`, and
/// two structs of one name is a worse error than the one this is avoiding.
fn unit_struct_ident(enum_ident: &syn::Ident, variant: &syn::Ident) -> syn::Ident {
    // `unraw` first: a raw identifier prints as `r#type`, and `Ident::new` *panics* on the
    // `#` — a proc-macro panic, which is the worst way for a CLI to learn it used a keyword
    // for a command name.
    let enum_name = enum_ident.unraw();
    let variant_name = variant.unraw();
    // Length-prefixed rather than separated: plain concatenation is ambiguous — `Foo::BarBaz`
    // and `FooBar::Baz` both read as `FooBarBaz` — and an underscore between the parts would
    // land a `non_camel_case_types` warning in the adopter's crate about a type they never
    // wrote. A count is unambiguous and still spells a name Rust is happy with.
    let n = enum_name.to_string().chars().count();
    syn::Ident::new(
        &format!("__Usage{n}{enum_name}{variant_name}"),
        variant.span(),
    )
}

/// One variant: a command name and the struct holding its flags and arguments.
pub struct Variant {
    pub ident: syn::Ident,
    /// Whether the command is kept out of help and completions.
    ///
    /// A spec says `hide=#true` on a `cmd`; mise hides eight commands that way, `asdf` and
    /// `dotfiles` among them. Without this the derive could declare the command but not that
    /// it is unadvertised, so help listed things a user was not meant to be offered.
    pub hide: bool,
    /// The command name, which is the variant name in kebab-case unless `name`
    /// says otherwise.
    pub name: String,
    /// What running this command does to the world, for a variant that holds nothing.
    ///
    /// `effect` is otherwise an `Args` attribute, because that is where a command declares
    /// itself. A bare variant has no `Args` to put it on, and the variant *is* the whole
    /// declaration — so it says it here and the generated struct carries it.
    /// Kept as the word rather than as tokens: it is written straight back out as an
    /// attribute on the struct this variant implies, and parsed there by the same code that
    /// reads every other one — so there is one definition of what the word may be.
    pub effect: Option<String>,
    /// Whether the variant holds nothing and had its struct written for it.
    ///
    /// Only the *construction* differs — `Command::Sponsors` rather than
    /// `Command::Sponsors(x)`. Everything else goes through the struct as usual.
    pub unit: bool,
    /// Fields declared directly on a struct-style enum variant. They are copied
    /// into a generated Args struct, then moved back into the enum after binding.
    pub inline_fields: Option<Vec<syn::Field>>,
    /// The struct the variant wraps, with any `Box` taken off.
    ///
    /// Everything generated speaks to the struct — its tables, its partial, its `build` —
    /// so the box is an artifact of how the variant holds it, restored by
    /// [`boxed`](Self::boxed) at the one point where a value is made.
    pub ty: Type,
    /// Whether the variant holds it in a `Box`.
    ///
    /// mise boxes its largest commands, which is how a CLI with thirty-flag subcommands
    /// keeps its command enum from being as large as its biggest variant — clap struggles
    /// at that size, and `clippy::large_enum_variant` says so about the rest.
    pub boxed: bool,
    /// Whether this variant is clap's `external_subcommand`: an unmatched word plus
    /// the rest of argv, held as `Vec<String>` or `Vec<OsString>`.
    pub external: bool,
    /// Whether the captured argv is `Vec<OsString>` rather than `Vec<String>`.
    pub external_os: bool,
    /// Other names this command answers to.
    ///
    /// Kept apart from [`hidden_aliases`](Self::hidden_aliases) only for the spec: the
    /// parser matches both, and the difference is whether help and completions mention
    /// them. mise has 67 of the first and 24 of the second.
    pub aliases: Vec<String>,
    pub hidden_aliases: Vec<String>,
    pub help: Option<proc_macro2::TokenStream>,
    pub long_help: Option<proc_macro2::TokenStream>,
    pub before_help: Option<proc_macro2::TokenStream>,
    pub before_long_help: Option<proc_macro2::TokenStream>,
    pub after_help: Option<proc_macro2::TokenStream>,
    pub after_long_help: Option<proc_macro2::TokenStream>,
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

        let mut rename_all = None;
        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "rename_all" => rename_all = Some(CasingStyle::parse(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a subcommand enum; \
                                 usage::Subcommands takes `rename_all` here"
                            ),
                        ));
                    }
                }
            }
        }

        let variants = data
            .variants
            .iter()
            .map(|v| Variant::from_variant(v, &input.ident, rename_all))
            .collect::<syn::Result<Vec<_>>>()?;
        for v in &variants {
            if v.effect.is_some() && !v.unit && v.inline_fields.is_none() {
                return Err(syn::Error::new_spanned(
                    &v.ident,
                    "`effect` belongs on the struct this variant wraps, where the command's \
                     other properties are declared — a variant may carry one only when it \
                     holds nothing, because then there is no struct to put it on",
                ));
            }
        }

        let externals: Vec<_> = variants.iter().filter(|v| v.external).collect();
        if externals.len() > 1 {
            return Err(syn::Error::new_spanned(
                &externals[1].ident,
                "a command has one catch-all: only one variant may be `external_subcommand`",
            ));
        }

        // Every name a command answers to, its aliases included: the parser takes the
        // first table entry that matches, so a name claimed twice means one of the two
        // commands can never be reached and nothing would have said so.
        let mut seen: Vec<(&str, Span)> = Vec::new();
        for variant in &variants {
            if variant.external {
                continue;
            }
            for name in std::iter::once(&variant.name)
                .chain(&variant.aliases)
                .chain(&variant.hidden_aliases)
            {
                if let Some((_, first)) = seen.iter().find(|(n, _)| n == name) {
                    return Err(dup(
                        variant.ty.span(),
                        *first,
                        &format!(
                            "`{name}` names two of these commands, counting aliases, so one \
                             of them could never be reached"
                        ),
                    ));
                }
                seen.push((name, variant.ty.span()));
            }
        }

        Ok(Subcommands {
            ident: input.ident.clone(),
            variants,
        })
    }
}

impl Variant {
    fn from_variant(
        variant: &syn::Variant,
        enum_ident: &syn::Ident,
        rename_all: Option<CasingStyle>,
    ) -> syn::Result<Self> {
        let mut verbatim_doc_comment = false;
        // `unraw` first: `r#type` is how a variant named after a keyword prints, and a command
        // called `r#type` is one no user could type. `type` is what they meant.
        let rust_name = variant.ident.unraw().to_string();
        let mut name = rename_all
            .map(|style| style.apply(&rust_name))
            .unwrap_or_else(|| to_kebab(&rust_name));
        let mut aliases: Vec<String> = Vec::new();
        let mut hidden_aliases: Vec<String> = Vec::new();
        let mut effect = None;
        let mut hide = false;
        let mut external = false;
        let mut help_attr: Option<proc_macro2::TokenStream> = None;
        let mut long_help_attr: Option<proc_macro2::TokenStream> = None;
        let mut before_help = None;
        let mut before_long_help = None;
        let mut after_help = None;
        let mut after_long_help = None;

        for attr in attrs(&variant.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => name = strip_dashes(&string_value(&meta)?),
                    // One as a value or several as a list, as the relationship options do.
                    "alias" => aliases.extend(selectors(&meta)?),
                    "alias_hidden" => hidden_aliases.extend(selectors(&meta)?),
                    "hide" => hide = flag_value(&meta)?,
                    "external_subcommand" => external = flag_value(&meta)?,
                    "effect" => {
                        // Checked where it is written, and checked again by the struct's own
                        // derive, which is where it is read — one definition of the word.
                        let word = string_value(&meta)?;
                        effect_value(&meta)?;
                        effect = Some(word);
                    }
                    // As on a field: a comment's paragraph is flowed, so text whose line
                    // breaks matter is declared instead.
                    "help" => help_attr = Some(metadata_expr(&meta)?),
                    "long_help" => long_help_attr = Some(metadata_expr(&meta)?),
                    "before_help" => before_help = Some(metadata_expr(&meta)?),
                    "before_long_help" => before_long_help = Some(metadata_expr(&meta)?),
                    "after_help" => after_help = Some(metadata_expr(&meta)?),
                    "after_long_help" => after_long_help = Some(metadata_expr(&meta)?),
                    "verbatim_doc_comment" => verbatim_doc_comment = flag_value(&meta)?,
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a variant; a subcommand \
                                 variant takes `name`, `alias`, `alias_hidden`, \
                                 `external_subcommand`, `help`, `long_help`, `before_help`, \
                                 `before_long_help`, `after_help`, `after_long_help`, and `verbatim_doc_comment` here, \
                                 and its description comes from the doc comment"
                            ),
                        ));
                    }
                }
            }
        }
        let (help, long_help) = doc_comment(&variant.attrs, verbatim_doc_comment)?;
        for alias in aliases.iter().chain(&hidden_aliases) {
            if alias.is_empty() {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "an alias with no name would answer to nothing",
                ));
            }
            if *alias == name {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    format!("`{alias}` is this command's own name, not another name for it"),
                ));
            }
        }

        let help = help_attr.or_else(|| help.map(|value| quote::quote!(#value)));
        let long_help = long_help_attr.or_else(|| long_help.map(|value| quote::quote!(#value)));

        if external {
            if !aliases.is_empty() || !hidden_aliases.is_empty() {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "`external_subcommand` is a catch-all, not a named command, so it \
                     takes no alias",
                ));
            }
            if effect.is_some() {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "`effect` belongs on a command; `external_subcommand` forwards to one",
                ));
            }
            if hide {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "`external_subcommand` is a catch-all rather than a command help lists, \
                     so there is nothing for `hide` to keep out of help",
                ));
            }
            if help.is_some()
                || long_help.is_some()
                || before_help.is_some()
                || before_long_help.is_some()
                || after_help.is_some()
                || after_long_help.is_some()
            {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "a catch-all has no page of its own, so a description here would be \
                     dropped; describe the forwarding on the command that declares it",
                ));
            }
            let held = match &variant.fields {
                Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                    unnamed.unnamed[0].ty.clone()
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "`external_subcommand` holds the unmatched name plus the rest of \
                         argv, as `External(Vec<String>)` or `External(Vec<OsString>)`",
                    ));
                }
            };
            let Some(inner) = peel(&held, "Vec") else {
                return Err(syn::Error::new_spanned(
                    &held,
                    "`external_subcommand` holds `Vec<String>` or `Vec<OsString>`",
                ));
            };
            let inner_name = type_name(&inner);
            let external_os = match inner_name.as_str() {
                "String" => false,
                "OsString" => true,
                other => {
                    return Err(syn::Error::new_spanned(
                        &inner,
                        format!(
                            "`external_subcommand` holds `Vec<String>` or `Vec<OsString>`, \
                             not `Vec<{other}>`"
                        ),
                    ));
                }
            };
            return Ok(Variant {
                ident: variant.ident.clone(),
                hide: false,
                name,
                effect: None,
                unit: false,
                inline_fields: None,
                ty: held,
                boxed: false,
                external: true,
                external_os,
                aliases,
                hidden_aliases,
                help: None,
                long_help: None,
                before_help: None,
                before_long_help: None,
                after_help: None,
                after_long_help: None,
            });
        }

        // One unnamed field can hold a separately declared Args struct. Named fields make
        // the variant itself the command body; codegen lowers that form to a private Args
        // struct and moves the built values back into the enum.
        // A bare variant is a command with nothing of its own — `Sponsors`, which clap also
        // allows. The struct it implies is written for it, so everything downstream keeps
        // speaking to a struct and only the construction differs.
        let mut unit = false;
        let mut inline_fields = None;
        let held = match &variant.fields {
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => unnamed.unnamed[0].ty.clone(),
            Fields::Unit => {
                unit = true;
                let ident = unit_struct_ident(enum_ident, &variant.ident);
                syn::parse_quote!(#ident)
            }
            Fields::Named(named) => {
                let ident = unit_struct_ident(enum_ident, &variant.ident);
                inline_fields = Some(named.named.iter().cloned().collect());
                syn::parse_quote!(#ident)
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "a subcommand variant wraps one struct, as in `Install(Install)` or \
                     `Install(Box<Install>)` — that struct is where its flags and arguments \
                     are declared — declares fields inline as `Install { force: bool }`, \
                     or holds nothing at all, for a command that has none",
                ));
            }
        };
        // `Box<T>` is read by its written form, as `Option<T>` is elsewhere: a macro
        // cannot resolve a path, and an alias for `Box` is rare enough to leave alone.
        //
        // Taken apart syntactically rather than by rendering the type to a string and
        // parsing it back: `type_name` keeps only the last segment, so a
        // `Box<crate::cmds::Install>` came out as `Install` and the generated code named a
        // type that is not in scope.
        let (ty, boxed) = match unbox(&held) {
            Some(inner) => (inner, true),
            None => (held, false),
        };

        Ok(Variant {
            ident: variant.ident.clone(),
            hide,
            name,
            effect,
            unit,
            inline_fields,
            ty,
            boxed,
            external: false,
            external_os: false,
            aliases,
            hidden_aliases,
            help,
            long_help,
            before_help,
            before_long_help,
            after_help,
            after_long_help,
        })
    }
}

/// An enum whose variants are the words a value may be.
pub struct ValueEnum {
    pub ident: syn::Ident,
    pub ignore_case: bool,
    /// Each variant, and the word it answers to.
    pub variants: Vec<ValueVariant>,
}

pub struct ValueVariant {
    pub ident: syn::Ident,
    pub name: String,
    pub aliases: Vec<ValueAlias>,
    pub help: Option<String>,
    pub hide: bool,
    pub cfg_attrs: Vec<syn::Attribute>,
}

pub struct ValueAlias {
    pub name: String,
    pub hide: bool,
}

#[derive(Clone, Copy)]
enum CasingStyle {
    Camel,
    Kebab,
    Pascal,
    ScreamingSnake,
    Snake,
    Lower,
    Upper,
    Verbatim,
}

impl CasingStyle {
    fn parse(meta: &Meta) -> syn::Result<Self> {
        let raw = string_value(meta)?;
        let normalized = raw
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>();
        match normalized.as_str() {
            "camel" | "camelcase" => Ok(Self::Camel),
            "kebab" | "kebabcase" => Ok(Self::Kebab),
            "pascal" | "pascalcase" => Ok(Self::Pascal),
            "screamingsnake" | "screamingsnakecase" => Ok(Self::ScreamingSnake),
            "snake" | "snakecase" => Ok(Self::Snake),
            "lower" | "lowercase" => Ok(Self::Lower),
            "upper" | "uppercase" => Ok(Self::Upper),
            "verbatim" | "verbatimcase" => Ok(Self::Verbatim),
            _ => Err(syn::Error::new_spanned(
                meta,
                format!("unsupported casing `{raw}`"),
            )),
        }
    }

    fn apply(self, name: &str) -> String {
        match self {
            Self::Camel => name.to_lower_camel_case(),
            Self::Kebab => name.to_kebab_case(),
            Self::Pascal => name.to_upper_camel_case(),
            Self::ScreamingSnake => name.to_shouty_snake_case(),
            Self::Snake => name.to_snake_case(),
            Self::Lower => name.to_snake_case().replace('_', ""),
            Self::Upper => name.to_shouty_snake_case().replace('_', ""),
            Self::Verbatim => name.to_string(),
        }
    }
}

fn cfg_gate_meta(meta: &Meta) -> syn::Result<Option<Meta>> {
    if meta.path().is_ident("cfg") {
        return Ok(Some(meta.clone()));
    }
    if !meta.path().is_ident("cfg_attr") {
        return Ok(None);
    }
    let Meta::List(list) = meta else {
        return Ok(None);
    };
    let nested = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
        .parse2(list.tokens.clone())?;
    let mut nested = nested.into_iter();
    let Some(condition) = nested.next() else {
        return Ok(None);
    };
    let gates = nested
        .map(|meta| cfg_gate_meta(&meta))
        .collect::<syn::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if gates.is_empty() {
        return Ok(None);
    }
    Ok(Some(syn::parse2(quote::quote!(
        cfg_attr(#condition, #(#gates),*)
    ))?))
}

fn cfg_gate_attrs(attrs: &[Attribute]) -> syn::Result<Vec<Attribute>> {
    attrs
        .iter()
        .filter_map(|attr| match cfg_gate_meta(&attr.meta) {
            Ok(Some(meta)) => {
                let mut attr = attr.clone();
                attr.meta = meta;
                Some(Ok(attr))
            }
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        })
        .collect()
}

fn cfg_variants_are_disjoint(left: &[Attribute], right: &[Attribute]) -> bool {
    let left = left
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .filter_map(|attr| attr.parse_args::<Meta>().ok())
        .collect::<Vec<_>>();
    let right = right
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .filter_map(|attr| attr.parse_args::<Meta>().ok())
        .collect::<Vec<_>>();

    left.iter()
        .any(|a| right.iter().any(|b| cfg_predicates_are_disjoint(a, b)))
}

fn cfg_predicates_are_disjoint(left: &Meta, right: &Meta) -> bool {
    let tokens_equal = |a: &Meta, b: &Meta| {
        quote::ToTokens::to_token_stream(a).to_string()
            == quote::ToTokens::to_token_stream(b).to_string()
    };
    let negates = |outer: &Meta, inner: &Meta| {
        let Meta::List(list) = outer else {
            return false;
        };
        list.path.is_ident("not")
            && list
                .parse_args::<Meta>()
                .is_ok_and(|value| tokens_equal(&value, inner))
    };
    if negates(left, right) || negates(right, left) {
        return true;
    }
    if matches!((left, right), (Meta::Path(a), Meta::Path(b)) if
        (a.is_ident("unix") && b.is_ident("windows"))
            || (a.is_ident("windows") && b.is_ident("unix")))
    {
        return true;
    }
    match (left, right) {
        (Meta::NameValue(a), Meta::NameValue(b))
            if a.path
                .get_ident()
                .zip(b.path.get_ident())
                .is_some_and(|(a, b)| {
                    a == b
                        && [
                            "target_abi",
                            "target_arch",
                            "target_endian",
                            "target_env",
                            "target_os",
                            "target_pointer_width",
                            "target_vendor",
                            "panic",
                        ]
                        .iter()
                        .any(|exclusive| a == exclusive)
                }) =>
        {
            quote::ToTokens::to_token_stream(&a.value).to_string()
                != quote::ToTokens::to_token_stream(&b.value).to_string()
        }
        _ => false,
    }
}

impl ValueEnum {
    pub fn from_input(input: &DeriveInput) -> syn::Result<Self> {
        let Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "usage::ValueEnum describes the words one value may be, so it needs an enum",
            ));
        };
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "usage::ValueEnum does not support generic parameters: the word list is a \
                 `const`",
            ));
        }

        let mut ignore_case = false;
        let mut rename_all = None;
        // Registering clap's `value` helper attribute means rustc accepts it at
        // either level. Parse it here too so unsupported enum-wide options fail
        // explicitly instead of being silently ignored.
        for attr in value_attrs(&input.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "ignore_case" => ignore_case = flag_value(&meta)?,
                    "rename_all" => rename_all = Some(CasingStyle::parse(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!("unknown value-enum option `{other}`"),
                        ))
                    }
                }
            }
        }
        let mut variants: Vec<ValueVariant> = Vec::new();
        for variant in &data.variants {
            if !matches!(variant.fields, Fields::Unit) {
                return Err(syn::Error::new_spanned(
                    &variant.fields,
                    "a value is one word, so each variant is a bare name: a variant holding \
                     fields would have nothing to build them from",
                ));
            }
            let cfg_attrs = cfg_gate_attrs(&variant.attrs)?;
            let rust_name = variant.ident.unraw().to_string();
            let mut name = rename_all
                .map(|style| style.apply(&rust_name))
                .unwrap_or_else(|| to_kebab(&rust_name));
            let mut aliases = Vec::new();
            let (doc_help, _) = doc_comment(&variant.attrs, false)?;
            let mut help = doc_help;
            let mut hide = false;
            for attr in value_attrs(&variant.attrs) {
                for meta in nested(attr)? {
                    let path = meta.path().clone();
                    match ident_of(&path).as_str() {
                        "name" => name = string_value(&meta)?,
                        "alias" | "aliases" => {
                            for alias in selectors(&meta)? {
                                if alias.is_empty() {
                                    return Err(syn::Error::new_spanned(
                                        &path,
                                        "an alias with no name would answer to nothing",
                                    ));
                                }
                                aliases.push(ValueAlias {
                                    name: alias,
                                    hide: true,
                                });
                            }
                        }
                        "visible_alias" | "visible_aliases" => {
                            for alias in selectors(&meta)? {
                                if alias.is_empty() {
                                    return Err(syn::Error::new_spanned(
                                        &path,
                                        "an alias with no name would answer to nothing",
                                    ));
                                }
                                aliases.push(ValueAlias {
                                    name: alias,
                                    hide: false,
                                });
                            }
                        }
                        "help" => help = Some(string_value(&meta)?),
                        "hide" => hide = flag_value(&meta)?,
                        other => {
                            return Err(syn::Error::new_spanned(
                                path,
                                format!(
                                    "unknown option `{other}` on a value; a variant takes \
                                     `name`, `alias`, `aliases`, `visible_alias`, \
                                     `visible_aliases`, `help`, or `hide` here"
                                ),
                            ));
                        }
                    }
                }
            }
            if name.is_empty() {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "a value with no name would answer to nothing",
                ));
            }
            variants.push(ValueVariant {
                ident: variant.ident.clone(),
                name,
                aliases,
                help,
                hide,
                cfg_attrs,
            });
        }
        if variants.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "an enum with no variants accepts no value at all",
            ));
        }

        let mut seen: Vec<(&str, Span, &[Attribute])> = Vec::new();
        for variant in &variants {
            for word in ::std::iter::once(variant.name.as_str())
                .chain(variant.aliases.iter().map(|alias| alias.name.as_str()))
            {
                let collision = seen.iter().find(|(seen, _, cfg)| {
                    let same = if ignore_case {
                        seen.eq_ignore_ascii_case(word)
                    } else {
                        *seen == word
                    };
                    same && !cfg_variants_are_disjoint(cfg, &variant.cfg_attrs)
                });
                if let Some((_, first_span, _)) = collision {
                    return Err(dup(
                        variant.ident.span(),
                        *first_span,
                        &format!(
                            "`{word}` names two of these values, counting aliases{}",
                            if ignore_case {
                                " without regard to case"
                            } else {
                                ""
                            }
                        ),
                    ));
                }
                seen.push((word, variant.ident.span(), &variant.cfg_attrs));
            }
        }

        Ok(ValueEnum {
            ident: input.ident.clone(),
            ignore_case,
            variants,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Kind, Subcommands, ValueEnum};

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
    fn unit_structs_are_empty_commands() {
        let cli = cli("struct Empty;").expect("unit commands should compile");
        assert!(cli.unit);
        assert!(cli.fields.is_empty());
    }

    #[test]
    fn tuple_structs_explain_the_named_flatten_rewrite() {
        let err = rejection("struct Wrapper(InnerArgs);");
        assert!(err.contains("tuple field"), "unhelpful: {err}");
        assert!(err.contains("#[usage(flatten)]"), "unhelpful: {err}");
    }

    #[test]
    fn computed_program_identity_keeps_a_portable_literal() {
        let err = rejection("#[usage(name = runtime_name())] struct Root {}");
        assert!(err.contains("name_spec"), "unhelpful: {err}");
        let err = rejection("#[usage(bin = runtime_bin())] struct Root {}");
        assert!(err.contains("bin_spec"), "unhelpful: {err}");

        let parsed = cli(
            "#[usage(name = runtime_name(), name_spec = \"portable\", bin = runtime_bin(), bin_spec = \"portable-bin\")] struct Root {}",
        )
        .expect("runtime identity with portable values should compile");
        assert_eq!(parsed.name, "portable");
        assert_eq!(parsed.bin.as_deref(), Some("portable-bin"));
        assert!(parsed.runtime_name.is_some());
        assert!(parsed.runtime_bin.is_some());
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
    fn clap_id_and_visible_alias_spellings_are_lossless() {
        let cli = cli(r#"
            #[command(rename_all = "kebab-case")]
            struct Ex {
                #[arg(id = "output", long, visible_aliases = ["out", "dest"], aliases = ["quietly", "silent-output"])]
                path: Option<String>,
            }
        "#)
        .expect("lossless clap spellings should compile");

        let field = &cli.fields[0];
        assert_eq!(field.name, "output");
        let Kind::Flag {
            longs,
            hidden_longs,
            ..
        } = &field.kind
        else {
            panic!("long should make this a flag");
        };
        assert_eq!(
            longs,
            &["output", "out", "dest", "quietly", "silent-output"]
        );
        assert_eq!(hidden_longs, &["quietly", "silent-output"]);
    }

    #[test]
    fn clap_kebab_casing_aliases_are_no_ops() {
        for case in ["kebab", "kebab-case", "kebab_case", "KebabCase"] {
            cli(&format!(
                r#"
                    #[command(rename_all = "{case}")]
                    struct Ex {{
                        #[arg(long)]
                        output_path: Option<String>,
                    }}
                "#
            ))
            .unwrap_or_else(|err| panic!("{case} should mean kebab case: {err}"));
        }
    }

    #[test]
    fn explicit_long_stays_canonical_after_a_visible_alias() {
        let cli = cli(r#"
            struct Ex {
                #[arg(visible_alias = "out", long = "result")]
                path: Option<String>,
            }
        "#)
        .expect("attribute order must not change the canonical spelling");

        let field = &cli.fields[0];
        assert_eq!(field.name, "result");
        let Kind::Flag { longs, .. } = &field.kind else {
            panic!("long should make this a flag");
        };
        assert_eq!(longs, &["result", "out"]);
    }

    #[test]
    fn a_hidden_alias_does_not_turn_a_positional_into_a_flag() {
        let err = rejection(
            r#"
            struct Ex {
                #[arg(alias = "quietly")]
                path: Option<String>,
            }
        "#,
        );
        assert!(err.contains("also needs `long` or `short`"), "{err}");
    }

    #[test]
    fn lossy_clap_field_spellings_get_migration_diagnostics() {
        let arity = rejection(
            r#"
            struct Ex {
                #[arg(long, num_args = 2)]
                pair: Vec<String>,
            }
        "#,
        );
        assert!(arity.contains("var_min"), "{arity}");

        let parser = rejection(
            r#"
            struct Ex {
                #[arg(long, value_parser = clap::value_parser!(u16))]
                port: u16,
            }
        "#,
        );
        assert!(parser.contains("FromStr"), "{parser}");
    }

    #[test]
    fn the_list_of_field_options_reads_as_a_sentence() {
        // A line continuation without its backslash left a long run of spaces in the middle of
        // the message, so the one place that tells an author what a field accepts looked
        // corrupted.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, nonsense)]
                out: Option<String>,
            }
        "#,
        );
        assert!(err.contains("unknown option `nonsense`"), "{err}");
        assert!(
            !err.contains("  "),
            "the list of options has a run of spaces in it: {err}"
        );
    }

    #[test]
    fn required_on_a_collection_is_refused_where_it_would_contradict_itself() {
        // `required` on a `Vec` means one or more values, always — and the check for it runs
        // unconditionally, because that is what it says. Each of these declarations means
        // something else, and all three compiled: the condition was emitted into the spec and
        // never consulted, and the `Option` could never be `None`.
        for decl in [
            "#[usage(long, required, required_if = \"--other\")]\n                tag: Vec<String>,\n                #[usage(long)]\n                other: bool,",
            "#[usage(long, required, required_unless(\"--other\"))]\n                tag: Vec<String>,\n                #[usage(long)]\n                other: bool,",
            "#[usage(long, required)]\n                tag: Option<Vec<String>>,",
        ] {
            let err = rejection(&format!("struct Ex {{\n                {decl}\n            }}"));
            assert!(
                err.contains("one or more values, always"),
                "unhelpful message for `{decl}`: {err}"
            );
        }
        // A plain required collection is still fine, which is what the refusals are protecting.
        let cli = cli(r#"
            struct Ex {
                #[usage(long, required)]
                tag: Vec<String>,
            }
        "#)
        .expect("should compile");
        assert!(cli.fields[0].required_collection);
    }

    #[test]
    fn value_optional_is_refused_where_nothing_would_read_it() {
        // It says a *flag's* value may be left off. `arg_meta` never emits it, and a valueless
        // flag has no value to make optional — so on a positional or a `bool`/`count` flag the
        // declaration compiled and was dropped. Worse than an error: a positional's brackets
        // come from its type, so the page went on saying the opposite of what was asked.
        for (decl, word) in [
            (
                "#[usage(arg, value_optional)]\n                out: Option<String>,",
                "type",
            ),
            // No `arg` either: a field with no `long`, `short` or `arg` is a positional too,
            // and testing `is_arg` rather than `is_flag` let this one through.
            (
                "#[usage(value_optional)]\n                out: Option<String>,",
                "type",
            ),
            (
                "#[usage(long, value_optional)]\n                out: bool,",
                "no value",
            ),
            (
                "#[usage(long, count, value_optional)]\n                out: u8,",
                "no value",
            ),
        ] {
            let err = rejection(&format!(
                "struct Ex {{\n                {decl}\n            }}"
            ));
            assert!(err.contains(word), "unhelpful message for `{decl}`: {err}");
        }
        // And where there is a flag value, it still works.
        let cli = cli(r#"
            struct Ex {
                #[usage(long, value_optional)]
                bump: Option<String>,
            }
        "#)
        .expect("should compile");
        assert!(cli.fields[0].value_optional);
    }

    #[test]
    fn value_name_is_refused_where_there_is_no_value_to_name() {
        // It names the placeholder a flag's *value* gets in help. `arg_meta` never emits it, and
        // a valueless flag has no placeholder to put it in — so on a positional or a
        // `bool`/`count` flag the declaration compiled and was dropped, reading as though it had
        // done something.
        for decl in [
            "#[usage(arg, value_name = \"FILE\")]\n                out: String,",
            "#[usage(long, value_name = \"FILE\")]\n                out: bool,",
            "#[usage(long, count, value_name = \"FILE\")]\n                out: u8,",
        ] {
            let err = rejection(&format!(
                "struct Ex {{\n                {decl}\n            }}"
            ));
            assert!(
                err.contains("placeholder"),
                "unhelpful message for `{decl}`: {err}"
            );
        }
        // And where there *is* a value, it still works.
        let cli = cli(r#"
            struct Ex {
                #[usage(long, value_name = "FILE")]
                out: Option<String>,
            }
        "#)
        .expect("should compile");
        assert_eq!(
            cli.fields[0].value_name.as_deref(),
            Some("FILE"),
            "a value-taking flag keeps it"
        );
    }

    #[test]
    fn exclusive_is_refused_on_a_positional() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(exclusive)]
                target: String,
            }
        "#,
        );
        assert!(
            err.contains("`exclusive` describes a flag"),
            "unhelpful message: {err}"
        );

        cli(r#"
            struct Ex {
                #[usage(long, exclusive)]
                dump: bool,
            }
        "#)
        .expect("exclusive remains valid on a flag");
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
        assert!(
            err.contains("names no argument"),
            "unhelpful message: {err}"
        );

        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, requires_if("json", "--schema"))]
                format: Option<String>,
            }
        "#,
        );
        assert!(err.contains("names no flag"), "unhelpful message: {err}");
    }

    #[test]
    fn conditional_requirements_need_pairs_and_a_flag() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, requires_if("json"))]
                format: Option<String>,
            }
        "#,
        );
        assert!(err.contains("exactly a value and a flag"), "{err}");

        let err = rejection(
            r#"
            struct Ex {
                #[usage(requires_if("json", "--schema"))]
                format: Option<String>,
                #[usage(long)]
                schema: Option<String>,
            }
        "#,
        );
        assert!(err.contains("relationship between flags"), "{err}");
    }

    #[test]
    fn conditional_defaults_need_a_flag_and_a_known_selector() {
        let cli = cli(r#"
            struct Ex {
                #[usage(long, default_if("--json", "true"))]
                bin_names: bool,
                #[usage(long)]
                json: bool,
                #[usage(long, default_if("--output", "json", "pretty"))]
                style: Option<String>,
                #[usage(long)]
                output: Option<String>,
            }
        "#)
        .expect("should compile");
        assert_eq!(cli.fields[0].default_if.len(), 1);
        assert!(cli.fields[0].default_if[0].when.is_none());
        assert_eq!(cli.fields[2].default_if[0].when.as_deref(), Some("json"));

        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, default_if("--json", "true"))]
                bin_names: bool,
            }
        "#,
        );
        assert!(err.contains("names no flag"), "{err}");

        let err = rejection(
            r#"
            struct Ex {
                #[usage(default_if("--json", "true"))]
                bin_names: bool,
                #[usage(long)]
                json: bool,
            }
        "#,
        );
        assert!(err.contains("relationship between flags"), "{err}");

        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, default_if("--json"))]
                bin_names: bool,
                #[usage(long)]
                json: bool,
            }
        "#,
        );
        assert!(err.contains("two arguments"), "{err}");
    }

    #[test]
    fn every_default_if_value_has_to_be_one_of_the_choices() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, default_if("--json", "wat"), choices("auto", "always", "never"))]
                color: Option<String>,
                #[usage(long)]
                json: bool,
            }
        "#,
        );
        assert!(
            err.contains("the default_if value `wat`"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn only_a_collection_takes_more_than_one_default() {
        // Several defaults is a `Vec`'s privilege, because a `Vec` is the only shape with
        // somewhere to put them. Keeping the last silently would leave the others declared,
        // emitted into the spec, and never applied.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, default = "a", default = "b")]
                out: Option<String>,
            }
        "#,
        );
        assert!(err.contains("one `default`"), "unhelpful message: {err}");
    }

    #[test]
    fn every_default_has_to_be_one_of_the_choices() {
        // Each of them, not the first: a collection's second default is as unusable as its
        // first if the choices do not allow it.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, var, choices("a", "b"), default = "a", default = "z")]
                out: Vec<String>,
            }
        "#,
        );
        assert!(err.contains("the default `z`"), "unhelpful message: {err}");
    }

    #[test]
    fn every_default_missing_has_to_be_one_of_the_choices() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, default_missing = "wat", choices("auto", "always", "never"))]
                color: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("the default_missing `wat`"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn only_required_exempts_an_argument_from_the_variadic_rule() {
        // `automatic` and `preserve` are about *flags* and about what a `--` means; neither ends
        // a variadic, so neither buys an argument a place behind an unbounded one. Easy to get
        // backwards now that all four modes can be written, and the cost of getting it backwards
        // is a field that compiles and can never be filled.
        for mode in ["automatic", "preserve"] {
            let err = rejection(&format!(
                r#"
                struct Ex {{
                    #[usage(arg)]
                    args: Vec<String>,
                    #[usage(arg, double_dash = "{mode}")]
                    rest: Vec<String>,
                }}
            "#
            ));
            assert!(
                err.contains("can never be filled"),
                "`{mode}` should not exempt an argument: {err}"
            );
        }
    }

    #[test]
    fn nothing_follows_a_variadic_that_keeps_the_separator() {
        // The one exemption from the can-never-be-filled rule is an argument that waits for a
        // `--`, on the grounds that the separator ends the variadic in front of it. A `preserve`
        // variadic takes the `--` as a value instead, so it ends nothing and the exemption is
        // false — the declaration compiled and the field could never hold anything.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, double_dash = "preserve")]
                rest: Vec<String>,
                #[usage(arg, double_dash = "required")]
                after: Vec<String>,
            }
        "#,
        );
        assert!(
            err.contains("keeps the `--` as a value"),
            "unhelpful message: {err}"
        );

        // A bound puts it back: the variadic hands over once it is full, separator or not.
        assert!(
            cli(r#"
            struct Ex {
                #[usage(arg, double_dash = "preserve", var_max = 2)]
                rest: Vec<String>,
                #[usage(arg, double_dash = "required")]
                after: Vec<String>,
            }
        "#)
            .is_ok(),
            "a bounded variadic hands over, so what follows is reachable"
        );
    }

    #[test]
    fn a_mode_the_spec_does_not_have_is_refused() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, double_dash = "sometimes")]
                rest: Vec<String>,
            }
        "#,
        );
        assert!(err.contains("is not a mode"), "unhelpful message: {err}");
        // And the message lists them, because the four names are not guessable.
        assert!(err.contains("preserve"), "unhelpful message: {err}");
    }

    #[test]
    fn only_one_argument_can_live_after_the_separator() {
        // The exemption is good for one argument, because a command line has one `--`.
        // A second variadic behind the separator would be as unreachable as an argument
        // after a plain variadic.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg)]
                args: Vec<String>,
                #[usage(arg, double_dash = "required")]
                rest: Vec<String>,
                #[usage(arg, double_dash = "required")]
                more: Vec<String>,
            }
        "#,
        );
        assert!(
            err.contains("only separator there is"),
            "unhelpful message: {err}"
        );

        // Including a plain argument, which the unbounded variadic behind the separator
        // would already have taken.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, double_dash = "required")]
                rest: Vec<String>,
                #[usage(arg)]
                trailing: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("only separator there is"),
            "unhelpful message: {err}"
        );
    }

    fn subcommands(body: &str) -> syn::Result<Subcommands> {
        Subcommands::from_input(&syn::parse_str::<syn::DeriveInput>(body).expect("valid Rust"))
    }

    /// The message a bad enum produces.
    fn enum_rejection(body: &str) -> String {
        match subcommands(body) {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn an_external_subcommand_holds_a_vec_of_strings() {
        let subs = subcommands(
            r#"
            enum Commands {
                Install(Install),
                #[usage(external_subcommand)]
                External(Vec<String>),
            }
        "#,
        )
        .expect("should compile");
        assert!(subs.variants[1].external);
        assert!(!subs.variants[1].external_os);

        let err = enum_rejection(
            r#"
            enum Commands {
                #[usage(external_subcommand)]
                Extra(::std::ffi::OsString),
            }
        "#,
        );
        assert!(err.contains("Vec<String>"), "{err}");

        let err = enum_rejection(
            r#"
            enum Commands {
                #[usage(external_subcommand)]
                Extra(Vec<u8>),
            }
        "#,
        );
        assert!(err.contains("Vec<String>"), "{err}");

        let err = enum_rejection(
            r#"
            enum Commands {
                #[usage(external_subcommand)]
                A(Vec<String>),
                #[usage(external_subcommand)]
                B(Vec<String>),
            }
        "#,
        );
        assert!(err.contains("only one variant"), "{err}");

        let err = enum_rejection(
            r#"
            enum Commands {
                #[usage(hide, external_subcommand)]
                External(Vec<String>),
            }
        "#,
        );
        assert!(err.contains("hide"), "{err}");

        let err = enum_rejection(
            r#"
            enum Commands {
                /// forwarded argv
                #[usage(external_subcommand)]
                External(Vec<String>),
            }
        "#,
        );
        assert!(
            err.contains("description") || err.contains("catch-all"),
            "{err}"
        );
    }

    fn value_enum(body: &str) -> syn::Result<ValueEnum> {
        ValueEnum::from_input(&syn::parse_str::<syn::DeriveInput>(body).expect("valid Rust"))
    }

    #[test]
    fn a_value_enum_takes_bare_variants_with_distinct_words() {
        let ve = value_enum(
            r#"
            enum Shell {
                Bash,
                #[usage(name = "pwsh")]
                PowerShell,
            }
        "#,
        )
        .expect("should compile");
        assert_eq!(
            ve.variants
                .iter()
                .map(|value| value.name.as_str())
                .collect::<Vec<_>>(),
            ["bash", "pwsh"]
        );

        // A variant holding fields has nothing to build them from: a value is one word.
        let err = match value_enum("enum Shell { Bash, Other(String) }") {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("bare name"), "unhelpful message: {err}");

        // Two variants answering to one word means one is unreachable.
        let err = match value_enum(
            r#"
            enum Shell {
                #[usage(name = "sh")]
                Bash,
                #[usage(name = "sh")]
                Dash,
            }
        "#,
        ) {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("names two of these values"),
            "unhelpful: {err}"
        );
    }

    #[test]
    fn a_value_enum_accepts_clap_value_metadata() {
        let ve = value_enum(
            r#"
            enum Provider {
                /// Password manager.
                #[value(name = "1password", alias = "op", visible_alias = "one", hide)]
                OnePassword,
            }
        "#,
        )
        .expect("clap value metadata should remain usable");
        assert_eq!(ve.variants[0].name, "1password");
        assert_eq!(ve.variants[0].help.as_deref(), Some("Password manager."));
        assert!(ve.variants[0].hide);
        assert_eq!(ve.variants[0].aliases[0].name, "op");
        assert!(ve.variants[0].aliases[0].hide);
        assert_eq!(ve.variants[0].aliases[1].name, "one");
        assert!(!ve.variants[0].aliases[1].hide);
    }

    #[test]
    fn a_value_enum_accepts_clap_container_casing() {
        let legacy = value_enum("enum Protocol { HTTPServer }")
            .expect("the default naming policy should compile");
        assert_eq!(legacy.variants[0].name, "h-t-t-p-server");

        for (style, expected) in [
            ("camelCase", "onePassword"),
            ("kebab-case", "one-password"),
            ("PascalCase", "OnePassword"),
            ("SCREAMING_SNAKE_CASE", "ONE_PASSWORD"),
            ("snake_case", "one_password"),
            ("lowercase", "onepassword"),
            ("UPPERCASE", "ONEPASSWORD"),
            ("verbatim", "OnePassword"),
        ] {
            let ve = value_enum(&format!(
                r#"
                #[value(rename_all = "{style}")]
                enum Provider {{ OnePassword }}
            "#
            ))
            .expect("clap container metadata should remain usable");
            assert_eq!(ve.variants[0].name, expected, "style {style}");
        }

        let err = match value_enum(
            r#"
            #[value(rename_all = "train-case")]
            enum Provider { OnePassword }
        "#,
        ) {
            Ok(_) => panic!("unsupported casing must be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("unsupported casing `train-case`"),
            "unhelpful error: {err}"
        );
    }

    #[test]
    fn a_value_enum_rejects_alias_collisions() {
        for body in [
            r#"
                enum Shell {
                    Bash,
                    #[usage(alias = "bash")]
                    Dash,
                }
            "#,
            r#"
                enum Shell {
                    #[usage(alias = "sh")]
                    Bash,
                    #[usage(alias = "sh")]
                    Dash,
                }
            "#,
            r#"
                #[usage(ignore_case)]
                enum Shell {
                    #[usage(alias = "SH")]
                    Bash,
                    #[usage(name = "sh")]
                    Dash,
                }
            "#,
        ] {
            let err = match value_enum(body) {
                Ok(_) => panic!("should not have compiled"),
                Err(e) => e.to_string(),
            };
            assert!(err.contains("counting aliases"), "unhelpful: {err}");
        }

        let err = match value_enum(
            r#"
                enum Shell {
                    #[usage(alias = "")]
                    Bash,
                }
            "#,
        ) {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("no name"), "unhelpful: {err}");
    }

    #[test]
    fn a_conditional_value_keeps_its_cfg_for_static_emission() {
        let value = value_enum(
            r#"
            enum Shell {
                Bash,
                #[cfg(windows)]
                PowerShell,
            }
        "#,
        )
        .expect("conditional variants should compile");
        assert!(value.variants[0].cfg_attrs.is_empty());
        assert_eq!(value.variants[1].cfg_attrs.len(), 1);
    }

    #[test]
    fn cfg_attr_keeps_only_variant_gates_for_static_emission() {
        let value = value_enum(
            r#"
            enum Shell {
                #[cfg_attr(windows, cfg(target_pointer_width = "64"), doc = "Windows shell")]
                PowerShell,
                #[cfg_attr(all(), doc = "Unix shell")]
                Bash,
            }
        "#,
        )
        .expect("non-gating cfg_attr metadata should be ignored");
        assert_eq!(value.variants[0].cfg_attrs.len(), 1);
        let emitted = quote::ToTokens::to_token_stream(&value.variants[0].cfg_attrs[0]).to_string();
        assert!(emitted.contains("cfg_attr"));
        assert!(emitted.contains("target_pointer_width"));
        assert!(!emitted.contains("doc"));
        assert!(value.variants[1].cfg_attrs.is_empty());
    }

    #[test]
    fn cfg_disjoint_values_may_reuse_one_cli_word() {
        value_enum(
            r#"
            enum Shell {
                #[cfg(windows)]
                #[usage(name = "native")]
                Windows,
                #[cfg(not(windows))]
                #[usage(name = "native")]
                Unix,
            }
        "#,
        )
        .expect("only one cfg-disjoint word exists on any target");
    }

    #[test]
    fn independently_true_target_cfgs_may_not_reuse_one_cli_word() {
        let result = value_enum(
            r#"
            enum AtomicWidth {
                #[cfg(target_has_atomic = "8")]
                #[usage(name = "native")]
                Eight,
                #[cfg(target_has_atomic = "16")]
                #[usage(name = "native")]
                Sixteen,
            }
        "#,
        );
        let Err(err) = result else {
            panic!("a target may support both atomic widths")
        };
        let err = err.to_string();
        assert!(
            err.contains("names two of these values"),
            "unhelpful: {err}"
        );
    }

    /// The position rules, which each derive applies for the place it stands in.
    fn position_error(body: &str, is_root: bool) -> String {
        let parsed = cli(body).expect("parses");
        let ident = syn::Ident::new("Probe", proc_macro2::Span::call_site());
        parsed
            .check_position(&ident, is_root)
            .expect_err("should have been refused")
            .to_string()
    }

    #[test]
    fn an_effect_the_spec_does_not_have_is_refused() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, effect = "mostly-harmless")]
                force: bool,
            }
        "#,
        );
        assert!(err.contains("is not one the spec has"), "unhelpful: {err}");
        // The message lists them, because three words are not guessable from the attribute.
        assert!(err.contains("destructive"), "unhelpful: {err}");
    }

    #[test]
    fn one_spec_makes_one_claim_about_which_usage_can_read_it() {
        // Only the root emits a spec, so only the root can say this. Accepted on an `Args` it
        // was parsed, stored, and dropped.
        let err = position_error(
            r#"
            #[usage(min_usage_version = "4.0")]
            struct Ex {
                #[usage(long)]
                plain: bool,
            }
        "#,
            false,
        );
        assert!(
            err.contains("`min_usage_version` belongs on the root"),
            "unhelpful: {err}"
        );
    }

    #[test]
    fn only_a_bare_variant_declares_its_own_effect() {
        // A variant that wraps a struct has somewhere to put it, and two places to say one
        // thing is two answers waiting to disagree.
        let input = syn::parse_str::<syn::DeriveInput>(
            r#"
            enum Commands {
                #[usage(effect = "write")]
                Install(Install),
            }
        "#,
        )
        .expect("valid Rust");
        let Err(err) = Subcommands::from_input(&input) else {
            panic!("should not have compiled")
        };
        let err = err.to_string();
        assert!(err.contains("belongs on the struct"), "unhelpful: {err}");
    }

    #[test]
    fn an_effect_belongs_to_a_flag_or_a_command_and_not_to_an_argument() {
        // A positional is what is acted on, not a choice to act — and `arg_meta` has nowhere to
        // put an effect, so a declaration here was stored and then dropped, which reads as
        // though it had done something.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, effect = "destructive")]
                path: String,
            }
        "#,
        );
        assert!(err.contains("is what is acted on"), "unhelpful: {err}");
    }

    #[test]
    fn a_subcommand_field_takes_nothing_but_subcommand() {
        // This branch used to *ignore* whatever else was written, so `effect` on it compiled
        // and said nothing. Refused as a class: the field holds a set of commands, and
        // everything else the attribute can say describes a value or a flag.
        for other in ["effect = \"write\"", "long", "default = \"x\"", "global"] {
            let err = rejection(&format!(
                r#"
                struct Ex {{
                    #[usage(subcommand, {other})]
                    command: Option<Commands>,
                }}
            "#
            ));
            assert!(
                err.contains("says nothing about a `subcommand` field"),
                "`{other}` should be refused: {err}"
            );
        }
    }

    #[test]
    fn the_root_does_nothing_to_the_world() {
        // Bare `communique` does nothing; one of its commands does. The spec writer asserts the
        // root carries no effect, so accepting it here would trade a message for a
        // `debug_assert!` in the writer — which is a poor way to learn where an attribute goes.
        let err = position_error(
            r#"
            #[usage(effect = "write")]
            struct Ex {
                #[usage(long)]
                plain: bool,
            }
        "#,
            true,
        );
        assert!(
            err.contains("`effect` belongs on a command"),
            "unhelpful: {err}"
        );
    }

    #[test]
    fn settings_needs_something_to_collect() {
        // The attribute says the flags are declared *elsewhere*, so there has to be an
        // elsewhere. With none, the generated layer called a `settings_given` that nothing
        // emitted, and an adopter's build failed with `cannot find function settings_given`
        // pointing at `#[derive(Cli)]` — naming neither the attribute nor the mistake.
        let input = syn::parse_str::<syn::DeriveInput>(
            r#"
            #[usage(settings)]
            struct Ex {
                #[usage(long)]
                plain: bool,
            }
        "#,
        )
        .expect("valid Rust");
        let cli = Cli::from_input(&input).expect("parses");
        let err = cli
            .check_position(&input.ident, true)
            .expect_err("should not have compiled")
            .to_string();
        assert!(err.contains("no elsewhere"), "unhelpful message: {err}");

        // Any of the three is enough, and each is a different way for a flag to be somewhere
        // this struct does not declare it.
        for body in [
            r#"struct Ex { #[usage(long, setting = "jobs")] jobs: Option<String> }"#,
            r#"struct Ex { #[usage(flatten)] group: Group }"#,
            r#"struct Ex { #[usage(subcommand)] command: Option<Commands> }"#,
        ] {
            let body = format!("#[usage(settings)]\n{body}");
            let input = syn::parse_str::<syn::DeriveInput>(&body).expect("valid Rust");
            let cli = Cli::from_input(&input).expect("parses");
            assert!(
                cli.check_position(&input.ident, true).is_ok(),
                "should have been accepted: {body}"
            );
        }
    }

    #[test]
    fn the_settings_attribute_belongs_on_the_root() {
        // It says "this CLI resolves settings whose flags are declared elsewhere", which only a
        // root can mean: a group is asked for its settings by whatever flattens it, and answers
        // whenever it has any. Accepted here it would have been parsed and never read — the
        // silence the attribute exists to replace with a compile error.
        let err = position_error(
            r#"
            #[usage(settings)]
            struct Ex {
                #[usage(long)]
                jobs: Option<usize>,
            }
            "#,
            false,
        );
        assert!(
            err.contains("`settings` belongs on the root"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn a_setting_is_allowed_wherever_a_flag_is() {
        // It used to be refused outside the root, because only the root generated a layer and one
        // written on a flattened group compiled into nothing. A group hands its bindings to its
        // parent now, so the place a flag is declared is the place its setting is declared.
        let body = r#"
            struct Ex {
                #[usage(long, setting = "jobs")]
                jobs: Option<usize>,
            }
            "#;
        let parsed = cli(body).expect("parses");
        let ident = syn::Ident::new("Probe", proc_macro2::Span::call_site());
        parsed
            .check_position(&ident, false)
            .expect("allowed in a group");
        parsed
            .check_position(&ident, true)
            .expect("and on the root");
    }

    #[test]
    fn a_short_form_must_be_ascii() {
        // Enforced for two reasons now: a multi-byte short could never be matched, and
        // `os_string_from_bytes` relies on every cut the parser makes landing on an ASCII
        // byte. A cluster is walked one byte at a time, so this is the rule that keeps a
        // value from beginning in the middle of a character.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(short = 'é', long)]
                enable: bool,
            }
        "#,
        );
        assert!(err.contains("is not ASCII"), "unhelpful message: {err}");
    }

    #[test]
    fn required_is_refused_where_the_type_already_answers() {
        // Required-ness is the type's to say everywhere but a collecting field. Accepting it
        // elsewhere would mean a declaration that either repeats the type or contradicts it,
        // and someone would eventually trust the wrong one.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, required)]
                target: Option<String>,
            }
        "#,
        );
        assert!(err.contains("contradicts `Option`"), "unhelpful: {err}");

        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, required)]
                target: String,
            }
        "#,
        );
        assert!(err.contains("already required"), "unhelpful: {err}");
    }

    #[test]
    fn flatten_cannot_be_combined_with_a_flag() {
        // `#[usage(flatten, long)]` reads as though the flattening were also a flag, and
        // there is no such thing — the flattened struct's own fields say what they are.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(flatten, long)]
                shared: Shared,
            }
        "#,
        );
        assert!(
            err.contains("cannot be combined with `long`"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn flatten_takes_no_value() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(flatten = "shared")]
                shared: Shared,
            }
        "#,
        );
        assert!(err.contains("takes no value"), "unhelpful message: {err}");
    }

    #[test]
    fn skip_is_a_field_that_is_not_a_flag_or_an_argument() {
        let cli = cli(r#"
            struct Ex {
                #[usage(long)]
                force: bool,
                #[usage(skip)]
                computed: usize,
            }
        "#)
        .expect("should compile");
        assert!(
            cli.fields
                .iter()
                .any(|f| matches!(f.kind, super::Kind::Skip) && f.ident == "computed"),
            "skip should be a kind of its own, not a flag whose tables then ignore it"
        );
        assert_eq!(
            cli.fields
                .iter()
                .filter(|f| matches!(f.kind, super::Kind::Flag { .. }))
                .count(),
            1,
            "a skipped field must not appear as a flag"
        );
    }

    #[test]
    fn skip_cannot_be_combined_with_a_flag_option() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(skip, long)]
                computed: usize,
            }
        "#,
        );
        assert!(
            err.contains("cannot be combined with `long`"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn allow_hyphen_values_is_a_flag_that_takes_a_value() {
        let cli = cli(r#"
            struct Ex {
                #[usage(long, allow_hyphen_values)]
                args: Option<String>,
            }
        "#)
        .expect("should compile");
        assert!(
            cli.fields.iter().any(|f| f.allow_hyphen_values),
            "the attribute has to reach the field the table is built from"
        );
    }

    #[test]
    fn allow_hyphen_values_cannot_sit_on_a_switch() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, allow_hyphen_values)]
                force: bool,
            }
        "#,
        );
        assert!(err.contains("takes none"), "unhelpful message: {err}");
    }

    #[test]
    fn allow_hyphen_values_cannot_sit_on_a_positional() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg, allow_hyphen_values)]
                rest: Vec<String>,
            }
        "#,
        );
        assert!(
            err.contains("double_dash"),
            "should point at the positional spelling: {err}"
        );
    }

    #[test]
    fn require_equals_is_a_flag_that_takes_a_value() {
        let cli = cli(r#"
            struct Ex {
                #[usage(long, require_equals)]
                inspect: Option<String>,
            }
        "#)
        .expect("should compile");
        assert!(
            cli.fields.iter().any(|f| f.require_equals),
            "the attribute has to reach the field the table is built from"
        );
    }

    #[test]
    fn require_equals_cannot_sit_on_a_switch() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, require_equals)]
                force: bool,
            }
        "#,
        );
        assert!(err.contains("takes none"), "unhelpful message: {err}");
    }

    #[test]
    fn default_missing_is_a_flag_that_takes_a_value() {
        let cli = cli(r#"
            struct Ex {
                #[usage(long, default_missing = "always")]
                color: Option<String>,
            }
        "#)
        .expect("should compile");
        assert_eq!(
            cli.fields[0].default_missing.as_deref(),
            Some("always"),
            "the attribute has to reach the field the table is built from"
        );
        assert!(
            cli.fields[0].value_optional,
            "a flag that can be given without a value should show one as optional"
        );
    }

    #[test]
    fn default_missing_cannot_sit_on_a_switch() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, default_missing = "always")]
                force: bool,
            }
        "#,
        );
        assert!(err.contains("takes none"), "unhelpful message: {err}");
    }

    #[test]
    fn flatten_on_an_option_is_refused_rather_than_guessed_at() {
        // `Option<T>` would mean "the whole group, or nothing", which needs a rule for when
        // the group counts as given. clap's answer is "any of its fields" — defensible, not
        // obviously right, and nothing in the fleet asks for it. Refused until something does.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(flatten)]
                shared: Option<Shared>,
            }
        "#,
        );
        assert!(err.contains("not supported"), "unhelpful message: {err}");
    }

    #[test]
    fn a_default_subcommand_needs_subcommands_to_name() {
        let err = position_error(
            r#"
            #[usage(bin = "ex", default_subcommand = "run")]
            struct Ex {
                #[usage(arg)]
                task: Option<String>,
            }
        "#,
            true,
        );
        assert!(
            err.contains("no subcommands to name"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn a_default_subcommand_is_refused_below_the_root() {
        // A spec declares one for the whole program, so a nested one has nowhere to go —
        // and `emit_args` would drop it in silence, which is worse than refusing it.
        let err = position_error(
            r#"
            #[usage(default_subcommand = "inner")]
            struct Nested {
                #[usage(subcommand)]
                command: Option<Commands>,
            }
        "#,
            false,
        );
        assert!(
            err.contains("belongs on the root"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn multicall_needs_subcommands_to_select() {
        let err = position_error(
            r#"
            #[usage(bin = "ex", multicall)]
            struct Ex {
                #[usage(arg)]
                task: Option<String>,
            }
        "#,
            true,
        );
        assert!(
            err.contains("no subcommands to select"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn multicall_is_refused_below_the_root() {
        let err = position_error(
            r#"
            #[usage(multicall)]
            struct Nested {
                #[usage(subcommand)]
                command: Option<Commands>,
            }
        "#,
            false,
        );
        assert!(
            err.contains("belongs on the root"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn a_mount_and_a_restart_token_are_refused_on_the_root() {
        // The spec writes both on a `cmd` node and the root is not one — usage-lib rejects a
        // spec with either at the top. Emitted anyway, they trip a `debug_assert!` in the KDL
        // writer and vanish in release, so the declaration is where to say no.
        for (attr, word) in [
            (r#"mount = "ex tasks --usage""#, "mount"),
            (r#"restart_token = ":::""#, "restart_token"),
        ] {
            let err = position_error(
                &format!(
                    r#"
                    #[usage(bin = "ex", {attr})]
                    struct Ex {{
                        #[usage(long)]
                        force: bool,
                    }}
                "#
                ),
                true,
            );
            assert!(
                err.contains(word) && err.contains("not on the root"),
                "unhelpful message for {word}: {err}"
            );
        }

        // On a command, which is where they belong, both are accepted.
        let parsed = cli(r#"
            #[usage(mount = "ex tasks --usage", restart_token = ":::")]
            struct Run {
                #[usage(long)]
                dry_run: bool,
            }
        "#)
        .expect("parses");
        assert!(parsed
            .check_position(
                &syn::Ident::new("Run", proc_macro2::Span::call_site()),
                false
            )
            .is_ok());
    }

    #[test]
    fn value_enum_and_choices_are_the_same_list_twice() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, value_enum, choices("a", "b"))]
                shell: Option<Shell>,
            }
        "#,
        );
        assert!(err.contains("one or the other"), "unhelpful message: {err}");

        // And a switch has no value for a word to be.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, value_enum)]
                force: bool,
            }
        "#,
        );
        assert!(err.contains("takes no value"), "unhelpful message: {err}");
    }

    #[test]
    fn a_group_is_declared_once_and_joined_by_at_least_two_arguments() {
        // Two declarations would be read first-match-wins, so the second one's
        // properties would be silently dropped — a `required` written and not enforced.
        let err = rejection(
            r#"
            #[usage(group("input", required))]
            #[usage(group("input", multiple))]
            struct Ex {
                #[usage(long, group = "input")]
                file: Option<String>,
                #[usage(long, group = "input")]
                url: Option<String>,
            }
        "#,
        );
        assert!(err.contains("declared twice"), "unhelpful message: {err}");

        // A group of one is a statement about that argument.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, group = "input")]
                file: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("one argument in it"),
            "unhelpful message: {err}"
        );

        // And a declaration nothing joins holds for nothing.
        let err = rejection(
            r#"
            #[usage(group("input", required))]
            struct Ex {
                #[usage(long)]
                file: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("no field is in it"),
            "unhelpful message: {err}"
        );

        cli(r#"
            struct Ex {
                #[usage(group = "input")]
                target: Option<String>,
                #[usage(long, group = "input")]
                url: Option<String>,
            }
        "#)
        .expect("a positional can join a group");

        // A group with no name answers to nothing, whichever way it is written.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long, group = "")]
                file: Option<String>,
                #[usage(long, group = "")]
                url: Option<String>,
            }
        "#,
        );
        assert!(err.contains("no name"), "unhelpful message: {err}");
    }

    #[test]
    fn an_alias_cannot_name_a_sibling() {
        // The parser takes the first table entry that matches, so a name claimed twice
        // leaves one command unreachable — silently, which is the part worth refusing.
        let err = enum_rejection(
            r#"
            enum Commands {
                Install(Install),
                #[usage(alias = "install")]
                Add(Add),
            }
        "#,
        );
        assert!(
            err.contains("names two of these"),
            "unhelpful message: {err}"
        );

        // Including two aliases that collide with each other rather than with a name.
        let err = enum_rejection(
            r#"
            enum Commands {
                #[usage(alias = "rm")]
                Remove(Remove),
                #[usage(alias_hidden = "rm")]
                Delete(Delete),
            }
        "#,
        );
        assert!(
            err.contains("names two of these"),
            "unhelpful message: {err}"
        );

        // Distinct names and aliases are fine.
        subcommands(
            r#"
            enum Commands {
                #[usage(alias = "i", alias_hidden = "add")]
                Install(Install),
                #[usage(alias("rm", "uninstall"))]
                Remove(Remove),
            }
        "#,
        )
        .expect("distinct aliases should compile");
    }

    #[test]
    fn a_bounded_variadic_lets_an_argument_follow_it() {
        // The bound is what stops it, so what comes after is reachable — the rule the
        // grammar now states and clap's `num_args` has always had.
        cli(r#"
            struct Ex {
                #[usage(arg, var_max = 2)]
                first: Vec<String>,
                #[usage(arg)]
                rest: Option<String>,
            }
        "#)
        .expect("a bounded variadic should allow an argument after it");

        // Unbounded, and the argument after it is unreachable.
        let err = rejection(
            r#"
            struct Ex {
                #[usage(arg)]
                first: Vec<String>,
                #[usage(arg)]
                rest: Option<String>,
            }
        "#,
        );
        assert!(
            err.contains("unbounded variadic"),
            "unhelpful message: {err}"
        );

        // And a bounded variadic behind the separator does not spend it either.
        cli(r#"
            struct Ex {
                #[usage(arg, double_dash = "required", var_max = 1)]
                first: Vec<String>,
                #[usage(arg)]
                rest: Option<String>,
            }
        "#)
        .expect("a bounded variadic behind the separator should allow one after it");
    }

    #[test]
    fn a_positional_can_declare_a_conflict() {
        cli(r#"
            struct Ex {
                #[usage(long)]
                force: bool,
                #[usage(conflicts = "--force")]
                file: String,
            }
        "#)
        .expect("a positional conflict is representable in the spec");
    }

    #[test]
    fn overrides_needs_a_flag_to_hold_between_too() {
        let err = rejection(
            r#"
            struct Ex {
                #[usage(long)]
                force: bool,
                #[usage(overrides = "--force")]
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
    fn overrides_may_cross_a_flatten_boundary() {
        cli(r#"
            struct Ex {
                #[usage(long, overrides = "--nested")]
                force: bool,
                #[usage(flatten)]
                shared: Shared,
            }
        "#)
        .expect("the composed partial resolves the target while binding");
    }

    #[test]
    fn next_help_heading_defaults_each_direct_field() {
        let cli = cli(r#"
            #[command(next_help_heading = "Network")]
            struct Ex {
                #[arg(long)]
                registry: Option<String>,
                #[arg(long, help_heading = "Authentication")]
                token: Option<String>,
            }
        "#)
        .unwrap();
        assert_eq!(cli.fields[0].help_heading.as_deref(), Some("Network"));
        assert_eq!(
            cli.fields[1].help_heading.as_deref(),
            Some("Authentication")
        );
    }

    #[test]
    fn command_casing_applies_to_inferred_names_and_bare_env() {
        let cli = cli(r#"
            #[command(rename_all = "camelCase", rename_all_env = "lowercase")]
            struct Ex {
                #[arg(long, env)]
                api_token: Option<String>,
                #[arg(long = "fixed", env = "FIXED_ENV")]
                explicit_name: Option<String>,
            }
        "#)
        .unwrap();

        assert_eq!(cli.fields[0].name, "apiToken");
        assert_eq!(cli.fields[0].env.as_deref(), Some("apitoken"));
        let Kind::Flag { longs, .. } = &cli.fields[0].kind else {
            panic!("long should make this a flag");
        };
        assert_eq!(longs, &["apiToken"]);

        let Kind::Flag { longs, .. } = &cli.fields[1].kind else {
            panic!("long should make this a flag");
        };
        assert_eq!(longs, &["fixed"]);
        assert_eq!(cli.fields[1].env.as_deref(), Some("FIXED_ENV"));
    }

    #[test]
    fn subcommand_container_casing_applies_to_inferred_variant_names() {
        let commands = subcommands(
            r#"
            #[command(rename_all = "SCREAMING_SNAKE_CASE")]
            enum Commands {
                ApiServer(ApiServer),
                #[command(name = "fixed")]
                Explicit(Explicit),
            }
        "#,
        )
        .unwrap();

        assert_eq!(commands.variants[0].name, "API_SERVER");
        assert_eq!(commands.variants[1].name, "fixed");
    }

    #[test]
    fn verbatim_doc_comments_preserve_paragraph_shape() {
        let cli = cli(r#"
            #[command(verbatim_doc_comment)]
            /// First line.
            /// Second line.
            ///
            ///     indented example
            struct Ex {}
        "#)
        .unwrap();

        assert_eq!(cli.about.as_deref(), Some("First line.\nSecond line."));
        assert_eq!(
            cli.long_about.as_deref(),
            Some("First line.\nSecond line.\n\n    indented example")
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
