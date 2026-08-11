//! Turning a [`Cli`] into the code that parses for it.
//!
//! Three things are emitted, and the split is the whole point of the design:
//!
//! - `static` parse tables, which is all a successful parse reads.
//! - `static` metadata, which spec emission and help reading read and a parse
//!   never touches.
//! - a `parse` function that is a `match` on table indices, assigning straight
//!   into the struct's fields. No map, no intermediate value, nothing allocated
//!   that does not end up in the result.
//!
//! Both trees are emitted into one hidden module per type so that a CLI's tables
//! do not collide with anything, and so `cargo expand` shows them together.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::model::{Cli, Field, Kind, Shape, Subcommands};

pub fn emit(cli: &Cli) -> TokenStream {
    let ident = &cli.ident;
    let module = format_ident!("__usage_{}", ident.to_string().to_lowercase());

    let flags: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|f| matches!(f.kind, Kind::Flag { .. }))
        .collect();
    let args: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|f| matches!(f.kind, Kind::Arg { .. }))
        .collect();

    // Resolved here rather than at parse time: the tables hold the effective value,
    // and with one command per struct there is nothing above it to inherit from yet.
    let unknown_flags = match cli.unknown_flags.as_deref() {
        Some("error") => quote!(::usage_argv::UnknownFlags::Error),
        _ => quote!(::usage_argv::UnknownFlags::Value),
    };

    let base = key_base(&ident.to_string());
    let root_key = base | KIND_COMMAND;
    let flag_tables = flags
        .iter()
        .enumerate()
        .map(|(i, f)| flag_table(i, f, base));
    let arg_tables = args.iter().enumerate().map(|(i, f)| arg_table(i, f, base));
    let flag_metas = flags.iter().enumerate().map(|(i, f)| flag_meta(i, f));
    let arg_metas = args.iter().enumerate().map(|(i, f)| arg_meta(i, f));

    let flag_refs = (0..flags.len()).map(|i| {
        let name = format_ident!("FLAG_{i}");
        quote!(&#name)
    });
    let arg_refs = (0..args.len()).map(|i| {
        let name = format_ident!("ARG_{i}");
        quote!(&#name)
    });
    let flag_meta_refs = (0..flags.len()).map(|i| format_ident!("FLAG_META_{i}"));
    let arg_meta_refs = (0..args.len()).map(|i| format_ident!("ARG_META_{i}"));

    let name = &cli.name;
    let bin = option_str(cli.bin.as_deref());
    let version = option_str(cli.version.as_deref());
    let about = option_str(cli.about.as_deref());
    let long_about = option_str(cli.long_about.as_deref());

    // The same wiring a nested command uses: the root differs only in how it is
    // entered, so it does not get its own copy.
    let parts = subcommand_parts(cli);
    let sub_commands = parts
        .as_ref()
        .map(|p| p.commands.clone())
        .unwrap_or_default();
    let sub_metas = parts.as_ref().map(|p| p.metas.clone()).unwrap_or_default();
    let sub_build = parts.as_ref().map(|p| p.build.clone()).unwrap_or_default();

    let partial = partial_struct(cli);
    let defaults = partial_defaults(cli);
    let apply = apply_fn(cli, base);
    let post = post_binding(cli);
    // `field: local` rather than the shorthand, because the locals are prefixed:
    // a field called `text` or `parser` would otherwise collide with something the
    // generated code needs.
    let field_finals = cli
        .fields
        .iter()
        .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
        .map(|f| {
            let ident = &f.ident;
            quote!(#ident: partial.#ident)
        });

    quote! {
        #[doc(hidden)]
        #[allow(
            non_upper_case_globals,
            non_snake_case,
            unused_imports,
            // Fires when a metadata struct happens to be fully specified. `..EMPTY`
            // is kept on purpose: it is what lets usage-argv gain a metadata field
            // without breaking every crate that derives.
            clippy::needless_update
        )]
        mod #module {
            use ::usage_argv::spec::{ArgMeta, CommandMeta, FlagMeta, Spec};
            use ::usage_argv::{Arg, Command, DoubleDash, Flag};

            #(#flag_tables)*
            #(#arg_tables)*

            pub static ROOT: Command = Command {
                unknown_flags: #unknown_flags,
                name: #name,
                key: #root_key,
                flags: &[#(#flag_refs),*],
                args: &[#(#arg_refs),*],
                #sub_commands
                ..Command::EMPTY
            };

            #(#flag_metas)*
            #(#arg_metas)*

            pub static ROOT_META: CommandMeta = CommandMeta {
                cmd: &ROOT,
                about: #about,
                long_about: #long_about,
                flags: &[#(#flag_meta_refs),*],
                args: &[#(#arg_meta_refs),*],
                #sub_metas
                ..CommandMeta::EMPTY
            };

            // Values arrive as the bytes that were on the command line. This version
            // holds them as `String`, and converts lossily, which is what mise
            // already does with its own argv. Rejecting a non-UTF-8 value needs an
            // error type for value conversion, and that arrives with typed fields.
            pub fn __usage_text(value: &[u8]) -> ::std::string::String {
                ::std::string::String::from_utf8_lossy(value).into_owned()
            }

            pub fn __usage_value_text(
                value: ::std::option::Option<&[u8]>,
            ) -> ::std::string::String {
                value.map(__usage_text).unwrap_or_default()
            }

            #partial
            #apply

            /// Everything decided after the last token.
            ///
            /// In the module rather than beside the parse, so every reference it makes
            /// to the user's own types sits at one consistent scope — the root and a
            /// nested command generate the same code here.
            pub fn check<'t, 'v>(
                partial: &mut Partial,
            ) -> ::std::result::Result<(), ::usage_argv::Error<'t, 'v>> {
                #post
                ::std::result::Result::Ok(())
            }

            pub static SPEC: Spec = Spec {
                name: #name,
                bin: #bin,
                version: #version,
                about: #about,
                long_about: #long_about,
                default_subcommand: None,
                root: &ROOT_META,
            };
        }

        impl #ident {
            /// The parse tables for this CLI.
            ///
            /// `static`, so reaching them costs nothing: there is no command tree
            /// to build before a parse can start.
            pub fn command() -> &'static ::usage_argv::Command<'static> {
                &#module::ROOT
            }

            /// This CLI's spec, for emitting, documenting, or completing.
            pub fn spec() -> &'static ::usage_argv::spec::Spec<'static> {
                &#module::SPEC
            }

            /// This CLI's spec as KDL, which is what `usage g markdown|manpage`
            /// and the completion generators read.
            pub fn to_kdl() -> ::std::string::String {
                #module::SPEC.to_kdl()
            }

            /// Parse a command line, excluding the program name.
            pub fn parse_from<'v>(
                argv: &'v [&'v ::std::ffi::OsStr],
            ) -> ::std::result::Result<Self, ::usage_argv::Error<'static, 'v>> {
                use ::usage_argv::Event;

                use #module::Partial;

                #defaults

                let mut __usage_parser = ::usage_argv::Parser::new(Self::command(), argv);
                while let ::std::option::Option::Some(__usage_event) =
                    __usage_parser.next_event()
                {
                    // `apply` handles this command's own fields and routes anything
                    // else into its subcommands, which is why a nested command needs
                    // nothing extra here.
                    #module::apply(&mut partial, &__usage_event?);
                }

                #module::check(&mut partial)?;

                ::std::result::Result::Ok(Self {
                    #sub_build
                    #(#field_finals),*
                })
            }

            /// Parse the process's own arguments.
            pub fn parse() -> ::std::result::Result<Self, ::std::string::String> {
                let __usage_raw: ::std::vec::Vec<::std::ffi::OsString> =
                    ::std::env::args_os().skip(1).collect();
                let __usage_argv: ::std::vec::Vec<&::std::ffi::OsStr> =
                    __usage_raw.iter().map(|a| a.as_os_str()).collect();
                // The error borrows argv, so it cannot outlive this function;
                // rendering it here is what makes the signature usable. Better
                // diagnostics are a separate piece of work.
                Self::parse_from(&__usage_argv).map_err(|e| ::std::format!("{e:?}"))
            }
        }
    }
}

