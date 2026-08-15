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
    /// From the struct's doc comment: first paragraph, and the whole thing.
    pub about: Option<String>,
    pub long_about: Option<String>,
    /// Whether a flag-like token that names no flag is a value or an error. Unset
    /// means the spec's default, which is `value`.
    pub unknown_flags: Option<String>,
    /// The command a bare invocation means: `mise build` is `mise run build`.
    ///
    /// Only the root has one, and it is what mise sets by hand on the emitted spec today.
    pub default_subcommand: Option<String>,
    /// Declared descriptions, for the case a doc comment cannot express: a long form that does
    /// not contain the short one.
    pub about_attr: Option<String>,
    pub long_about_attr: Option<String>,
    /// Text around the rest of the help page. mise puts an Examples section in
    /// `after_long_help` on 115 commands, and a page without it is missing what a reader came
    /// for. Nothing derives these from the code, so they are declared.
    pub before_help: Option<String>,
    pub before_long_help: Option<String>,
    pub after_help: Option<String>,
    pub after_long_help: Option<String>,
    /// The word that starts another invocation of the same command: mise's `:::`.
    pub restart_token: Option<String>,
    /// A command to run for subcommands discovered at completion time.
    ///
    /// Carried into the spec and nowhere else. The parser never runs it: a mount costs a
    /// subprocess, and completions are the cold path where that is affordable.
    pub mount: Option<String>,
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
    pub default: Option<String>,
    pub help_heading: Option<String>,
    /// Whether a collecting argument needs at least one value.
    ///
    /// Required-ness is normally the type's to say: a bare `String` has nowhere to put
    /// "absent" and an `Option` does. A `Vec` has neither shape, so `<TARGET>…` — a spec's
    /// way of saying "one or more" — could not be declared at all, and came back as
    /// `[TARGET]…`. This is the one place it has to be stated rather than inferred.
    pub required_collection: bool,
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
    pub var_min: Option<usize>,
    pub var_max: Option<usize>,
    /// Flags this one displaces. Applied while parsing rather than after it: the
    /// question is which of them came last, so the answer is decided by the token
    /// that arrives, not by the state it leaves behind.
    pub overrides: Vec<String>,
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
            completion: false,
            settings: false,
            version: None,
            about,
            long_about,
            unknown_flags: None,
            default_subcommand: None,
            about_attr: None,
            long_about_attr: None,
            before_help: None,
            before_long_help: None,
            after_help: None,
            after_long_help: None,
            restart_token: None,
            mount: None,
            fields: Vec::new(),
        };

        for attr in attrs(&input.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => cli.name = string_value(&meta)?,
                    "bin" => cli.bin = Some(string_value(&meta)?),
                    // Through the same helper as `global` and `var`, so `completion = false`
                    // means false rather than being read as the bare word with something
                    // decorative after it.
                    "completion" => cli.completion = flag_value(&meta)?,
                    "settings" => cli.settings = flag_value(&meta)?,
                    "version" => cli.version = Some(string_value(&meta)?),
                    // A doc comment's long form always contains its short one — the short form
                    // *is* the comment's first paragraph. A spec keeps `about` and `about_long`
                    // independent, and mise's differ entirely: "Dev tools, env vars, and tasks
                    // in one CLI" against "mise prepares your development environment before
                    // each command runs." There is no comment that says both, so they can be
                    // declared.
                    "about" => cli.about_attr = Some(string_value(&meta)?),
                    "long_about" => cli.long_about_attr = Some(string_value(&meta)?),
                    "before_help" => cli.before_help = Some(string_value(&meta)?),
                    "before_long_help" => cli.before_long_help = Some(string_value(&meta)?),
                    "after_help" => cli.after_help = Some(string_value(&meta)?),
                    "after_long_help" => cli.after_long_help = Some(string_value(&meta)?),
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
                    "restart_token" => cli.restart_token = Some(string_value(&meta)?),
                    "mount" => cli.mount = Some(string_value(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a struct; usage::Cli takes \
                                 `name`, `bin`, `version`, `unknown_flags`, \
                                 `default_subcommand`, `restart_token`, and `mount` here, \
                                 and the description comes from the doc comment"
                            ),
                        ));
                    }
                }
            }
        }

        // Declared descriptions win over the comment, which is the point of declaring them.
        if let Some(about) = cli.about_attr.take() {
            cli.about = Some(about);
        }
        if let Some(long) = cli.long_about_attr.take() {
            cli.long_about = Some(long);
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
            // The completion command answers for the whole CLI, so it is declared where the
            // whole CLI is. Accepted silently on an `Args`, it generated nothing and said
            // nothing, which reads as a CLI that has completions and does not.
            if self.completion {
                return Err(syn::Error::new_spanned(
                    ident,
                    "`completion` belongs on the root, where `#[derive(Cli)]` is: the hidden \
                     command it adds answers for the whole program, not for one of its commands",
                ));
            }
            // A spec declares one `default_subcommand`, at the top.
            if self.default_subcommand.is_some() {
                return Err(syn::Error::new_spanned(
                    ident,
                    "`default_subcommand` belongs on the root, where `#[derive(Cli)]` is: a \
                     spec declares one for the whole program, not one per command",
                ));
            }
            return Ok(());
        }

        // `mount` and `restart_token` are written on a `cmd` node, and the root is not one.
        // Verified against usage-lib, which rejects a spec that puts either at the top.
        for (present, what) in [
            (self.mount.is_some(), "mount"),
            (self.restart_token.is_some(), "restart_token"),
        ] {
            if present {
                return Err(syn::Error::new_spanned(
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
            return Err(syn::Error::new_spanned(
                ident,
                "`default_subcommand` names the command a bare invocation means, and this \
                 one has no subcommands to name",
            ));
        }
        Ok(())
    }

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
        // A variadic that only takes what follows a `--`, which is the end of the line in
        // both senses: it holds the remaining words and the separator cannot come twice.
        let mut spent_separator: Option<Span> = None;
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
                Kind::Arg {
                    double_dash_required,
                } => {
                    // A variadic takes every remaining word, so anything after it can
                    // never be filled — with two exceptions, both of which are something
                    // that stops the variadic. An argument only fillable after a `--`,
                    // because the separator ends the collecting; and any argument at all
                    // when the variadic is *bounded*, because it hands over the words past
                    // its bound. mise relies on the first on `run`, `exec` and `git`.
                    if let Some(first) = variadic_arg.filter(|_| !*double_dash_required) {
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
                        if *double_dash_required {
                            spent_separator = Some(field.span);
                        } else {
                            variadic_arg = Some(field.span);
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

        // Every relationship names a flag that exists. Resolving these at compile time
        // is the advantage of declaring them in code: a spec written by hand can only
        // find a typo'd selector at parse time, or never, since a selector naming
        // nothing quietly holds no relationship at all.
        for field in &self.fields {
            for (option, selectors) in [
                ("overrides", &field.overrides),
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
            kind: Kind::Flatten {
                ty: field.ty.clone(),
            },
            // A flattened field holds declarations, not a value, so none of what describes a
            // value applies — the same as a subcommand field.
            shape: Shape::Bool,
            value_ty: None,
            optional_collection: false,
            help: None,
            long_help: None,
            env: None,
            setting: None,
            default: None,
            help_heading: None,
            value_name: None,
            required_collection: false,
            choices: Vec::new(),
            value_enum: false,
            var_min: None,
            var_max: None,
            overrides: Vec::new(),
            conflicts: Vec::new(),
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
            value_ty: None,
            optional_collection: false,
            help: None,
            long_help: None,
            env: None,
            setting: None,
            default: None,
            help_heading: None,
            value_name: None,
            required_collection: false,
            choices: Vec::new(),
            value_enum: false,
            var_min: None,
            var_max: None,
            overrides: Vec::new(),
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
        if let Some(flattened) = Self::flatten(field, &ident, span)? {
            return Ok(flattened);
        }

        let mut name = to_kebab(&ident.to_string());
        let mut name_given = false;
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
        let mut setting = None;
        let mut default = None;
        let mut help_heading = None;
        let mut value_name = None;
        let mut required_collection = false;
        let mut help_attr: Option<String> = None;
        let mut long_help_attr: Option<String> = None;
        let mut hide = false;
        let mut is_arg = false;
        let mut choices: Vec<String> = Vec::new();
        let mut value_enum = false;
        let mut var_min: Option<usize> = None;
        let mut var_max: Option<usize> = None;
        let mut overrides: Vec<String> = Vec::new();
        let mut conflicts: Vec<String> = Vec::new();
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
                    // The *setting* this flag sets, which is a different thing from the flag's name
                    // and from the environment variable: `--jobs`, `HK_JOBS` and `jobs` are three
                    // spellings of one value, and only the last is what a config file calls it.
                    "setting" => setting = Some(string_value(&meta)?),
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
                    "overrides" => overrides = selectors(&meta)?,
                    "conflicts" => conflicts = selectors(&meta)?,
                    "required_if" => required_if = selectors(&meta)?,
                    "required_unless" => required_unless = selectors(&meta)?,
                    "value_enum" => value_enum = flag_value(&meta)?,
                    "var_min" => var_min = Some(int_value(&meta)?),
                    "var_max" => var_max = Some(int_value(&meta)?),
                    "default" => default = Some(string_value(&meta)?),
                    "help_heading" => help_heading = Some(string_value(&meta)?),
                    "value_name" => value_name = Some(string_value(&meta)?),
                    // Help text a doc comment cannot carry. A comment's first paragraph is
                    // read the way Rust reads one — line breaks inside it are spaces — so
                    // help whose breaks are meant literally has to be given directly.
                    "help" => help_attr = Some(string_value(&meta)?),
                    "long_help" => long_help_attr = Some(string_value(&meta)?),
                    "required" => required_collection = flag_value(&meta)?,
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
                                 `var_min`, `var_max`, `value_enum`, `overrides`, \
                                 `conflicts`, `required_if`, \
                                 `required_unless`, `help_heading`, `value_name`, \
                                 `required`, and `double_dash`"
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
            ("overrides", &overrides),
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
                value_name = Some(name.clone());
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

        Ok(Field {
            ident,
            ty: field.ty.clone(),
            name,
            kind,
            shape,
            value_ty,
            optional_collection,
            help,
            long_help,
            env,
            setting,
            default,
            help_heading,
            value_name,
            required_collection,
            choices,
            value_enum,
            var_min,
            var_max,
            overrides,
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
                // Only the one space `///` conventionally adds, and trailing space. Trimming
                // each line outright flattened every indented example in a CLI's help — and
                // mise's help is full of them, since an indented block is how a spec shows a
                // command to type.
                let raw = s.value();
                lines.push(raw.strip_prefix(' ').unwrap_or(&raw).trim_end().to_string());
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
    /// Whether the command is kept out of help and completions.
    ///
    /// A spec says `hide=#true` on a `cmd`; mise hides eight commands that way, `asdf` and
    /// `dotfiles` among them. Without this the derive could declare the command but not that
    /// it is unadvertised, so help listed things a user was not meant to be offered.
    pub hide: bool,
    /// The command name, which is the variant name in kebab-case unless `name`
    /// says otherwise.
    pub name: String,
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
    /// Other names this command answers to.
    ///
    /// Kept apart from [`hidden_aliases`](Self::hidden_aliases) only for the spec: the
    /// parser matches both, and the difference is whether help and completions mention
    /// them. mise has 67 of the first and 24 of the second.
    pub aliases: Vec<String>,
    pub hidden_aliases: Vec<String>,
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

        // Every name a command answers to, its aliases included: the parser takes the
        // first table entry that matches, so a name claimed twice means one of the two
        // commands can never be reached and nothing would have said so.
        let mut seen: Vec<(&str, Span)> = Vec::new();
        for variant in &variants {
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
        let mut aliases: Vec<String> = Vec::new();
        let mut hidden_aliases: Vec<String> = Vec::new();
        let mut hide = false;
        let mut help_attr: Option<String> = None;
        let mut long_help_attr: Option<String> = None;

        for attr in attrs(&variant.attrs) {
            for meta in nested(attr)? {
                let path = meta.path().clone();
                match ident_of(&path).as_str() {
                    "name" => name = strip_dashes(&string_value(&meta)?),
                    // One as a value or several as a list, as the relationship options do.
                    "alias" => aliases.extend(selectors(&meta)?),
                    "alias_hidden" => hidden_aliases.extend(selectors(&meta)?),
                    "hide" => hide = flag_value(&meta)?,
                    // As on a field: a comment's paragraph is flowed, so text whose line
                    // breaks matter is declared instead.
                    "help" => help_attr = Some(string_value(&meta)?),
                    "long_help" => long_help_attr = Some(string_value(&meta)?),
                    other => {
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "unknown option `{other}` on a variant; a subcommand \
                                 variant takes `name`, `alias` and `alias_hidden` here, \
                                 and its description comes from the doc comment"
                            ),
                        ));
                    }
                }
            }
        }
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

        // One unnamed field, holding the struct that declares the command's flags
        // and arguments. A variant with no fields would have nothing to parse into,
        // and named fields would make the variant itself the struct — which is a
        // second way to say the same thing.
        let held = match &variant.fields {
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => unnamed.unnamed[0].ty.clone(),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "a subcommand variant wraps exactly one struct, as in \
                     `Install(Install)` or `Install(Box<Install>)`: that struct is where \
                     its flags and arguments are declared",
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

        let (help, long_help) = (help_attr.or(help), long_help_attr.or(long_help));
        Ok(Variant {
            ident: variant.ident.clone(),
            hide,
            name,
            ty,
            boxed,
            aliases,
            hidden_aliases,
            help,
            long_help,
        })
    }
}

/// An enum whose variants are the words a value may be.
pub struct ValueEnum {
    pub ident: syn::Ident,
    /// Each variant, and the word it answers to.
    pub variants: Vec<(syn::Ident, String)>,
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

        let mut variants: Vec<(syn::Ident, String)> = Vec::new();
        for variant in &data.variants {
            if !matches!(variant.fields, Fields::Unit) {
                return Err(syn::Error::new_spanned(
                    &variant.fields,
                    "a value is one word, so each variant is a bare name: a variant holding \
                     fields would have nothing to build them from",
                ));
            }
            // A variant that may not exist cannot be listed. `CHOICES` is a `const` array and
            // an array literal takes no attributes on its elements, so a `cfg`-ed-out
            // variant would either leave a word in the list that nothing answers to, or an
            // arm referring to a variant that is not there. Refused rather than
            // miscompiled; `cfg` the whole enum, or keep the words and map them yourself.
            if let Some(cfg) = variant
                .attrs
                .iter()
                .find(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
            {
                return Err(syn::Error::new_spanned(
                    cfg,
                    "a value's variants are a `const` list of words, which cannot have holes \
                     in it: `cfg` the whole enum instead",
                ));
            }
            let mut name = to_kebab(&variant.ident.to_string());
            for attr in attrs(&variant.attrs) {
                for meta in nested(attr)? {
                    let path = meta.path().clone();
                    match ident_of(&path).as_str() {
                        "name" => name = string_value(&meta)?,
                        other => {
                            return Err(syn::Error::new_spanned(
                                path,
                                format!(
                                    "unknown option `{other}` on a value; a variant takes \
                                     `name` here"
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
            if let Some((first, _)) = variants.iter().find(|(_, n)| *n == name) {
                return Err(dup(
                    variant.ident.span(),
                    first.span(),
                    &format!("`{name}` names two of these values"),
                ));
            }
            variants.push((variant.ident.clone(), name));
        }
        if variants.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "an enum with no variants accepts no value at all",
            ));
        }

        Ok(ValueEnum {
            ident: input.ident.clone(),
            variants,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Subcommands, ValueEnum};

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
                .map(|(_, w)| w.as_str())
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
    fn a_conditional_value_is_refused_rather_than_miscompiled() {
        // The word list is a `const` array, so a variant that may not exist would leave
        // either a word nothing answers to or an arm naming a variant that is not there.
        let err = match value_enum(
            r#"
            enum Shell {
                Bash,
                #[cfg(windows)]
                PowerShell,
            }
        "#,
        ) {
            Ok(_) => panic!("should not have compiled"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("cannot have holes"),
            "unhelpful message: {err}"
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