fn flag_table(i: usize, field: &Field, base: u64) -> TokenStream {
    let name = format_ident!("FLAG_{i}");
    let key = base | KIND_FLAG | i as u64;
    let field_name = &field.name;
    let Kind::Flag {
        longs,
        shorts,
        negate,
        global,
        variadic,
    } = &field.kind
    else {
        unreachable!("filtered by the caller");
    };
    let shorts: Vec<u8> = shorts.iter().map(|c| *c as u8).collect();
    let negate = option_str(negate.as_deref());
    let takes_value = field.takes_value();

    quote! {
        pub static #name: Flag = Flag {
            key: #key,
            name: #field_name,
            longs: &[#(#longs),*],
            shorts: &[#(#shorts),*],
            negate: #negate,
            takes_value: #takes_value,
            variadic: #variadic,
            global: #global,
        };
    }
}

fn arg_table(i: usize, field: &Field, base: u64) -> TokenStream {
    let name = format_ident!("ARG_{i}");
    let key = base | KIND_ARG | i as u64;
    let field_name = &field.name;
    let var = field.shape == Shape::Many;
    let Kind::Arg {
        double_dash_required,
    } = &field.kind
    else {
        unreachable!("filtered by the caller");
    };
    let double_dash = if *double_dash_required {
        quote!(DoubleDash::Required)
    } else {
        quote!(DoubleDash::Optional)
    };

    quote! {
        pub static #name: Arg = Arg {
            key: #key,
            name: #field_name,
            var: #var,
            double_dash: #double_dash,
        };
    }
}

fn flag_meta(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("FLAG_META_{i}");
    let table = format_ident!("FLAG_{i}");
    let help = option_str(field.help.as_deref());
    let long_help = option_str(field.long_help.as_deref());
    let env = option_str(field.env.as_deref());
    let help_heading = option_str(field.help_heading.as_deref());
    let default = match field.default.as_deref() {
        Some(d) => quote!(&[#d]),
        None => quote!(&[]),
    };
    let hide = field.hide;
    let count = field.shape == Shape::Count;
    let repeatable = field.repeatable;
    // Same rule as an argument: a `String` has nowhere to put "absent". The runtime
    // check already enforced it; the spec has to say it too, or docs and completions
    // describe a different CLI from the one that runs.
    let required = field.shape == Shape::Required;
    let choices = choices_tokens(field);
    let (var_min, var_max) = bounds_tokens(field);

    quote! {
        pub static #name: FlagMeta = FlagMeta {
            flag: &#table,
            help: #help,
            long_help: #long_help,
            env: #env,
            default: #default,
            help_heading: #help_heading,
            hide: #hide,
            count: #count,
            repeatable: #repeatable,
            required: #required,
            choices: #choices,
            var_min: #var_min,
            var_max: #var_max,
            ..FlagMeta::EMPTY
        };
    }
}

fn arg_meta(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("ARG_META_{i}");
    let table = format_ident!("ARG_{i}");
    let help = option_str(field.help.as_deref());
    let long_help = option_str(field.long_help.as_deref());
    let env = option_str(field.env.as_deref());
    let help_heading = option_str(field.help_heading.as_deref());
    let default = match field.default.as_deref() {
        Some(d) => quote!(&[#d]),
        None => quote!(&[]),
    };
    let hide = field.hide;
    // `String` must be filled; `Option` and `Vec` need not be.
    let required = field.shape == Shape::Required;
    let choices = choices_tokens(field);
    let (var_min, var_max) = bounds_tokens(field);

    quote! {
        pub static #name: ArgMeta = ArgMeta {
            arg: &#table,
            help: #help,
            long_help: #long_help,
            env: #env,
            default: #default,
            help_heading: #help_heading,
            hide: #hide,
            required: #required,
            choices: #choices,
            var_min: #var_min,
            var_max: #var_max,
            ..ArgMeta::EMPTY
        };
    }
}

/// A field's declared choices, as the metadata holds them.
fn choices_tokens(field: &Field) -> TokenStream {
    let choices = &field.choices;
    quote!(&[#(#choices),*])
}

/// A field's declared bounds, as the metadata holds them.
fn bounds_tokens(field: &Field) -> (TokenStream, TokenStream) {
    let render = |bound: Option<usize>| match bound {
        Some(n) => quote!(::std::option::Option::Some(#n)),
        None => quote!(::std::option::Option::None),
    };
    (render(field.var_min), render(field.var_max))
}

/// Which kind of thing a key belongs to, in the bits above its index.
///
/// A command, a flag, and an argument each get their own space, so no two things in
/// one type can share a key even though each counts its own from zero. The parser
/// says which kind it bound, so this is not needed in order to *dispatch* — it is
/// needed so that "every key in the tree is distinct" is true, which is what makes a
/// collision between two types detectable at all.
const KIND_FLAG: u64 = 0;
const KIND_ARG: u64 = 1 << 30;
const KIND_COMMAND: u64 = 2 << 30;

/// The high half of every key a type's fields get.
///
/// Two macro expansions cannot see each other, so a shared counter is not available:
/// keys carry a hash of the type they came from instead. A parse dispatches on the
/// key, so two type names would have to collide in 32 bits to bind the wrong field,
/// and `Spec::to_kdl` asserts the tree has no duplicates.
fn key_base(type_name: &str) -> u64 {
    // FNV-1a, spelled out rather than taken from a `Hasher`, which is not guaranteed
    // to be stable between compilations — and these end up baked into generated code.
    let mut hash: u32 = 0x811c_9dc5;
    for byte in type_name.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash as u64) << 32
}

/// A user's type, named from inside a generated module.
///
/// The generated `mod` is a child of wherever the derive was written, and a name
/// from the parent scope is not in scope inside it — so a reference to the user's own
/// type has to say `super::`. An absolute path already resolves from anywhere and is
/// left alone.
fn in_module(ty: &syn::Type) -> TokenStream {
    let syn::Type::Path(path) = ty else {
        return quote!(#ty);
    };
    // Already absolute, or rooted at the crate: resolves the same from anywhere.
    if path.path.leading_colon.is_some() {
        return quote!(#ty);
    }
    let mut segments = path.path.segments.iter();
    match segments.next().map(|s| s.ident.to_string()).as_deref() {
        Some("crate") => quote!(#ty),
        // `self` and `super` are relative to where the user wrote them, which is one
        // level out from the generated module — so each shifts by one.
        Some("self") => {
            let rest = segments;
            quote!(super::#(#rest)::*)
        }
        Some("super") => {
            let rest = segments;
            quote!(super::super::#(#rest)::*)
        }
        // A relative path, which the generated module is one level below.
        _ => quote!(super::#ty),
    }
}

fn flag_arm(i: usize, field: &Field, base: u64) -> TokenStream {
    let key = base | KIND_FLAG | i as u64;
    let ident = &field.ident;
    let given = format_ident!("__given_{}", ident);
    let body = match field.shape {
        // `negated` is what distinguishes `--color` from `--no-color`.
        Shape::Bool => quote!(partial.#ident = !negated;),
        // Saturating, because a `u8` field given 256 occurrences would otherwise
        // panic in debug and wrap to zero in release.
        Shape::Count => quote!(partial.#ident = partial.#ident.saturating_add(1);),
        Shape::Optional => quote! {
            partial.#ident = ::std::option::Option::Some(__usage_value_text(value));
        },
        Shape::Required => quote!(partial.#ident = __usage_value_text(value);),
        Shape::Many => quote!(partial.#ident.push(__usage_value_text(value));),
    };
    quote! {
        #key => { #body partial.#given = true; true }
    }
}

fn arg_arm(i: usize, field: &Field, base: u64) -> TokenStream {
    let key = base | KIND_ARG | i as u64;
    let ident = &field.ident;
    let given = format_ident!("__given_{}", ident);
    let body = match field.shape {
        Shape::Many => quote!(partial.#ident.push(__usage_text(value));),
        Shape::Optional => quote! {
            partial.#ident = ::std::option::Option::Some(__usage_text(value));
        },
        _ => quote!(partial.#ident = __usage_text(value);),
    };
    quote! {
        #key => { #body partial.#given = true; true }
    }
}

fn option_str(value: Option<&str>) -> TokenStream {
    match value {
        Some(v) => quote!(::std::option::Option::Some(#v)),
        None => quote!(::std::option::Option::None),
    }
}

/// The struct that collects values while parsing.
///
/// One field per declared field, of the type that accumulates it: a `bool` for a
/// switch, a `Vec` for something repeatable, and so on. It exists so the same
/// generated `apply` serves a root command and a subcommand's arguments — a
/// subcommand cannot use locals in the root's parse function, because the root
/// cannot see its fields.
fn partial_struct(cli: &Cli) -> TokenStream {
    let sub = subcommand_parts(cli)
        .map(|p| p.partial_fields)
        .unwrap_or_default();
    let fields = cli.fields.iter().filter_map(|f| {
        if matches!(f.kind, Kind::Subcommand { .. }) {
            // Its values live in the enum's own partial.
            return None;
        }
        let ident = &f.ident;
        let ty = match f.shape {
            Shape::Bool => quote!(bool),
            Shape::Count => {
                let ty = &f.ty;
                quote!(#ty)
            }
            Shape::Optional => quote!(::std::option::Option<::std::string::String>),
            Shape::Required => quote!(::std::string::String),
            Shape::Many => quote!(::std::vec::Vec<::std::string::String>),
        };
        let given = format_ident!("__given_{}", ident);
        // Whether a token supplied this, as opposed to a default sitting in it: an
        // empty string is a value somebody typed, and `--jobs=` has to be able to
        // mean it.
        Some(quote!(pub #ident: #ty, pub #given: bool,))
    });

    // `Default` rather than a generated `new`: every accumulator type has one, and
    // a default that says "nothing was given" is what a partial starts as.
    // `Default` is not derived once a subcommand is in play: the nested partial has
    // its own starting values, which `start` puts in place.
    quote! {
        pub struct Partial {
            #(#fields)*
            #sub
        }
    }
}

/// The defaults a partial cannot express as `Default::default()`.
///
/// A declared `default` has to be in place before parsing starts, since nothing
/// later distinguishes "the default" from "what the user typed".
fn partial_defaults(cli: &Cli) -> TokenStream {
    let sub_starts = subcommand_parts(cli)
        .map(|p| p.partial_starts)
        .unwrap_or_default();
    let plain = cli.fields.iter().filter_map(|f| {
        if matches!(f.kind, Kind::Subcommand { .. }) {
            return None;
        }
        let ident = &f.ident;
        let given = format_ident!("__given_{}", ident);
        Some(quote! {
            #ident: ::std::default::Default::default(),
            #given: false,
        })
    });
    let assignments = cli.fields.iter().filter_map(|f| {
        let ident = &f.ident;
        let default = f.default.as_deref()?;
        Some(match f.shape {
            Shape::Bool => {
                let on = default == "true";
                quote!(partial.#ident = #on;)
            }
            Shape::Optional => {
                quote!(partial.#ident = ::std::option::Option::Some(#default.to_string());)
            }
            Shape::Required => quote!(partial.#ident = #default.to_string();),
            // Rejected in the model: a count starts at zero, and a default for a
            // collecting field is not applied yet.
            Shape::Count | Shape::Many => quote!(),
        })
    });
    quote! {
        let mut partial = Partial {
            #(#plain)*
            #sub_starts
        };
        #(#assignments)*
    }
}

/// Take one event and say whether it belonged to this command.
fn apply_fn(cli: &Cli, base: u64) -> TokenStream {
    let route = subcommand_parts(cli).map(|p| p.route).unwrap_or_default();
    let flags: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|f| matches!(f.kind, Kind::Flag { .. }))
        .collect();
    let args: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|f| matches!(f.kind, Kind::Arg { .. }))
        .collect();
    let flag_arms = flags.iter().enumerate().map(|(i, f)| flag_arm(i, f, base));
    let arg_arms = args.iter().enumerate().map(|(i, f)| arg_arm(i, f, base));

    quote! {
        pub fn apply(
            partial: &mut Partial,
            event: &::usage_argv::Event<'_, '_>,
        ) -> bool {
            use ::usage_argv::Event;
            #route
            // Each arm evaluates to whether it claimed the event, rather than
            // returning: a command with no flags of its own would otherwise have every
            // arm diverge, leaving an unreachable tail.
            match event {
                Event::Flag { flag, value, negated } => {
                    let (value, negated) = (*value, *negated);
                    let _ = (value, negated);
                    match flag.key {
                        #(#flag_arms)*
                        // Another command's flag, left for whoever owns it.
                        _ => false,
                    }
                }
                Event::Arg { arg, value } => {
                    let value = *value;
                    let _ = value;
                    match arg.key {
                        #(#arg_arms)*
                        _ => false,
                    }
                }
                // Descending is the caller's business: it is what decides which
                // command's fields the following events belong to.
                Event::Command(_) => false,
            }
        }
    }
}

/// What a command with subcommands needs, wherever it sits in the tree.
///
/// The root and a nested command differ only in how they are entered, so they share
/// this: the tables to splice, the state to carry, how an event is routed, and how
/// the field is finally built.
struct SubcommandParts {
    /// `subcommands:` for the `Command` table.
    commands: TokenStream,
    /// `subcommands:` for the `CommandMeta`.
    metas: TokenStream,
    /// Fields the partial needs to carry.
    partial_fields: TokenStream,
    /// Their starting values.
    partial_starts: TokenStream,
    /// Routing an event that this command did not claim.
    route: TokenStream,
    /// Checking whichever subcommand was selected.
    check: TokenStream,
    /// Building the field.
    build: TokenStream,
}

fn subcommand_parts(cli: &Cli) -> Option<SubcommandParts> {
    let (field, ty) = cli.fields.iter().find_map(|f| match &f.kind {
        Kind::Subcommand { ty, .. } => Some((f, ty)),
        _ => None,
    })?;
    let ident = &field.ident;
    let optional = matches!(&field.kind, Kind::Subcommand { optional: true, .. });
    let in_mod = in_module(ty);

    let selected = quote! {
        match partial.__usage_selected {
            ::std::option::Option::Some(__usage_key) => {
                <#ty as ::usage_argv::spec::Subcommands>::select(
                    partial.__usage_sub,
                    __usage_key,
                )?
            }
            ::std::option::Option::None => ::std::option::Option::None,
        }
    };
    let build = if optional {
        quote!(#ident: #selected,)
    } else {
        quote! {
            #ident: match #selected {
                ::std::option::Option::Some(__usage_cmd) => __usage_cmd,
                ::std::option::Option::None => {
                    return ::std::result::Result::Err(
                        ::usage_argv::Error::MissingSubcommand,
                    );
                }
            },
        }
    };

    Some(SubcommandParts {
        commands: quote!(subcommands: <#in_mod as ::usage_argv::spec::Subcommands>::COMMANDS,),
        metas: quote!(subcommands: <#in_mod as ::usage_argv::spec::Subcommands>::METAS,),
        partial_fields: quote! {
            pub __usage_sub: <#in_mod as ::usage_argv::spec::Subcommands>::Partial,
            /// Which of this command's subcommands was reached, if any.
            pub __usage_selected: ::std::option::Option<u64>,
        },
        partial_starts: quote! {
            __usage_sub: ::std::default::Default::default(),
            __usage_selected: ::std::option::Option::None,
        },
        // `route` and `check` are emitted inside the generated module, so they name
        // the user's enum through `super::`; `build` sits in the impl beside it and
        // names it directly.
        route: quote! {
            // A command word only counts as *this* command's if one of its own
            // subcommands answers to it: a deeper descent belongs to whoever owns
            // that command, and recording it here would make the wrong variant look
            // selected.
            if let ::usage_argv::Event::Command(__usage_cmd) = event {
                if <#in_mod as ::usage_argv::spec::Subcommands>::COMMANDS
                    .iter()
                    .any(|candidate| candidate.key == __usage_cmd.key)
                {
                    partial.__usage_selected = ::std::option::Option::Some(__usage_cmd.key);
                }
            }
            if <#in_mod as ::usage_argv::spec::Subcommands>::apply(
                &mut partial.__usage_sub,
                event,
            ) {
                return true;
            }
        },
        check: quote! {
            if let ::std::option::Option::Some(__usage_key) = partial.__usage_selected {
                <#in_mod as ::usage_argv::spec::Subcommands>::check(
                    &mut partial.__usage_sub,
                    __usage_key,
                )?;
            }
        },
        build,
    })
}

/// A subcommand's argument struct: tables, metadata, and the trait that lets a
/// parent reach them.
pub fn emit_args(cli: &Cli) -> TokenStream {
    let ident = &cli.ident;
    let module = format_ident!("__usage_args_{}", ident.to_string().to_lowercase());
    let base = key_base(&ident.to_string());
    let command_key = base | KIND_COMMAND;

    let flags: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|f| matches!(f.kind, Kind::Flag { .. }))
        .collect();
    let args: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|f| matches!(f.kind, Kind::Arg { .. }))
        .collect();

    let flag_tables = flags
        .iter()
        .enumerate()
        .map(|(i, f)| flag_table(i, f, base));
    let arg_tables = args.iter().enumerate().map(|(i, f)| arg_table(i, f, base));
    let flag_metas = flags.iter().enumerate().map(|(i, f)| flag_meta(i, f));
    let arg_metas = args.iter().enumerate().map(|(i, f)| arg_meta(i, f));
    let flag_refs = (0..flags.len()).map(|i| {
        let name = format_ident!("FLAG_{i}");
        quote!(&#name)
    });
    let arg_refs = (0..args.len()).map(|i| {
        let name = format_ident!("ARG_{i}");
        quote!(&#name)
    });
    let flag_meta_refs = (0..flags.len()).map(|i| format_ident!("FLAG_META_{i}"));
    let arg_meta_refs = (0..args.len()).map(|i| format_ident!("ARG_META_{i}"));

    let name = &cli.name;
    let about = option_str(cli.about.as_deref());
    let long_about = option_str(cli.long_about.as_deref());
    let partial = partial_struct(cli);
    let defaults = partial_defaults(cli);
    let apply = apply_fn(cli, base);
    let post = post_binding(cli);
    let parts = subcommand_parts(cli);
    let sub_commands = parts
        .as_ref()
        .map(|p| p.commands.clone())
        .unwrap_or_default();
    let sub_metas = parts.as_ref().map(|p| p.metas.clone()).unwrap_or_default();
    let sub_build = parts.as_ref().map(|p| p.build.clone()).unwrap_or_default();
    let field_finals = cli
        .fields
        .iter()
        .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
        .map(|f| {
            let ident = &f.ident;
            quote!(#ident: partial.#ident)
        });

    quote! {
        #[doc(hidden)]
        #[allow(
            non_upper_case_globals,
            non_snake_case,
            unused_imports,
            // Fires when a metadata struct happens to be fully specified. `..EMPTY`
            // is kept on purpose: it is what lets usage-argv gain a metadata field
            // without breaking every crate that derives.
            clippy::needless_update
        )]
        mod #module {
            use ::usage_argv::spec::{ArgMeta, CommandMeta, FlagMeta};
            use ::usage_argv::{Arg, Command, DoubleDash, Flag};

            #(#flag_tables)*
            #(#arg_tables)*

            pub static COMMAND: Command = Command {
                name: #name,
                key: #command_key,
                flags: &[#(#flag_refs),*],
                args: &[#(#arg_refs),*],
                #sub_commands
                ..Command::EMPTY
            };

            #(#flag_metas)*
            #(#arg_metas)*

            pub static COMMAND_META: CommandMeta = CommandMeta {
                cmd: &COMMAND,
                about: #about,
                long_about: #long_about,
                flags: &[#(#flag_meta_refs),*],
                args: &[#(#arg_meta_refs),*],
                #sub_metas
                ..CommandMeta::EMPTY
            };

            pub fn __usage_text(value: &[u8]) -> ::std::string::String {
                ::std::string::String::from_utf8_lossy(value).into_owned()
            }

            pub fn __usage_value_text(
                value: ::std::option::Option<&[u8]>,
            ) -> ::std::string::String {
                value.map(__usage_text).unwrap_or_default()
            }

            #partial
            #apply

            pub fn start() -> Partial {
                #defaults
                partial
            }

            /// Everything decided after the last token, for this command.
            ///
            /// Separate from `build` because only the *selected* command's
            /// requirements apply: a flag that `install` requires says nothing about
            /// an invocation that ran `run`.
            pub fn check<'t, 'v>(
                partial: &mut Partial,
            ) -> ::std::result::Result<(), ::usage_argv::Error<'t, 'v>> {
                #post
                ::std::result::Result::Ok(())
            }
        }

        impl ::usage_argv::spec::CommandArgs for #ident {
            type Partial = #module::Partial;

            const COMMAND: &'static ::usage_argv::Command<'static> = &#module::COMMAND;
            const META: &'static ::usage_argv::spec::CommandMeta<'static> =
                &#module::COMMAND_META;

            fn start() -> Self::Partial {
                #module::start()
            }

            fn apply(
                partial: &mut Self::Partial,
                event: &::usage_argv::Event<'_, '_>,
            ) -> bool {
                #module::apply(partial, event)
            }

            fn check<'t, 'v>(
                partial: &mut Self::Partial,
            ) -> ::std::result::Result<(), ::usage_argv::Error<'t, 'v>> {
                #module::check(partial)
            }

            fn build<'t, 'v>(
                partial: Self::Partial,
            ) -> ::std::result::Result<Self, ::usage_argv::Error<'t, 'v>> {
                ::std::result::Result::Ok(Self {
                    #sub_build
                    #(#field_finals),*
                })
            }
        }
    }
}

/// The enum a `subcommand` field holds: its variants' tables, and the trait a
/// parent uses to route events into them.
pub fn emit_subcommands(subs: &Subcommands) -> TokenStream {
    let ident = &subs.ident;
    let module = format_ident!("__usage_subs_{}", ident.to_string().to_lowercase());

    // One partial per variant, since a parse can only fill one but does not know
    // which until the word arrives.
    let partial_fields = subs.variants.iter().enumerate().map(|(i, v)| {
        let field = format_ident!("v{i}");
        let ty = in_module(&v.ty);
        quote!(pub #field: <#ty as ::usage_argv::spec::CommandArgs>::Partial,)
    });
    let partial_starts = subs.variants.iter().enumerate().map(|(i, v)| {
        let field = format_ident!("v{i}");
        let ty = in_module(&v.ty);
        quote!(#field: <#ty as ::usage_argv::spec::CommandArgs>::start(),)
    });
    let applies = subs.variants.iter().enumerate().map(|(i, v)| {
        let field = format_ident!("v{i}");
        let ty = &v.ty;
        quote! {
            if <#ty as ::usage_argv::spec::CommandArgs>::apply(&mut partial.#field, event) {
                return true;
            }
        }
    });
    // The enum is where the command set is declared, so the variant names the
    // command — its own kebab-case name, or whatever `name` says. Splicing the
    // struct's table unchanged would have used the *struct's* name instead, which
    // only looks right when the two happen to match.
    let command_overrides = subs.variants.iter().enumerate().map(|(i, v)| {
        let name = format_ident!("COMMAND_{i}");
        let ty = in_module(&v.ty);
        let cmd_name = &v.name;
        quote! {
            pub static #name: ::usage_argv::Command = ::usage_argv::Command {
                name: #cmd_name,
                ..*<#ty as ::usage_argv::spec::CommandArgs>::COMMAND
            };
        }
    });
    let commands = (0..subs.variants.len()).map(|i| {
        let name = format_ident!("COMMAND_{i}");
        quote!(&#module::#name)
    });
    // A doc comment on the variant wins over the struct's, since that is where a
    // reader of the enum expects to describe the command — and ignoring it would lose
    // the description without saying so. Overriding one field of the struct's
    // metadata is possible in a const, so the tables stay static.
    let meta_overrides = subs.variants.iter().enumerate().map(|(i, v)| {
        let name = format_ident!("META_{i}");
        let cmd = format_ident!("COMMAND_{i}");
        let ty = in_module(&v.ty);
        // A doc comment on the variant wins over the struct's, since that is where a
        // reader of the enum expects to describe the command. Absent one, the
        // struct's own description carries through.
        let (about, long_about) = match &v.help {
            Some(_) => (
                option_str(v.help.as_deref()),
                option_str(v.long_help.as_deref()),
            ),
            None => (
                quote!(<#ty as ::usage_argv::spec::CommandArgs>::META.about),
                quote!(<#ty as ::usage_argv::spec::CommandArgs>::META.long_about),
            ),
        };
        quote! {
            pub static #name: ::usage_argv::spec::CommandMeta =
                ::usage_argv::spec::CommandMeta {
                    cmd: &#cmd,
                    about: #about,
                    long_about: #long_about,
                    ..*<#ty as ::usage_argv::spec::CommandArgs>::META
                };
        }
    });
    let metas = (0..subs.variants.len()).map(|i| {
        let name = format_ident!("META_{i}");
        quote!(&#module::#name)
    });
    // Matched on the command's key rather than its name, so selecting a variant is
    // an integer comparison and cannot be confused by an alias.
    let checks = subs.variants.iter().enumerate().map(|(i, v)| {
        let field = format_ident!("v{i}");
        let ty = &v.ty;
        quote! {
            if key == <#ty as ::usage_argv::spec::CommandArgs>::COMMAND.key {
                return <#ty as ::usage_argv::spec::CommandArgs>::check(&mut partial.#field);
            }
        }
    });
    let selects = subs.variants.iter().enumerate().map(|(i, v)| {
        let field = format_ident!("v{i}");
        let variant = &v.ident;
        let ty = &v.ty;
        quote! {
            if key == <#ty as ::usage_argv::spec::CommandArgs>::COMMAND.key {
                return ::std::result::Result::Ok(::std::option::Option::Some(
                    #ident::#variant(
                        <#ty as ::usage_argv::spec::CommandArgs>::build(partial.#field)?,
                    ),
                ));
            }
        }
    });

    quote! {
        #[doc(hidden)]
        #[allow(
            non_upper_case_globals,
            non_snake_case,
            unused_imports,
            // Fires when a metadata struct happens to be fully specified. `..EMPTY`
            // is kept on purpose: it is what lets usage-argv gain a metadata field
            // without breaking every crate that derives.
            clippy::needless_update
        )]
        mod #module {
            pub struct Partial {
                #(#partial_fields)*
            }

            impl ::std::default::Default for Partial {
                fn default() -> Self {
                    Self { #(#partial_starts)* }
                }
            }

            #(#command_overrides)*
            #(#meta_overrides)*
        }

        impl ::usage_argv::spec::Subcommands for #ident {
            type Partial = #module::Partial;

            const COMMANDS: &'static [&'static ::usage_argv::Command<'static>] =
                &[#(#commands),*];
            const METAS: &'static [&'static ::usage_argv::spec::CommandMeta<'static>] =
                &[#(#metas),*];

            fn apply(
                partial: &mut Self::Partial,
                event: &::usage_argv::Event<'_, '_>,
            ) -> bool {
                #(#applies)*
                false
            }

            fn check<'t, 'v>(
                partial: &mut Self::Partial,
                key: u64,
            ) -> ::std::result::Result<(), ::usage_argv::Error<'t, 'v>> {
                #(#checks)*
                ::std::result::Result::Ok(())
            }

            fn select<'t, 'v>(
                partial: Self::Partial,
                key: u64,
            ) -> ::std::result::Result<
                ::std::option::Option<Self>,
                ::usage_argv::Error<'t, 'v>,
            > {
                #(#selects)*
                ::std::result::Result::Ok(::std::option::Option::None)
            }
        }
    }
}

/// Everything decided once the last token has been read.
///
/// Ordered deliberately. The environment fills what argv left out, so it runs
/// before required-ness — a flag with `env` set is not missing. Choices and bounds
/// come last, because they judge a value however it arrived, including one that came
/// from the environment or a default.
fn post_binding(cli: &Cli) -> TokenStream {
    let sub_check = subcommand_parts(cli).map(|p| p.check).unwrap_or_default();
    let env_fallbacks = cli.fields.iter().filter_map(|f| {
        let ident = &f.ident;
        let given = format_ident!("__given_{}", ident);
        let var = f.env.as_deref()?;
        let assign = match f.shape {
            Shape::Optional => quote!(partial.#ident = ::std::option::Option::Some(value);),
            Shape::Required => quote!(partial.#ident = value;),
            Shape::Many => quote!(partial.#ident.push(value);),
            // A switch reads as on for anything but the spellings of "off", which is
            // what every tool that takes a boolean from the environment settles on.
            Shape::Bool => quote! {
                partial.#ident = !matches!(
                    value.as_str(),
                    "" | "0" | "false" | "no" | "off"
                );
            },
            // A number, since the environment cannot repeat a flag: `EX_VERBOSE=3`
            // is how you say `-vvv`. An unparseable value leaves the field alone
            // rather than being counted as given.
            Shape::Count => {
                let ty = &f.ty;
                quote! {
                    match value.parse::<#ty>() {
                        ::std::result::Result::Ok(count) => partial.#ident = count,
                        ::std::result::Result::Err(_) => continue_unset = true,
                    }
                }
            }
        };
        Some(quote! {
            if !partial.#given {
                if let ::std::result::Result::Ok(value) = ::std::env::var(#var) {
                    let mut continue_unset = false;
                    #assign
                    if !continue_unset {
                        partial.#given = true;
                    }
                }
            }
        })
    });

    let required_checks = cli.fields.iter().filter_map(|f| {
        // A `String` has nowhere to put "absent", so the type is the declaration.
        // Anything with a default or an environment variable is already filled.
        if f.shape != Shape::Required || f.default.is_some() {
            return None;
        }
        let given = format_ident!("__given_{}", f.ident);
        let name = &f.name;
        Some(quote! {
            if !partial.#given {
                return ::std::result::Result::Err(
                    ::usage_argv::Error::MissingRequired { name: #name },
                );
            }
        })
    });

    let choice_checks = cli.fields.iter().filter_map(|f| {
        if f.choices.is_empty() {
            return None;
        }
        let ident = &f.ident;
        let name = &f.name;
        let choices = &f.choices;
        let values = match f.shape {
            Shape::Optional => quote!(partial.#ident.iter()),
            Shape::Required => quote!(::std::iter::once(&partial.#ident)),
            Shape::Many => quote!(partial.#ident.iter()),
            // Rejected in the model: there is no value to check.
            Shape::Bool | Shape::Count => return None,
        };
        Some(quote! {
            for value in #values {
                if ![#(#choices),*].contains(&value.as_str()) {
                    return ::std::result::Result::Err(
                        ::usage_argv::Error::InvalidChoice {
                            name: #name,
                            choices: &[#(#choices),*],
                        },
                    );
                }
            }
        })
    });

    let bound_checks = cli.fields.iter().filter_map(|f| {
        if f.var_min.is_none() && f.var_max.is_none() {
            return None;
        }
        let ident = &f.ident;
        let name = &f.name;
        let min = match f.var_min {
            Some(min) => quote! {
                if got < #min {
                    return ::std::result::Result::Err(
                        ::usage_argv::Error::VarTooFew { name: #name, min: #min, got },
                    );
                }
            },
            None => quote!(),
        };
        let max = match f.var_max {
            Some(max) => quote! {
                if got > #max {
                    return ::std::result::Result::Err(
                        ::usage_argv::Error::VarTooMany { name: #name, max: #max, got },
                    );
                }
            },
            None => quote!(),
        };
        let given = format_ident!("__given_{}", ident);
        Some(quote! {
            // Only when the field was used. A bound says "if you give values, give
            // this many" — reading an unused optional flag as a violation would make
            // `var_min` a second way to spell required-ness, and there would then be
            // no way to say "at least two, if you use it at all".
            if partial.#given {
                let got = partial.#ident.len();
                #min
                #max
            }
        })
    });

    quote! {
        #(#env_fallbacks)*
        #(#required_checks)*
        #(#choice_checks)*
        #(#bound_checks)*
        #sub_check
    }
}

#[cfg(test)]
mod in_module_tests {
    use super::in_module;

    fn rendered(ty: &str) -> String {
        in_module(&syn::parse_str::<syn::Type>(ty).unwrap())
            .to_string()
            .replace(' ', "")
    }

    #[test]
    fn a_path_is_qualified_for_the_generated_module() {
        assert_eq!(rendered("Commands"), "super::Commands");
        assert_eq!(rendered("cmds::Commands"), "super::cmds::Commands");
        assert_eq!(rendered("crate::cmds::Commands"), "crate::cmds::Commands");
        assert_eq!(rendered("::other::Commands"), "::other::Commands");
        assert_eq!(rendered("self::Commands"), "super::Commands");
        assert_eq!(rendered("super::Commands"), "super::super::Commands");
    }
}
