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

use crate::crate_name::{crate_name, FoundCrate};
use crate::model::{
    rendered_path, to_kebab, Cli, ConditionalDefault, DoubleDash, Field, Kind, Shape, Subcommands,
    ValueEnum,
};

/// Construct the user's command type after its generated partial has been checked.
///
/// An empty named struct is `Self {}`, while a unit struct is just `Self`. Keeping
/// that syntax decision here prevents the root, settings, and nested-command build
/// paths from drifting apart.
fn built_value(cli: &Cli, sub_build: &TokenStream, fields: &[TokenStream]) -> TokenStream {
    if cli.unit {
        debug_assert!(cli.fields.is_empty());
        quote!(Self)
    } else {
        quote! {
            Self {
                #sub_build
                #(#fields),*
            }
        }
    }
}

/// The runtime as the adopter depended on it.
///
/// A direct `usage-argv` dependency wins when both forms are present: a low-level adopter may
/// deliberately enable a different feature set there. Otherwise the `usage-rs` facade provides
/// the runtime as `usage::argv`, keeping derives, tables, and their versions behind one
/// dependency.
///
/// Resolved by reading the adopter's `Cargo.toml` directly rather than via `proc-macro-crate`,
/// so the derive does not drag `toml_edit` into every compile.
fn runtime_path() -> TokenStream {
    match crate_name("usage-argv") {
        Ok(FoundCrate::Name(name)) => {
            let runtime = format_ident!("{name}");
            quote!(::#runtime)
        }
        _ => match crate_name("usage-rs") {
            Ok(FoundCrate::Itself) => quote!(::usage_rs::argv),
            Ok(FoundCrate::Name(name)) => {
                let facade = format_ident!("{name}");
                quote!(::#facade::argv)
            }
            // Preserve the old useful compiler error when neither dependency was declared.
            _ => quote!(::usage_argv),
        },
    }
}

/// The derive package as the adopter depended on it.
///
/// Most emitted code only needs the runtime path. Unit subcommands synthesize an empty `Args`
/// struct, though, so that derive must come through the facade too when it is the application's
/// only dependency.
fn derive_path() -> TokenStream {
    match crate_name("usage-derive") {
        Ok(FoundCrate::Name(name)) => {
            let derive = format_ident!("{name}");
            quote!(::#derive)
        }
        _ => match crate_name("usage-rs") {
            Ok(FoundCrate::Itself) => quote!(::usage_rs),
            Ok(FoundCrate::Name(name)) => {
                let facade = format_ident!("{name}");
                quote!(::#facade)
            }
            _ => quote!(::usage_derive),
        },
    }
}

/// The cold expression evaluator, resolved independently of the binding runtime.
fn validation_path() -> TokenStream {
    match crate_name("usage-validation") {
        Ok(FoundCrate::Itself) => quote!(::usage_validation),
        Ok(FoundCrate::Name(name)) => {
            let validation = format_ident!("{}", name.replace('-', "_"));
            quote!(::#validation)
        }
        _ => match crate_name("usage-rs") {
            Ok(FoundCrate::Itself) => quote!(::usage_rs::validation),
            Ok(FoundCrate::Name(name)) => {
                let facade = format_ident!("{}", name.replace('-', "_"));
                quote!(::#facade::validation)
            }
            _ => quote!(::usage_validation),
        },
    }
}

pub fn emit(cli: &Cli) -> TokenStream {
    let ident = &cli.ident;
    let runtime = runtime_path();
    let validation = validation_path();
    let validation_import = cli
        .fields
        .iter()
        .any(|field| field.validate.is_some())
        .then(|| quote!(use #validation as usage_validation;));

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

    // Left unset when the struct says nothing, which is what lets the parser inherit it: one
    // command per struct, and a macro expansion cannot see the command above it. An earlier
    // version resolved it here and wrote `Value` for every silent command, which made the
    // root's declaration reach the root alone.
    let unknown_flags = unknown_flags_tokens(cli);

    let default_subcommand = option_str(cli.default_subcommand.as_deref());
    let multicall = cli.multicall;
    let no_binary_name = cli.no_binary_name;
    let arg_required_else_help = cli.arg_required_else_help;
    let dont_delimit_trailing_values = cli.dont_delimit_trailing_values;
    let usage = option_str(cli.usage.as_deref());
    let restart_token = option_str(cli.restart_token.as_deref());
    let mount = option_str(cli.mount.as_deref());
    // A bare `T` subcommand field says the command cannot run alone; an `Option<T>` says it
    // can. The parser already refuses the invocation from the type — this is so the emitted
    // spec says it too, since help, docs and completions read that rather than the type.
    let flatten_checks = flatten_checks(cli);
    let subcommand_required = cli.fields.iter().any(|f| {
        matches!(
            f.kind,
            Kind::Subcommand {
                optional: false,
                ..
            }
        )
    });
    let before_help = option_expr(cli.before_help.as_ref());
    let before_long_help = option_expr(cli.before_long_help.as_ref());
    let after_help = option_expr(cli.after_help.as_ref());
    let after_long_help = option_expr(cli.after_long_help.as_ref());
    let root_key = key_ident("COMMAND", None);
    let keys = key_consts(&cli.fingerprint, flags.len(), args.len());
    let flag_tables = flags.iter().enumerate().map(|(i, f)| flag_table(i, f));
    let arg_tables = args.iter().enumerate().map(|(i, f)| arg_table(i, f));
    let flag_metas = flags
        .iter()
        .enumerate()
        .map(|(i, f)| flag_meta(cli, i, f, &cli.ident));
    let arg_metas = args
        .iter()
        .enumerate()
        .map(|(i, f)| arg_meta(cli, i, f, &cli.ident));

    // Both the plain slices and, when a field is flattened, the joined arrays.
    let tables = tables(cli);
    let table_decls = &tables.decls;
    let meta_table_decls = &tables.meta_decls;
    let (group_meta_decl, group_meta_table_ref) = group_meta_table(cli);
    let flag_table_ref = &tables.flags;
    let arg_table_ref = &tables.args;
    let flag_meta_table_ref = &tables.flag_metas;
    let arg_meta_table_ref = &tables.arg_metas;

    let name = &cli.name;
    let bin = option_str(cli.bin.as_deref());
    let version = match &cli.version {
        Some(tokens) => quote!(::core::option::Option::Some(#tokens)),
        None => quote!(::core::option::Option::None),
    };
    let about = cli
        .about_attr
        .as_ref()
        .map(|value| option_expr(Some(value)))
        .unwrap_or_else(|| option_str(cli.about.as_deref()));
    let long_about = cli
        .long_about_attr
        .as_ref()
        .map(|value| option_expr(Some(value)))
        .unwrap_or_else(|| option_str(cli.long_about.as_deref()));

    // The same wiring a nested command uses: the root differs only in how it is
    // entered, so it does not get its own copy.
    let parts = subcommand_parts(cli);
    let sub_commands = parts
        .as_ref()
        .map(|p| p.commands.clone())
        .unwrap_or_default();
    let sub_default = parts
        .as_ref()
        .map(|p| p.default.clone())
        .unwrap_or_default();
    let sub_external = parts
        .as_ref()
        .map(|p| p.external.clone())
        .unwrap_or_default();
    let sub_metas = parts.as_ref().map(|p| p.metas.clone()).unwrap_or_default();
    let sub_build = parts.as_ref().map(|p| p.build.clone()).unwrap_or_default();

    let partial = partial_struct(cli);
    let argument_lookup = argument_lookup_functions(cli);
    let defaults = partial_defaults(cli);
    // A root resolves settings when it binds one itself, or when it says so — which is how a CLI
    // whose bound flags all live in a flattened group asks for the entry points, since it cannot
    // see another struct's fields. A root that does neither gets the compile-time guard instead of
    // the layer, so a group's binding cannot go quietly uncollected.
    let resolves = cli.fields.iter().any(|f| f.setting.is_some()) || cli.settings;
    let parts = settings(cli);
    // Only the layer calls it, so a root that has children and no settings of its own emits
    // neither: the guard below is what speaks for that case.
    let settings_given = parts.as_ref().filter(|_| resolves).map(|s| s.given.clone());
    let settings_bindings = parts
        .as_ref()
        .filter(|_| resolves)
        .map(|s| s.bindings.clone());
    let settings_layer = resolves.then(settings_layer);
    let settings_guard = (!resolves).then(|| settings_guard(cli)).flatten();
    // The name an adopter uses, forwarding to the one inside the const block, which is where
    // the table that reads it lives.
    let settings_binding_forward = settings_bindings.as_ref().map(|_| {
        quote! {
            /// Every flag this CLI reads into a setting, and the setting it sets.
            ///
            /// What `usage_config::Registry::drift` compares against the flags the spec
            /// *declares*, so a documented flag nothing reads — hk has thirteen — fails a test
            /// rather than a user.
            pub const SETTINGS_BINDINGS: &'static [(&'static str, &'static str)] =
                SETTINGS_BINDINGS;
        }
    });
    // The second entry point, emitted only when something is bound: it returns what the parser saw
    // as well as the struct, which is the whole reason it exists.
    let settings_parse = settings_bindings.as_ref().map(|_| {
        let sub_build = sub_build.clone();
        let field_finals: Vec<_> = cli
            .fields
            .iter()
            .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
            .map(field_final)
            .collect();
        let built = built_value(cli, &sub_build, &field_finals);
        quote! {
            /// Parse a command line, and the settings it gave values for.
            ///
            /// The layer is built before the struct because the two read the same partial and the
            /// struct consumes it — and because a value the struct refuses is one this never
            /// returns, the layer going with it.
            pub fn parse_from_with_settings<'v>(
                argv: &[&'v ::std::ffi::OsStr],
            ) -> ::std::result::Result<
                (Self, ::usage_config::CliLayer),
                usage_argv::Error<'static, 'v>,
            > {
                // The layer from what argv left, and only then the rest: `check` fills a field
                // from its `env` and marks it given, and a variable's value contributed here
                // would sit in the layer that outranks every other, named after a flag nobody
                // typed — and counted twice by a `union` setting whose CLI also has an
                // `EnvLayer`. The environment has a layer of its own to arrive in.
                #defaults
                read_argv_into(Self::command(), argv, &mut partial)?;
                let __usage_settings = settings_layer(&partial);
                check(&mut partial)?;
                let __usage_built = #built;
                ::std::result::Result::Ok((__usage_built, __usage_settings))
            }
        }
    });
    let apply = apply_fn(cli);
    let post = post_binding(cli);
    let (completion, completion_intercept) = completion_fns(cli);
    // `field: local` rather than the shorthand, because the locals are prefixed:
    // a field called `text` or `parser` would otherwise collide with something the
    // generated code needs.
    // Collected rather than left lazy: it is used by both the inherent `build` and the trait
    // impl beside it, and an iterator cannot be walked twice.
    let field_finals: Vec<_> = cli
        .fields
        .iter()
        .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
        .map(field_final)
        .collect();
    let built = built_value(cli, &sub_build, &field_finals);

    let min_usage_version = option_str(cli.min_usage_version.as_deref());
    let has_version = cli.version.is_some();
    let runtime_version = cli
        .runtime_version
        .as_ref()
        .or(cli.version.as_ref())
        .cloned()
        .unwrap_or_else(|| quote!(""));
    let runtime_name_decl = cli.runtime_name.as_ref().map(|runtime_name| {
        quote! {
            let __usage_runtime_name: &'static str = #runtime_name;
        }
    });
    let runtime_bin_decl = cli.runtime_bin.as_ref().map(|runtime_bin| {
        quote! {
            let __usage_runtime_bin: &'static str = #runtime_bin;
        }
    });
    let runtime_version_decl = cli.runtime_version.as_ref().map(|_| {
        quote! {
            let __usage_runtime_version =
                ::std::string::ToString::to_string(&(#runtime_version));
        }
    });
    let effective_name = if cli.runtime_name.is_some() {
        quote!(__usage_runtime_name)
    } else {
        quote!(Self::spec().name)
    };
    let effective_bin = if cli.runtime_bin.is_some() {
        quote!(::std::option::Option::Some(__usage_runtime_bin))
    } else {
        quote!(Self::spec().bin)
    };
    let effective_version = if cli.runtime_version.is_some() {
        quote!(::std::option::Option::Some(
            __usage_runtime_version.as_str()
        ))
    } else {
        quote!(Self::spec().version)
    };
    let has_runtime_identity = cli.runtime_name.is_some() || cli.runtime_bin.is_some();
    let effective_spec = if has_runtime_identity || cli.runtime_version.is_some() {
        quote! {
            // Portable literals remain in `SPEC`; runtime expressions are evaluated only on
            // cold output paths. Successful argv parsing still reads the static tables directly.
            #runtime_name_decl
            #runtime_bin_decl
            #runtime_version_decl
            let __usage_runtime_spec = usage_argv::spec::Spec {
                name: #effective_name,
                bin: #effective_bin,
                version: #effective_version,
                ..*Self::spec()
            };
            let __usage_spec = &__usage_runtime_spec;
        }
    } else {
        quote! {
            let __usage_spec = Self::spec();
        }
    };
    let runtime_program = cli
        .runtime_bin
        .as_ref()
        .or(cli.runtime_name.as_ref())
        .cloned();
    let runtime_program_for_version = runtime_program
        .as_ref()
        .map(|program| {
            quote! {
                let __usage_bin: &'static str = #program;
            }
        })
        .unwrap_or_else(|| {
            quote! {
                let __usage_bin = Self::spec().bin.unwrap_or(Self::spec().name);
            }
        });
    let runtime_name_view = cli.runtime_name.as_ref().map(|name| quote!(.name(#name)));
    let runtime_bin_view = cli.runtime_bin.as_ref().map(|bin| quote!(.bin(#bin)));
    let runtime_app = has_runtime_identity.then(|| {
        quote! {
            /// This CLI's cold-path view with its computed process identity applied.
            ///
            /// Runtime identity is required to be `&'static str`, so this borrows the
            /// static metadata and allocates nothing.
            pub fn runtime_app() -> usage_argv::spec::SpecView<'static> {
                Self::app() #runtime_name_view #runtime_bin_view
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
        const _: () = {
            use #runtime as usage_argv;
            #validation_import

            #flatten_checks
            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #table_decls

            pub static ROOT: usage_argv::Command = usage_argv::Command {
                // Only where a version was declared, which is when clap adds the flag: a
                // `--version` that answers with nothing is worse than one that is not there.
                version: #has_version,
                unknown_flags: #unknown_flags,
                arg_required_else_help: #arg_required_else_help,
                dont_delimit_trailing_values: #dont_delimit_trailing_values,
                name: #name,
                key: #root_key,
                flags: #flag_table_ref,
                args: #arg_table_ref,
                #sub_commands
                #sub_default
                #sub_external
                ..usage_argv::Command::EMPTY
            };

            // The model can see that the root has a subcommand field, but the enum is
            // expanded separately. Its COMMANDS table is the point where an
            // external-only enum becomes distinguishable from one with named applets.
            const _: () = assert!(
                !#multicall || !ROOT.subcommands.is_empty(),
                "`multicall` needs at least one named subcommand to select",
            );

            #(#flag_metas)*
            #(#arg_metas)*
            #meta_table_decls

            #group_meta_decl

            pub static ROOT_META: usage_argv::spec::CommandMeta = usage_argv::spec::CommandMeta {
                cmd: &ROOT,
                about: #about,
                long_about: #long_about,
                restart_token: #restart_token,
                subcommand_required: #subcommand_required,
                mount: #mount,
                before_help: #before_help,
                before_long_help: #before_long_help,
                after_help: #after_help,
                after_long_help: #after_long_help,
                flags: #flag_meta_table_ref,
                args: #arg_meta_table_ref,
                groups: #group_meta_table_ref,
                #sub_metas
                ..usage_argv::spec::CommandMeta::EMPTY
            };

            // Values arrive as the bytes that were on the command line. This version
            // holds them as `String`, and converts lossily, which is what mise
            // already does with its own argv. Rejecting a non-UTF-8 value needs an
            // error type for value conversion, and that arrives with typed fields.
            pub fn __usage_text(value: &[u8]) -> ::std::vec::Vec<u8> {
                value.to_vec()
            }

            pub fn __usage_value_text(
                value: ::std::option::Option<&[u8]>,
            ) -> ::std::vec::Vec<u8> {
                value.map(__usage_text).unwrap_or_default()
            }

            #partial
            #apply
            #argument_lookup

            /// Everything decided after the last token.
            ///
            /// In the module rather than beside the parse, so every reference it makes
            /// to the user's own types sits at one consistent scope — the root and a
            /// nested command generate the same code here.
            pub fn check<'t, 'v>(
                partial: &mut Partial,
            ) -> ::std::result::Result<(), usage_argv::Error<'t, 'v>> {
                // Read unconditionally: a command that declares nothing to check would
                // otherwise leave the parameter unused in the user's crate, where
                // nobody can silence it.
                let _ = &partial;
                #post
                ::std::result::Result::Ok(())
            }

            /// Every token read, and nothing else decided.
            ///
            /// The partial as *argv* left it: before a declared default fills a field, before the
            /// environment does, before anything is checked. In the module because two entry
            /// points want it, and one of them wants exactly this much — a settings layer is
            /// what the command line contributed, and `check` fills fields from the environment
            /// and marks them given, which would hand a variable's value to the layer that
            /// outranks every other and name it after a flag nobody typed.
            pub fn read_argv<'v>(
                command: &'static usage_argv::Command<'static>,
                argv: &[&'v ::std::ffi::OsStr],
            ) -> ::std::result::Result<Partial, usage_argv::Error<'static, 'v>> {
                #defaults
                read_argv_into(command, argv, &mut partial)?;
                ::std::result::Result::Ok(partial)
            }

            /// `read_argv`, filling a partial the caller already owns.
            ///
            /// The entry points call this rather than `read_argv`, because a `Partial` is
            /// the whole CLI's worth of accumulator — every command's fields, inlined —
            /// and returning one by value is a memcpy of all of it. At mise's scale that is
            /// 11KB a move, and returning it up the chain from here to `parse_from` made
            /// four: 87% of a parse was spent copying a struct of which all but one
            /// command's worth goes untouched. Threading `&mut` puts the partial in the
            /// frame that will consume it, leaving only the one copy that builds it.
            pub fn read_argv_into<'v>(
                command: &'static usage_argv::Command<'static>,
                argv: &[&'v ::std::ffi::OsStr],
                partial: &mut Partial,
            ) -> ::std::result::Result<(), usage_argv::Error<'static, 'v>> {
                let mut __usage_parser = usage_argv::Parser::new(command, argv);
                while let ::std::option::Option::Some(__usage_event) =
                    __usage_parser.next_event()
                {
                    let __usage_event = __usage_event?;
                    // Asked *before* the event is applied, and answered with the command in
                    // scope: `mise config --help` is a question about `config`, and the parser
                    // is what knows how deep the words reached.
                    if let usage_argv::Event::Flag { flag, .. } = &__usage_event {
                        if flag.key == usage_argv::HELP_LONG_KEY
                            || flag.key == usage_argv::HELP_SHORT_KEY
                        {
                            return ::std::result::Result::Err(usage_argv::Error::Help {
                                cmd: __usage_parser.command(),
                                long: flag.key == usage_argv::HELP_LONG_KEY,
                            });
                        }
                        // Same shape, and for the same reason: a question rather than a
                        // failure, answered by whoever knows the version string.
                        if usage_argv::is_version_flag(flag) {
                            return ::std::result::Result::Err(
                                usage_argv::Error::Version,
                            );
                        }
                    }
                    // `apply` handles this command's own fields and routes anything
                    // else into its subcommands, which is why a nested command needs
                    // nothing extra here.
                    apply(partial, &__usage_event);
                }

                if __usage_parser.command().arg_required_else_help
                    && __usage_parser.command_start() == argv.len()
                {
                    return ::std::result::Result::Err(usage_argv::Error::Help {
                        cmd: __usage_parser.command(),
                        long: false,
                    });
                }

                ::std::result::Result::Ok(())
            }

            /// Every token read, and everything decided after the last one.
            ///
            /// What a caller that only wants the struct wants: a partial nothing more will be
            /// added to. A struct cannot answer what the *parser* saw — a `bool` field is `false`
            /// whether the flag was absent or negated — so the entry point that wants that reads
            /// the two halves apart instead.
            pub fn read<'v>(
                command: &'static usage_argv::Command<'static>,
                argv: &[&'v ::std::ffi::OsStr],
            ) -> ::std::result::Result<Partial, usage_argv::Error<'static, 'v>> {
                #defaults
                read_into(command, argv, &mut partial)?;
                ::std::result::Result::Ok(partial)
            }

            /// `read`, filling a partial the caller already owns. See `read_argv_into`.
            pub fn read_into<'v>(
                command: &'static usage_argv::Command<'static>,
                argv: &[&'v ::std::ffi::OsStr],
                partial: &mut Partial,
            ) -> ::std::result::Result<(), usage_argv::Error<'static, 'v>> {
                read_argv_into(command, argv, partial)?;
                check(partial)?;
                ::std::result::Result::Ok(())
            }

            #settings_given
            #settings_bindings
            #settings_layer
            #settings_guard

            pub static SPEC: usage_argv::spec::Spec = usage_argv::spec::Spec {
                name: #name,
                bin: #bin,
                version: #version,
                min_usage_version: #min_usage_version,
                about: #about,
                long_about: #long_about,
                usage: #usage,
                default_subcommand: #default_subcommand,
                multicall: #multicall,
                root: &ROOT_META,
            };

            impl #ident {
                /// The parse tables for this CLI.
                ///
                /// `static`, so reaching them costs nothing: there is no command tree
                /// to build before a parse can start.
                pub fn command() -> &'static usage_argv::Command<'static> {
                    &ROOT
                }

                /// This CLI's spec, for emitting, documenting, or completing.
                pub fn spec() -> &'static usage_argv::spec::Spec<'static> {
                    &SPEC
                }

                /// A borrowed cold-path view for runtime identity and sparse metadata overlays.
                ///
                /// Parsing continues to read [`Self::command`] directly; constructing this
                /// copies no command tree and performs no work on an ordinary invocation.
                pub fn app() -> usage_argv::spec::SpecView<'static> {
                    Self::spec().view()
                }

                #runtime_app

                /// This CLI's spec as KDL, which is what `usage g markdown|manpage`
                /// and the completion generators read.
                pub fn to_kdl() -> ::std::string::String {
                    SPEC.to_kdl()
                }

                #settings_binding_forward
                #settings_parse

                /// Parse a command line, excluding the program name.
                pub fn parse_from<'v>(
                    argv: &[&'v ::std::ffi::OsStr],
                ) -> ::std::result::Result<Self, usage_argv::Error<'static, 'v>> {
                    // The partial is built here and filled through `&mut`, rather than
                    // returned up the chain: see `read_argv_into`.
                    #defaults
                    read_into(Self::command(), argv, &mut partial)?;
                    ::std::result::Result::Ok(#built)
                }

                /// Parse a full argv, including the program name.
                ///
                /// This is the test- and embedding-friendly counterpart to
                /// [`Self::parse`]: it strips argv0 for an ordinary CLI and
                /// applies the same basename-based applet selection for a
                /// multicall CLI, while returning errors instead of exiting.
                pub fn parse_from_argv<'v>(
                    argv: &[&'v ::std::ffi::OsStr],
                ) -> ::std::result::Result<Self, usage_argv::Error<'static, 'v>> {
                    let ::std::option::Option::Some((__usage_argv0, __usage_words)) =
                        argv.split_first()
                    else {
                        return Self::parse_from(&[]);
                    };
                    if SPEC.multicall {
                        if let ::std::option::Option::Some(__usage_word) =
                            __usage_argv0.to_str().and_then(|s| {
                                usage_argv::multicall_applet(s, SPEC.name, SPEC.bin)
                            })
                        {
                            let mut __usage_rewritten =
                                ::std::vec::Vec::with_capacity(argv.len());
                            __usage_rewritten.push(::std::ffi::OsStr::new(__usage_word));
                            __usage_rewritten.extend_from_slice(__usage_words);
                            return Self::parse_from(&__usage_rewritten);
                        }
                    }
                    Self::parse_from(__usage_words)
                }

                /// Parse using clap's `try_parse_from` argv contract.
                ///
                /// Input includes argv0 by default. `#[command(no_binary_name)]`
                /// opts into treating every supplied word as an argument.
                pub fn try_parse_from<'v>(
                    argv: &[&'v ::std::ffi::OsStr],
                ) -> ::std::result::Result<Self, usage_argv::Error<'static, 'v>> {
                    if #no_binary_name {
                        Self::parse_from(argv)
                    } else {
                        Self::parse_from_argv(argv)
                    }
                }

                /// Parse the process's own arguments.
                #completion

                pub fn parse() -> Self {
                    #completion_intercept
                    let __usage_raw: ::std::vec::Vec<::std::ffi::OsString> = if SPEC.multicall {
                        let mut __usage_all: ::std::vec::Vec<::std::ffi::OsString> =
                            ::std::env::args_os().collect();
                        if !__usage_all.is_empty() {
                            let __usage_argv0 = __usage_all.remove(0);
                            if let ::std::option::Option::Some(__usage_word) =
                                __usage_argv0.to_str().and_then(|s| {
                                    usage_argv::multicall_applet(s, SPEC.name, SPEC.bin)
                                })
                            {
                                __usage_all.insert(
                                    0,
                                    ::std::ffi::OsString::from(__usage_word),
                                );
                            }
                        }
                        __usage_all
                    } else {
                        ::std::env::args_os().skip(1).collect()
                    };
                    let __usage_argv: ::std::vec::Vec<&::std::ffi::OsStr> =
                        __usage_raw.iter().map(|a| a.as_os_str()).collect();
                    // This is the entry point that *is* the process — it already exits for a help
                    // request — so it answers a failure the way a command-line program does:
                    // the message on stderr, and a non-zero status. `parse_from` hands the error
                    // back instead, for a library embedding this that wants to decide.
                    match Self::parse_from(&__usage_argv) {
                        ::std::result::Result::Ok(parsed) => parsed,
                        // Not failures: someone asked a question, and the answer goes to stdout.
                        ::std::result::Result::Err(usage_argv::Error::Version) => {
                            #runtime_program_for_version
                            let __usage_version = #runtime_version;
                            ::std::println!("{__usage_bin} {__usage_version}");
                            ::std::process::exit(0);
                        }
                        ::std::result::Result::Err(usage_argv::Error::Help { cmd, long }) => {
                            #effective_spec
                            // By the route the words took, not by the command's address: one
                            // `Subcommands` type mounted under two parents is one address, and a
                            // page found by searching for it carries the first mount's path and
                            // globals. Falls back where the route cannot be rebuilt.
                            let __usage_page = match usage_argv::help::route_to(
                                Self::command(),
                                &__usage_argv,
                                cmd,
                            ) {
                                ::std::option::Option::Some(route) => {
                                    usage_argv::help::render_at(__usage_spec, &route, long)
                                }
                                ::std::option::Option::None => {
                                    usage_argv::help::render(__usage_spec, cmd, long)
                                }
                            };
                            match __usage_page {
                                ::std::option::Option::Some(page) => {
                                    ::std::print!("{page}");
                                    ::std::process::exit(0);
                                }
                                // Only reachable if the command came from another CLI's tables.
                                ::std::option::Option::None => ::std::process::exit(0),
                            }
                        }
                        ::std::result::Result::Err(e) => {
                            #effective_spec
                            ::std::eprint!(
                                "{}",
                                usage_argv::render_failure(__usage_spec, &__usage_argv, &e)
                            );
                            // clap's, so a script that checks for it keeps working.
                            ::std::process::exit(2);
                        }
                    }
                }
            }
        };
    }
}

/// A compile-time refusal of a flattened group that declares subcommands.
///
/// Flatten joins one struct's flags and arguments into another's tables. Subcommands are not
/// joined — the parent's table has no entry for them — but the group's own `build` still
/// demands one, so the shape compiles into a command that cannot be run and whose emitted
/// spec says it can: `subcommand_required` reads the parent's own fields and sees none.
///
/// Refused rather than supported, as `Option<T>` flatten is: joining two commands' subcommand
/// sets needs a rule for which enum a word selects from, and nothing in the fleet asks for
/// one. Checked in the *parent's* expansion, where the group is only a type — its `COMMAND`
/// is a `const`, so the parent can look at it during const evaluation without the two
/// expansions ever seeing each other. Written as the user wrote it, like every other use of a
/// flattened type here: the tables are emitted beside their struct now rather than in a module
/// above it, so there is no path to rewrite.
fn flatten_checks(cli: &Cli) -> TokenStream {
    let shape_checks = cli.fields.iter().filter_map(|f| {
        let Kind::Flatten { ty } = &f.kind else {
            return None;
        };
        Some(quote! {
            const _: () = ::core::assert!(
                <#ty as usage_argv::spec::CommandArgs>::COMMAND.subcommands.is_empty(),
                "a flattened group cannot declare subcommands: flatten joins flags and \
                 arguments into the parent's tables and leaves subcommands behind, so the \
                 command would require one that no word could select. Declare the \
                 `subcommand` field on the command's own struct instead."
            );
        })
    });
    let command = if cli.composable {
        quote!(COMMAND)
    } else {
        quote!(ROOT)
    };
    let relationship_checks = cli.fields.iter().flat_map(|field| {
        field
            .overrides
            .iter()
            .filter(|selector| cli.field_for_selector(selector).is_none())
            .map(|selector| {
                quote! {
                    const _: () = ::core::assert!(
                        usage_argv::spec::flag_selector_count(&#command, #selector) == 1,
                        ::core::concat!(
                            "`overrides = ",
                            #selector,
                            "` must name exactly one flag after flattened groups are composed",
                        ),
                    );
                }
            })
    });
    quote!(#(#shape_checks)* #(#relationship_checks)*)
}

/// A command's `unknown_flags`, as the table's `Option`.
///
/// `None` is not a default so much as a deferral: the parser carries the enclosing command's
/// answer down, so a struct that declares nothing keeps whatever it was given. Only a struct
/// that says something writes anything.
fn unknown_flags_tokens(cli: &Cli) -> TokenStream {
    match cli.unknown_flags.as_deref() {
        Some("error") => quote!(::core::option::Option::Some(
            usage_argv::UnknownFlags::Error
        )),
        Some(_) => quote!(::core::option::Option::Some(
            usage_argv::UnknownFlags::Value
        )),
        None => quote!(::core::option::Option::None),
    }
}

/// The completion entry points, for a CLI that asked for them.
///
/// Two pieces: a function that answers a request, and the line in `parse` that notices one. Both
/// empty without the opt-in, so a binary carries neither the hidden command nor the protocol —
/// and `completion_script` cannot be called for a CLI whose binary would not answer it.
fn completion_fns(cli: &Cli) -> (TokenStream, TokenStream) {
    if !cli.completion {
        return (TokenStream::new(), TokenStream::new());
    }
    let completion_program = cli
        .runtime_bin
        .as_ref()
        .or(cli.runtime_name.as_ref())
        .map(|program| {
            quote! {
                let __usage_runtime_program: &'static str = #program;
                __usage_runtime_program.to_string()
            }
        })
        .unwrap_or_else(|| {
            quote! {
                let spec = Self::spec();
                spec.bin.unwrap_or(spec.name).to_string()
            }
        });
    let functions = quote! {
        // Says what is missing, where the alternative is `unresolved module complete` — which
        // names the symptom and not the attribute that asked for it.
        usage_argv::__usage_needs_complete_feature!();

        /// This CLI's completion script for `shell`, to be written to a file or sourced.
        ///
        /// Emitted under the same attribute as the command it calls, which is what makes a
        /// script that names a command the binary does not answer a compile error instead of a
        /// silence at the prompt.
        pub fn completion_script(
            shell: usage_argv::complete::Shell,
        ) -> ::std::string::String {
            let __usage_program = { #completion_program };
            usage_argv::script::script(&__usage_program, shell)
        }

        /// The word a shell is completing, answered from this CLI's own tables.
        ///
        /// `None` when argv is an ordinary invocation. The request is recognized before the
        /// parse rather than inside it: a completion is not a command this CLI runs, and putting
        /// it in the tables would make it one — visible to the grammar, the help and the spec.
        pub fn completion_request(
            argv: &[::std::ffi::OsString],
        ) -> ::std::option::Option<::std::string::String> {
            let first = argv.first()?.to_str()?;
            if first != "__complete_word__" {
                return ::std::option::Option::None;
            }
            // Its own flags, read by hand: three of them, and reading them with the parser
            // would mean putting them in the tables this is deliberately outside of.
            let mut shell = usage_argv::complete::Shell::Bash;
            let mut line = ::std::string::String::new();
            let mut cursor = ::std::option::Option::None;
            let mut candidates_for: ::std::option::Option<::std::string::String> =
                ::std::option::Option::None;
            let mut rest = argv[1..].iter();
            while let ::std::option::Option::Some(arg) = rest.next() {
                match arg.to_str().unwrap_or_default() {
                    "--shell" => {
                        if let ::std::option::Option::Some(name) = rest.next() {
                            if let ::std::option::Option::Some(found) =
                                usage_argv::complete::Shell::from_name(
                                    &name.to_string_lossy(),
                                )
                            {
                                shell = found;
                            }
                        }
                    }
                    "--line" => {
                        if let ::std::option::Option::Some(value) = rest.next() {
                            line = value.to_string_lossy().into_owned();
                        }
                    }
                    "--cursor" => {
                        cursor = rest
                            .next()
                            .and_then(|value| value.to_str().and_then(|v| v.parse().ok()));
                    }
                    // What the `run=` in this CLI's own emitted spec asks for: one named
                    // completer's answers, rather than everything the cursor could take. That is
                    // the shape a spec's `complete` block promises, so anything reading the KDL
                    // gets what it expects from the binary the KDL names.
                    "--candidates" => {
                        candidates_for = rest.next().map(|v| v.to_string_lossy().into_owned());
                    }
                    // Anything else is a shell passing something this version does not know
                    // about. Ignored rather than refused: a completion that errors out is a
                    // shell that beeps at every keystroke.
                    _ => {}
                }
            }
            // No cursor means the end of the line, which is where a shell puts it when it has
            // no way to say — nushell, whose completer only ever sees the words.
            let cursor = cursor.unwrap_or(line.len());
            let split = usage_argv::complete::split(&line, cursor, shell);
            if let ::std::option::Option::Some(name) = candidates_for {
                // Walked here as well, because a `--candidates` request names a completer and
                // says nothing about where the cursor is — and the completer still wants the
                // words its own command was given.
                let position =
                    usage_argv::complete::walk(Self::spec().root.cmd, split.argv());
                let __usage_words = split.argv();
                let __usage_path: ::std::vec::Vec<(
                    &usage_argv::Command<'_>,
                    &[::std::string::String],
                )> = position
                    .path
                    .iter()
                    .map(|(cmd, start)| (*cmd, __usage_words.get(*start..).unwrap_or(&[])))
                    .collect();
                let ctx = usage_argv::complete::CompleteCtx {
                    words: &split.words,
                    cword: split.cword,
                    prefix: &split.prefix,
                    command_words: __usage_words
                        .get(position.command_start..)
                        .unwrap_or(&[]),
                    command_path: &__usage_path,
                };
                // Nothing of that name is an empty answer rather than an error: a spec written
                // against a newer version of this CLI is a stale script, and a stale script
                // should complete nothing rather than print a message into the user's prompt.
                let found =
                    usage_argv::complete::for_name(Self::spec(), &name, &ctx).unwrap_or_default();
                let answer = usage_argv::complete::Completions {
                    candidates: found,
                    files: ::std::option::Option::None,
                };
                return ::std::option::Option::Some(usage_argv::complete::render(&answer, shell));
            }
            let answer = usage_argv::complete::complete(Self::spec(), &split);
            ::std::option::Option::Some(usage_argv::complete::render(&answer, shell))
        }
    };
    let intercept = quote! {
        // Before anything else, including the parse: a completion request is not this CLI's
        // grammar and must not be measured against it.
        {
            let __usage_args: ::std::vec::Vec<::std::ffi::OsString> =
                ::std::env::args_os().skip(1).collect();
            if let ::std::option::Option::Some(answer) = Self::completion_request(&__usage_args) {
                ::std::print!("{answer}");
                ::std::process::exit(0);
            }
        }
    };
    (functions, intercept)
}

fn flag_table(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("FLAG_{i}");
    let key = key_ident("FLAG", Some(i));
    let field_name = &field.name;
    let Kind::Flag {
        longs,
        hidden_longs: _,
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
    // The bound on one occurrence's values. A repeatable flag's bound counts occurrences
    // instead, which no single token can decide, so that one stays a post-binding check.
    let var_max = match field.value_var_max.filter(|_| *variadic) {
        Some(max) => {
            // Saturating, not `as`: on a 64-bit target a bound above `u32::MAX` would
            // narrow, and `4294967296` narrows to zero — a limit of "none" read as a limit
            // of "stop immediately". A variadic bounded above four billion is unbounded in
            // every practical sense, so clamping says the same thing safely.
            let max = u32::try_from(max).unwrap_or(u32::MAX);
            quote!(::std::option::Option::Some(#max))
        }
        None => quote!(::std::option::Option::None),
    };

    let table_delimiter = table_delimiter(field);
    let allow_hyphen_values = field.allow_hyphen_values;
    let allow_negative_numbers = field.allow_negative_numbers;
    let value_terminator = match field.value_terminator.as_deref() {
        Some(value) => quote!(::core::option::Option::Some(#value.as_bytes())),
        None => quote!(::core::option::Option::None),
    };
    let require_equals = field.require_equals;
    let default_missing = match field.default_missing.as_deref() {
        Some(value) => quote!(::core::option::Option::Some(#value.as_bytes())),
        None => quote!(::core::option::Option::None),
    };
    quote! {
        pub static #name: usage_argv::Flag = usage_argv::Flag {
            key: #key,
            name: #field_name,
            longs: &[#(#longs),*],
            shorts: &[#(#shorts),*],
            negate: #negate,
            takes_value: #takes_value,
            variadic: #variadic,
            var_max: #var_max,
            delimiter: #table_delimiter,
            allow_hyphen_values: #allow_hyphen_values,
            allow_negative_numbers: #allow_negative_numbers,
            value_terminator: #value_terminator,
            require_equals: #require_equals,
            default_missing: #default_missing,
            global: #global,
        };
    }
}

/// The delimiter as the parser tables want it: a byte, or nothing.
///
/// Validated to one byte where the attribute is read, so a `char` that does not fit is
/// already impossible here rather than silently truncated.
fn table_delimiter(field: &Field) -> TokenStream {
    // ASCII, matching the rule the attribute enforces: splitting is by byte, and a
    // separator that is one byte as a scalar but two as UTF-8 would match the continuation
    // bytes inside unrelated characters.
    match field.delimiter.filter(char::is_ascii).map(|d| d as u8) {
        Some(byte) => quote!(::std::option::Option::Some(#byte)),
        None => quote!(::std::option::Option::None),
    }
}

fn arg_table(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("ARG_{i}");
    let key = key_ident("ARG", Some(i));
    let field_name = &field.name;
    let var = field.shape == Shape::Many;
    let Kind::Arg { double_dash } = &field.kind else {
        unreachable!("filtered by the caller");
    };
    let double_dash = match double_dash {
        DoubleDash::Optional => quote!(usage_argv::DoubleDash::Optional),
        DoubleDash::Required => quote!(usage_argv::DoubleDash::Required),
        DoubleDash::Preserve => quote!(usage_argv::DoubleDash::Preserve),
        DoubleDash::Automatic => quote!(usage_argv::DoubleDash::Automatic),
    };
    // A bound stops the variadic while binding, so the argument after it is reachable.
    let var_max = match field.var_max.filter(|_| var) {
        Some(max) => {
            // Saturating, for the reason given on the flag above.
            let max = u32::try_from(max).unwrap_or(u32::MAX);
            quote!(::std::option::Option::Some(#max))
        }
        None => quote!(::std::option::Option::None),
    };

    let table_delimiter = table_delimiter(field);
    let allow_negative_numbers = field.allow_negative_numbers;
    let value_terminator = match field.value_terminator.as_deref() {
        Some(value) => quote!(::core::option::Option::Some(#value.as_bytes())),
        None => quote!(::core::option::Option::None),
    };
    quote! {
        pub static #name: usage_argv::Arg = usage_argv::Arg {
            key: #key,
            name: #field_name,
            var: #var,
            var_max: #var_max,
            delimiter: #table_delimiter,
            allow_negative_numbers: #allow_negative_numbers,
            value_terminator: #value_terminator,
            double_dash: #double_dash,
        };
    }
}

/// The completer entry for a field, and the wrapper that gives it a typed partial.
///
/// A table entry has one uniform signature, and the function a CLI writes has the signature that
/// is useful — its own command's half-parsed struct, and the context. The wrapper is what turns
/// the first into the second: it reparses the words that command was given, which the position
/// reports, and hands over the result. So `mise task ls --file other.toml ⌶` can be completed
/// against that file, which is exactly what a `run=` shelling out to `mise tasks ls --complete`
/// cannot see.
fn completer_tokens(
    i: usize,
    field: &Field,
    kind: &str,
    owner: &syn::Ident,
) -> (TokenStream, TokenStream) {
    let Some(path) = &field.complete else {
        return (TokenStream::new(), quote!(::std::option::Option::None));
    };
    let wrapper = format_ident!("__usage_complete_{kind}_{i}");
    // Through the same rewriting a field's *type* goes through: the generated module is one
    // level below where the user wrote the path, so a bare name shifts by one — while
    // `crate::…`, a leading `::` and `self::…` mean something already, and prefixing those
    // produced a path that does not resolve.
    let completer_path = path;
    let decl = quote! {
        fn #wrapper(
            ctx: &usage_argv::complete::CompleteCtx<'_>,
        ) -> ::std::vec::Vec<usage_argv::complete::Candidate<'static>> {
            // The words this command was given, parsed against this command's own tables — so
            // what the callback reads is what the parser would have bound, rather than a slice
            // of the line it has to interpret itself.
            // This command's own words, asked for by this command — not the deepest one the
            // line reached. A global flag is declared on an ancestor, and reparsing the
            // subcommand's words against the ancestor's tables would drop everything the
            // ancestor was given before the subcommand's name.
            let __usage_declaration =
                <#owner as usage_argv::spec::CommandArgs>::COMMAND;
            let (__usage_command, __usage_words) = ctx
                .command_for(__usage_declaration)
                .unwrap_or((__usage_declaration, ctx.command_words));
            let __usage_owned: ::std::vec::Vec<::std::ffi::OsString> = __usage_words
                .iter()
                .map(::std::ffi::OsString::from)
                .collect();
            let __usage_argv: ::std::vec::Vec<&::std::ffi::OsStr> =
                __usage_owned.iter().map(|a| a.as_os_str()).collect();
            let mut partial = <#owner as usage_argv::spec::CommandArgs>::start();
            let mut parser = usage_argv::Parser::new(
                __usage_command,
                &__usage_argv,
            );
            // A line being completed is unfinished by definition, so an error means the grammar
            // ran out — the partial holds what was understood before that, which is the point.
            while let ::std::option::Option::Some(event) = parser.next_event() {
                match event {
                    ::std::result::Result::Ok(event) => {
                        let _ = <#owner as usage_argv::spec::CommandArgs>::apply(
                            &mut partial,
                            &event,
                        );
                    }
                    ::std::result::Result::Err(_) => break,
                }
            }
            #completer_path(&partial, ctx)
        }
    };
    (decl, quote!(::std::option::Option::Some(#wrapper)))
}

fn flag_meta(cli: &Cli, i: usize, field: &Field, owner: &syn::Ident) -> TokenStream {
    let name = format_ident!("FLAG_META_{i}");
    let table = format_ident!("FLAG_{i}");
    let help = option_str(field.help.as_deref());
    let long_help = option_str(field.long_help.as_deref());
    let env = option_str(field.env.as_deref());
    let help_heading = option_str(field.help_heading.as_deref());
    let value_name = option_str(field.value_name.as_deref());
    let value_names = &field.value_names;
    let complete_type = option_str(field.complete_type.as_deref());
    let defaults = &field.default;
    let default = quote!(&[#(#defaults),*]);
    let hide = field.hide;
    let count = field.shape == Shape::Count;
    let repeatable = field.repeatable;
    let hidden_longs = match &field.kind {
        Kind::Flag { hidden_longs, .. } => hidden_longs,
        _ => unreachable!("flag metadata is generated only for flags"),
    };
    // Same rule as an argument: a `String` has nowhere to put "absent". The runtime
    // check already enforced it; the spec has to say it too, or docs and completions
    // describe a different CLI from the one that runs.
    // A collecting field's type cannot say whether one value is needed, so `required` may
    // declare it. Every other shape gets its answer from the type.
    let required =
        (field.shape == Shape::Required || field.required_collection) && field.default.is_empty();
    // Declared, not inferred: `Option<String>` already says the *flag* is optional and says
    // nothing about whether its value is.
    let value_optional = field.value_optional;
    let (choices, accepted_choices, choice_aliases, choice_details, ignore_case) =
        choices_tokens(field);
    let validate = option_str(field.validate.as_deref());
    let validate_error = option_str(field.validate_error.as_deref());
    let (var_min, var_max) = bounds_tokens(field);
    let (value_var_min, value_var_max) = value_bounds_tokens(field);
    // Written as declared, in the spec's own spelling, so the emitted KDL says what
    // the struct says.
    let overrides = &field.overrides;
    let conflicts: Vec<String> = field
        .conflicts
        .iter()
        .map(|selector| {
            cli.field_for_selector(selector)
                .and_then(Cli::selector_for_field)
                .unwrap_or_else(|| selector.clone())
        })
        .collect();
    let canonical = |selector: &String| {
        cli.field_for_selector(selector)
            .and_then(Cli::selector_for_field)
            .unwrap_or_else(|| selector.clone())
    };
    let requires: Vec<String> = field.requires.iter().map(canonical).collect();
    let requires_if = field.requires_if.iter().map(|condition| {
        let value = &condition.value;
        let requires = canonical(&condition.requires);
        quote!(usage_argv::spec::RequiresIf { value: #value, requires: #requires })
    });
    let default_if = field.default_if.iter().map(|condition| {
        let selector = canonical(&condition.selector);
        let value = &condition.value;
        let when = match &condition.when {
            Some(when) => quote!(::core::option::Option::Some(#when)),
            None => quote!(::core::option::Option::None),
        };
        quote!(usage_argv::spec::DefaultIf { selector: #selector, when: #when, value: #value })
    });
    let exclusive = field.exclusive;
    let delimiter = match field.delimiter {
        Some(c) => quote!(::std::option::Option::Some(#c)),
        None => quote!(::std::option::Option::None),
    };
    let required_if: Vec<String> = field
        .required_if
        .iter()
        .map(|selector| {
            cli.field_for_selector(selector)
                .and_then(Cli::selector_for_field)
                .unwrap_or_else(|| selector.clone())
        })
        .collect();
    let required_if_eq = field.required_if_eq.iter().map(|condition| {
        let selector = cli
            .field_for_selector(&condition.selector)
            .and_then(Cli::selector_for_field)
            .unwrap_or_else(|| condition.selector.clone());
        let value = &condition.value;
        quote!(usage_argv::spec::RequiredIfEq { selector: #selector, value: #value })
    });
    let required_if_eq_all = field.required_if_eq_all.iter().map(|condition| {
        let selector = cli
            .field_for_selector(&condition.selector)
            .and_then(Cli::selector_for_field)
            .unwrap_or_else(|| condition.selector.clone());
        let value = &condition.value;
        quote!(usage_argv::spec::RequiredIfEq { selector: #selector, value: #value })
    });
    let required_unless: Vec<String> = field.required_unless.iter().map(canonical).collect();
    let required_unless_all: Vec<String> =
        field.required_unless_all.iter().map(canonical).collect();

    let (completer_decl, completer) = completer_tokens(i, field, "flag", owner);

    // `None` unless declared, and only on a flag: an argument is not something a user
    // supplies to change what happens, it is the thing being acted on.
    let effect = field
        .effect
        .clone()
        .unwrap_or_else(|| quote!(::core::option::Option::None));
    quote! {
        #completer_decl
        pub static #name: usage_argv::spec::FlagMeta = usage_argv::spec::FlagMeta {
            effect: #effect,
            complete: #completer,
            complete_type: #complete_type,
            flag: &#table,
            help: #help,
            long_help: #long_help,
            env: #env,
            default: #default,
            help_heading: #help_heading,
            value_name: #value_name,
            value_names: &[#(#value_names),*],
            hide: #hide,
            count: #count,
            repeatable: #repeatable,
            hidden_shorts: &[],
            hidden_longs: &[#(#hidden_longs),*],
            required: #required,
            value_optional: #value_optional,
            accepted_choices: #accepted_choices,
            choices: #choices,
            choice_aliases: #choice_aliases,
            choice_details: #choice_details,
            ignore_case: #ignore_case,
            validate: #validate,
            validate_error: #validate_error,
            var_min: #var_min,
            var_max: #var_max,
            value_var_min: #value_var_min,
            value_var_max: #value_var_max,
            overrides: &[#(#overrides),*],
            conflicts: &[#(#conflicts),*],
            requires: &[#(#requires),*],
            requires_if: &[#(#requires_if),*],
            default_if: &[#(#default_if),*],
            exclusive: #exclusive,
            delimiter: #delimiter,
            required_if: &[#(#required_if),*],
            required_if_eq: &[#(#required_if_eq),*],
            required_if_eq_all: &[#(#required_if_eq_all),*],
            required_unless: &[#(#required_unless),*],
            required_unless_all: &[#(#required_unless_all),*],
            ..usage_argv::spec::FlagMeta::EMPTY
        };
    }
}

fn arg_meta(cli: &Cli, i: usize, field: &Field, owner: &syn::Ident) -> TokenStream {
    let name = format_ident!("ARG_META_{i}");
    let table = format_ident!("ARG_{i}");
    let help = option_str(field.help.as_deref());
    let long_help = option_str(field.long_help.as_deref());
    let env = option_str(field.env.as_deref());
    let help_heading = option_str(field.help_heading.as_deref());
    let complete_type = option_str(field.complete_type.as_deref());
    let value_names = &field.value_names;
    let defaults = &field.default;
    let default = quote!(&[#(#defaults),*]);
    let hide = field.hide;
    let conflicts: Vec<String> = field
        .conflicts
        .iter()
        .map(|selector| {
            cli.field_for_selector(selector)
                .and_then(Cli::selector_for_field)
                .unwrap_or_else(|| selector.clone())
        })
        .collect();
    let canonical = |selector: &String| {
        cli.field_for_selector(selector)
            .and_then(Cli::selector_for_field)
            .unwrap_or_else(|| selector.clone())
    };
    let requires: Vec<String> = field.requires.iter().map(canonical).collect();
    let required_if: Vec<String> = field.required_if.iter().map(canonical).collect();
    let required_if_eq = field.required_if_eq.iter().map(|condition| {
        let selector = canonical(&condition.selector);
        let value = &condition.value;
        quote!(usage_argv::spec::RequiredIfEq { selector: #selector, value: #value })
    });
    let required_if_eq_all = field.required_if_eq_all.iter().map(|condition| {
        let selector = canonical(&condition.selector);
        let value = &condition.value;
        quote!(usage_argv::spec::RequiredIfEq { selector: #selector, value: #value })
    });
    let required_unless: Vec<String> = field.required_unless.iter().map(canonical).collect();
    let required_unless_all: Vec<String> =
        field.required_unless_all.iter().map(canonical).collect();
    // `String` must be filled; `Option` and `Vec` need not be.
    // A collecting field's type cannot say whether one value is needed, so `required` may
    // declare it. Every other shape gets its answer from the type.
    let required =
        (field.shape == Shape::Required || field.required_collection) && field.default.is_empty();
    let (choices, accepted_choices, choice_aliases, choice_details, ignore_case) =
        choices_tokens(field);
    let validate = option_str(field.validate.as_deref());
    let validate_error = option_str(field.validate_error.as_deref());
    let (var_min, var_max) = if matches!(field.kind, Kind::Flag { .. }) {
        value_bounds_tokens(field)
    } else {
        bounds_tokens(field)
    };
    let delimiter = match field.delimiter {
        Some(c) => quote!(::std::option::Option::Some(#c)),
        None => quote!(::std::option::Option::None),
    };
    let (completer_decl, completer) = completer_tokens(i, field, "arg", owner);

    quote! {
        #completer_decl
        pub static #name: usage_argv::spec::ArgMeta = usage_argv::spec::ArgMeta {
            complete: #completer,
            complete_type: #complete_type,
            arg: &#table,
            value_names: &[#(#value_names),*],
            help: #help,
            long_help: #long_help,
            env: #env,
            default: #default,
            help_heading: #help_heading,
            hide: #hide,
            conflicts: &[#(#conflicts),*],
            requires: &[#(#requires),*],
            required_if: &[#(#required_if),*],
            required_if_eq: &[#(#required_if_eq),*],
            required_if_eq_all: &[#(#required_if_eq_all),*],
            required_unless: &[#(#required_unless),*],
            required_unless_all: &[#(#required_unless_all),*],
            required: #required,
            accepted_choices: #accepted_choices,
            choices: #choices,
            choice_aliases: #choice_aliases,
            choice_details: #choice_details,
            ignore_case: #ignore_case,
            validate: #validate,
            validate_error: #validate_error,
            var_min: #var_min,
            var_max: #var_max,
            delimiter: #delimiter,
            ..usage_argv::spec::ArgMeta::EMPTY
        };
    }
}

/// A field's declared choices, as the metadata holds them.
fn choices_tokens(
    field: &Field,
) -> (
    TokenStream,
    TokenStream,
    TokenStream,
    TokenStream,
    TokenStream,
) {
    // From the type when the field says `value_enum`, so the spec, the help and the check
    // all read the list the type declares rather than a copy of it.
    if let (true, Some(ty)) = (field.value_enum, field.value_ty.as_ref()) {
        return (
            quote!(<#ty as usage_argv::spec::ValueEnum>::CHOICES),
            quote!(<#ty as usage_argv::spec::ValueEnum>::ACCEPTED_CHOICES),
            quote!(<#ty as usage_argv::spec::ValueEnum>::ALIASES),
            quote!(<#ty as usage_argv::spec::ValueEnum>::DETAILS),
            quote!(<#ty as usage_argv::spec::ValueEnum>::IGNORE_CASE),
        );
    }
    let choices = &field.choices;
    let choices = quote!(&[#(#choices),*]);
    (
        choices.clone(),
        choices,
        quote!(&[]),
        quote!(&[]),
        quote!(false),
    )
}

/// A field's declared bounds, as the metadata holds them.
fn bounds_tokens(field: &Field) -> (TokenStream, TokenStream) {
    let render = |bound: Option<usize>| match bound {
        Some(n) => quote!(::std::option::Option::Some(#n)),
        None => quote!(::std::option::Option::None),
    };
    (render(field.var_min), render(field.var_max))
}

/// Bounds on the values consumed by one flag occurrence.
fn value_bounds_tokens(field: &Field) -> (TokenStream, TokenStream) {
    let render = |bound: Option<usize>| match bound {
        Some(n) => quote!(::std::option::Option::Some(#n)),
        None => quote!(::std::option::Option::None),
    };
    (render(field.value_var_min), render(field.value_var_max))
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

/// A hash of the declaration a derive was handed, which is half of a key.
///
/// The other half is the module the declaration sits in, mixed in by
/// [`usage_argv::key_base`] where `module_path!()` is available — a macro cannot see a
/// module path, and without it two byte-identical declarations in different modules
/// collide. They used to: `Spec::to_kdl` asserts the tree holds no duplicate keys, so a
/// perfectly good CLI with an `add::Op` and a `remove::Op` failed that assertion.
fn declaration_hash(fingerprint: &str) -> u32 {
    // FNV-1a, spelled out rather than taken from a `Hasher`, which is not guaranteed to
    // be stable between compilations — and this ends up baked into generated code.
    let mut hash: u32 = 0x811c_9dc5;
    for byte in fingerprint.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// The `const` items a generated module needs before it can name a key.
///
/// One base, then one per flag, argument and command, because a key is used as a `match`
/// pattern as well as a table field — and a pattern cannot be an expression, so
/// `BASE | KIND_FLAG | 0` would parse as an or-pattern of three.
fn key_consts(fingerprint: &str, flags: usize, args: usize) -> TokenStream {
    let declaration = declaration_hash(fingerprint);
    let command = key_ident("COMMAND", None);
    let flag_keys = (0..flags).map(|i| {
        let name = key_ident("FLAG", Some(i));
        let index = i as u64;
        quote!(const #name: u64 = __USAGE_KEY_BASE | #KIND_FLAG | #index;)
    });
    let arg_keys = (0..args).map(|i| {
        let name = key_ident("ARG", Some(i));
        let index = i as u64;
        quote!(const #name: u64 = __USAGE_KEY_BASE | #KIND_ARG | #index;)
    });
    quote! {
        const __USAGE_KEY_BASE: u64 =
            usage_argv::key_base(::core::module_path!(), #declaration);
        const #command: u64 = __USAGE_KEY_BASE | #KIND_COMMAND;
        #(#flag_keys)*
        #(#arg_keys)*
    }
}

/// The name of the `const` holding one key.
fn key_ident(kind: &str, index: Option<usize>) -> proc_macro2::Ident {
    match index {
        Some(i) => format_ident!("__USAGE_KEY_{kind}_{i}"),
        None => format_ident!("__USAGE_KEY_{kind}"),
    }
}

/// The table expressions for one command, and whatever has to be declared to build them.
///
/// Without a `flatten` these are the plain slices the derive has always emitted, so nothing
/// about an existing CLI's generated code changes. With one, the flattened struct's tables
/// have to appear inside the parent's — at the position the field was written, because for
/// positional arguments the position is the meaning — which needs the const-concat helpers
/// and a `static` to hold each joined array.
struct Tables {
    /// Declared before the `Command`: the parse-table groups and their joined arrays.
    decls: TokenStream,
    /// Declared after the per-field metadata, which it reads — so it cannot share `decls`.
    meta_decls: TokenStream,
    flags: TokenStream,
    args: TokenStream,
    flag_metas: TokenStream,
    arg_metas: TokenStream,
}

/// Build the four expressions, splicing any flattened groups into place.
///
/// Used by both emitters. The first version of the typed-value conversion was wired into only
/// one of them, which compiled in the tests and not in an adopter's crate — so anything that
/// belongs to "what a command's tables are" goes here, once.
fn tables(cli: &Cli) -> Tables {
    // What a group is: a run of this struct's own declarations, or one flattened struct's
    // whole table. Walked in field order so the runs and the splices interleave correctly.
    let mut flag_groups: Vec<TokenStream> = Vec::new();
    let mut arg_groups: Vec<TokenStream> = Vec::new();
    let mut flag_meta_groups: Vec<TokenStream> = Vec::new();
    let mut arg_meta_groups: Vec<TokenStream> = Vec::new();
    let mut own_flags: Vec<usize> = Vec::new();
    let mut own_args: Vec<usize> = Vec::new();
    let (mut flag_at, mut arg_at) = (0usize, 0usize);
    let mut flattened = false;

    // Flush the runs collected so far, so that what follows lands after them.
    fn flush_flags(
        own: &mut Vec<usize>,
        groups: &mut Vec<TokenStream>,
        metas: &mut Vec<TokenStream>,
    ) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|i| {
            let name = format_ident!("FLAG_{i}");
            quote!(&#name)
        });
        groups.push(quote!(&[#(#refs),*]));
        let meta_refs = own.iter().map(|i| format_ident!("FLAG_META_{i}"));
        metas.push(quote!(&[#(#meta_refs),*]));
        own.clear();
    }
    fn flush_args(
        own: &mut Vec<usize>,
        groups: &mut Vec<TokenStream>,
        metas: &mut Vec<TokenStream>,
    ) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|i| {
            let name = format_ident!("ARG_{i}");
            quote!(&#name)
        });
        groups.push(quote!(&[#(#refs),*]));
        let meta_refs = own.iter().map(|i| format_ident!("ARG_META_{i}"));
        metas.push(quote!(&[#(#meta_refs),*]));
        own.clear();
    }

    for field in &cli.fields {
        match &field.kind {
            Kind::Flag { .. } => {
                own_flags.push(flag_at);
                flag_at += 1;
            }
            Kind::Arg { .. } => {
                own_args.push(arg_at);
                arg_at += 1;
            }
            Kind::Flatten { ty } => {
                flattened = true;
                flush_flags(&mut own_flags, &mut flag_groups, &mut flag_meta_groups);
                flush_args(&mut own_args, &mut arg_groups, &mut arg_meta_groups);
                flag_groups.push(quote!(<#ty as usage_argv::spec::CommandArgs>::COMMAND.flags));
                arg_groups.push(quote!(<#ty as usage_argv::spec::CommandArgs>::COMMAND.args));
                flag_meta_groups.push(quote!(<#ty as usage_argv::spec::CommandArgs>::META.flags));
                arg_meta_groups.push(quote!(<#ty as usage_argv::spec::CommandArgs>::META.args));
            }
            Kind::Subcommand { .. } | Kind::Skip => {}
        }
    }
    flush_flags(&mut own_flags, &mut flag_groups, &mut flag_meta_groups);
    flush_args(&mut own_args, &mut arg_groups, &mut arg_meta_groups);

    if !flattened {
        // The shape every CLI without a flatten already had.
        let flag_refs = (0..flag_at).map(|i| {
            let name = format_ident!("FLAG_{i}");
            quote!(&#name)
        });
        let arg_refs = (0..arg_at).map(|i| {
            let name = format_ident!("ARG_{i}");
            quote!(&#name)
        });
        let flag_meta_refs = (0..flag_at).map(|i| format_ident!("FLAG_META_{i}"));
        let arg_meta_refs = (0..arg_at).map(|i| format_ident!("ARG_META_{i}"));
        return Tables {
            decls: TokenStream::new(),
            meta_decls: TokenStream::new(),
            flags: quote!(&[#(#flag_refs),*]),
            args: quote!(&[#(#arg_refs),*]),
            flag_metas: quote!(&[#(#flag_meta_refs),*]),
            arg_metas: quote!(&[#(#arg_meta_refs),*]),
        };
    }

    Tables {
        decls: quote! {
            const FLAG_GROUPS: &[&[&usage_argv::Flag<'static>]] = &[#(#flag_groups),*];
            const ARG_GROUPS: &[&[&usage_argv::Arg<'static>]] = &[#(#arg_groups),*];
            static FLAGS: [&usage_argv::Flag<'static>;
                usage_argv::table_len(FLAG_GROUPS)] =
                usage_argv::concat_flags(FLAG_GROUPS);
            static ARGS: [&usage_argv::Arg<'static>; usage_argv::table_len(ARG_GROUPS)] =
                usage_argv::concat_args(ARG_GROUPS);
        },
        meta_decls: quote! {
            const FLAG_META_GROUPS: &[&[usage_argv::spec::FlagMeta<'static>]] =
                &[#(#flag_meta_groups),*];
            const ARG_META_GROUPS: &[&[usage_argv::spec::ArgMeta<'static>]] =
                &[#(#arg_meta_groups),*];
            static FLAG_METAS: [usage_argv::spec::FlagMeta<'static>;
                usage_argv::table_len(FLAG_META_GROUPS)] =
                usage_argv::spec::concat_flag_metas(FLAG_META_GROUPS);
            static ARG_METAS: [usage_argv::spec::ArgMeta<'static>;
                usage_argv::table_len(ARG_META_GROUPS)] =
                usage_argv::spec::concat_arg_metas(ARG_META_GROUPS);
        },
        flags: quote!(&FLAGS),
        args: quote!(&ARGS),
        flag_metas: quote!(&FLAG_METAS),
        arg_metas: quote!(&ARG_METAS),
    }
}

fn flag_arm(cli: &Cli, i: usize, field: &Field) -> TokenStream {
    let key = key_ident("FLAG", Some(i));
    let ident = &field.ident;
    let given = format_ident!("__given_{}", ident);
    let displaced = displacements(cli, field);
    let undisplaced = is_displaceable(cli, field).then(|| {
        let overridden = format_ident!("__overridden_{}", ident);
        quote!(partial.#overridden = false;)
    });
    let duplicate = rejects_duplicate(field).then(|| {
        let duplicated = format_ident!("__duplicated_{}", ident);
        if has_negate(field) {
            let negated = format_ident!("__negated_{}", ident);
            // For a global the question is asked per level, exactly as in the arm
            // below: clap accepts `--colour sub --colour` when `--colour` is global
            // and negatable just as when it is plain.
            let (guard, mark) = if duplicates_per_level(field) {
                let here = format_ident!("__here_{}", ident);
                (quote!(partial.#here), quote!(partial.#here = true;))
            } else {
                (quote!(partial.#given), TokenStream::new())
            };
            quote! {
                if #guard {
                    // The positive and negative spellings override one another: the
                    // last of `--color --no-color` wins just like an explicit
                    // `overrides` pair. Repeating the same spelling is still an error.
                    partial.#duplicated = partial.#negated == negated;
                }
                #mark
                partial.#negated = negated;
            }
        } else if duplicates_per_level(field) {
            let here = format_ident!("__here_{}", ident);
            quote! {
                if partial.#here {
                    partial.#duplicated = true;
                }
                partial.#here = true;
            }
        } else {
            quote! {
                if partial.#given {
                    partial.#duplicated = true;
                }
            }
        }
    });
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
        Shape::Many => match field.delimiter {
            Some(delimiter) => {
                let byte = u8::try_from(u32::from(delimiter))
                    .expect("the model rejects non-ASCII delimiters");
                quote! {
                    let value = __usage_value_text(value);
                    for part in value.split(|b| *b == #byte) {
                        partial.#ident.push(part.to_vec());
                    }
                }
            }
            None => quote!(partial.#ident.push(__usage_value_text(value));),
        },
    };
    let table = format_ident!("FLAG_{i}");
    quote! {
        // The key gets us to the right arm in one jump; the identity check makes a
        // collision harmless rather than wrong. Two identical declarations in
        // different modules hash alike — a macro cannot see a module path — and
        // without this, one command's flag would fill another's field. `static` items
        // have distinct addresses, so this is exact.
        #key if ::core::ptr::eq(*flag, &#table) => {
            #duplicate
            #body
            partial.#given = true;
            // Given again after having lost: it is standing once more, which matters
            // when the flags alternate — `--include a --all --include b`.
            #undisplaced
            // Whatever this flag displaces, undone here rather than after the parse:
            // `overrides` is about which of two flags came last, and the token that
            // just arrived is the last one so far.
            #(#displaced)*
            true
        }
    }
}

/// Whether a repeat of this flag is only a duplicate *within one command*.
///
/// A `global` flag is in scope for every descendant, and clap lets it be given again on a
/// subcommand — the inner occurrence simply wins, which is what makes `mise -y install -y` a
/// line that works today. Repeating it at *one* level is still an error there, so the check
/// cannot simply be dropped: it has to be per level, which is what `__here_` records.
fn duplicates_per_level(field: &Field) -> bool {
    rejects_duplicate(field) && matches!(field.kind, Kind::Flag { global: true, .. })
}

/// Clearing those markers, to run when a command word is read: descending starts a new level.
fn reset_per_level(cli: &Cli) -> TokenStream {
    let resets = cli
        .fields
        .iter()
        .filter(|f| duplicates_per_level(f))
        .map(|f| {
            let here = format_ident!("__here_{}", f.ident);
            quote!(partial.#here = false;)
        });
    quote!(#(#resets)*)
}

/// Whether another occurrence is a command-line mistake rather than another value.
///
/// Counts and collections repeat by definition, and `var` explicitly opts a value-taking flag
/// into repetition. Every other flag matches clap's default of one occurrence.
///
/// **This is stricter than the grammar, deliberately.** The corpus specifies the opposite for a
/// *spec-driven* parse — `long-repeated-keeps-the-last` says "a repeat is a correction, typically
/// a wrapper appending to a command line it did not write" — and usage-argv's parser and
/// usage-lib both honour that. The rule lives here, in the derive's post-binding layer, and
/// nowhere else: `Error::DuplicateFlag` is constructed at one call site, in this file.
///
/// Kept, because the two cases are not the same case. A spec parsed at run time may well be
/// describing someone else's command line, which is the wrapper the corpus has in mind. A
/// derived CLI is an authored, fixed surface, where `--jobs 1 --jobs 2` is far likelier to be a
/// mistake than an amendment — and it is what clap does, so an adopter replacing clap sees no
/// change. Dropping it would make a derived binary quietly accept a line clap rejects today,
/// which is the direction that costs an adopter rather than the one that helps.
///
/// Recorded because it looks like an inconsistency and is not one. `differential.rs` in the gate
/// carries the same note from the other end.
fn rejects_duplicate(field: &Field) -> bool {
    matches!(field.kind, Kind::Flag { .. })
        && !matches!(field.shape, Shape::Count | Shape::Many)
        && !field.repeatable
}

/// Whether a boolean flag has a negative spelling that overrides its positive one.
fn has_negate(field: &Field) -> bool {
    matches!(
        &field.kind,
        Kind::Flag {
            negate: Some(_),
            ..
        }
    )
}

/// Undoing the flags a flag displaces, as statements to run once it has been bound.
///
/// Both directions. `overrides` is declared on one flag and holds between the two:
/// clap resolves `--file a --stdin` and `--stdin --file a` the same way whichever side
/// declared it, and so does usage-lib.
fn displacements(cli: &Cli, field: &Field) -> Vec<TokenStream> {
    let mut displacements: Vec<TokenStream> = displaced_by(cli, field)
        .into_iter()
        .map(displace_statement)
        .collect();
    for selector in field
        .overrides
        .iter()
        .filter(|selector| cli.field_for_selector(selector).is_none())
    {
        for flattened in &cli.fields {
            let Kind::Flatten { ty } = &flattened.kind else {
                continue;
            };
            let ident = &flattened.ident;
            displacements.push(quote! {
                let _ = <#ty as usage_argv::spec::CommandArgs>::displace(
                    &mut partial.#ident,
                    #selector,
                );
            });
        }
    }
    displacements
}

fn displace_statement(field: &Field) -> TokenStream {
    let reset = reset_to_default(field);
    let given = format_ident!("__given_{}", field.ident);
    let overridden = format_ident!("__overridden_{}", field.ident);
    let duplicated = rejects_duplicate(field).then(|| {
        let duplicated = format_ident!("__duplicated_{}", field.ident);
        quote!(partial.#duplicated = false;)
    });
    quote! {
        #reset
        partial.#given = false;
        #duplicated
        // Remembered, not just cleared: without this the environment fallback
        // would refill the flag that lost and mark it given again, and a
        // displaced `String` would be reported missing. usage-lib keeps the
        // same set for the same reason.
        partial.#overridden = true;
    }
}

/// The fields a flag displaces, in both directions.
fn displaced_by<'a>(cli: &'a Cli, field: &Field) -> Vec<&'a Field> {
    let names = |target: &Field, selectors: &[String]| {
        selectors.iter().any(|selector| {
            cli.field_for_selector(selector)
                .is_some_and(|named| named.ident == target.ident)
        })
    };
    cli.fields
        .iter()
        .filter(|other| {
            other.ident != field.ident
                && (names(other, &field.overrides) || names(field, &other.overrides))
        })
        .collect()
}

/// Whether a field is on either side of an `overrides`, and so needs somewhere to
/// record that it lost.
fn is_displaceable(cli: &Cli, field: &Field) -> bool {
    (cli.composable && matches!(field.kind, Kind::Flag { .. }))
        || !field.overrides.is_empty()
        || !displaced_by(cli, field).is_empty()
}

fn arg_arm(i: usize, field: &Field) -> TokenStream {
    let key = key_ident("ARG", Some(i));
    let ident = &field.ident;
    let given = format_ident!("__given_{}", ident);
    let body = match field.shape {
        Shape::Many => match field.delimiter {
            Some(delimiter) => {
                let byte = u8::try_from(u32::from(delimiter))
                    .expect("the model rejects non-ASCII delimiters");
                quote! {
                    if delimit {
                        for part in value.split(|b| *b == #byte) {
                            partial.#ident.push(__usage_text(part));
                        }
                    } else {
                        partial.#ident.push(__usage_text(value));
                    }
                }
            }
            None => quote!(partial.#ident.push(__usage_text(value));),
        },
        Shape::Optional => quote! {
            partial.#ident = ::std::option::Option::Some(__usage_text(value));
        },
        _ => quote!(partial.#ident = __usage_text(value);),
    };
    let table = format_ident!("ARG_{i}");
    quote! {
        #key if ::core::ptr::eq(*arg, &#table) => {
            #body
            partial.#given = true;
            true
        }
    }
}

fn option_str(value: Option<&str>) -> TokenStream {
    match value {
        Some(v) => quote!(::std::option::Option::Some(#v)),
        None => quote!(::std::option::Option::None),
    }
}

fn option_expr(value: Option<&TokenStream>) -> TokenStream {
    match value {
        Some(v) => quote!(::std::option::Option::Some(#v)),
        None => quote!(::std::option::Option::None),
    }
}

/// The presence summaries a parent needs to enforce exclusivity across a flattened
/// `CommandArgs` boundary.
fn presence_methods(cli: &Cli) -> TokenStream {
    let direct_given = cli.fields.iter().filter_map(|field| {
        if matches!(
            field.kind,
            Kind::Flatten { .. } | Kind::Subcommand { .. } | Kind::Skip
        ) {
            return None;
        }
        let given = format_ident!("__given_{}", field.ident);
        let name = &field.name;
        Some(quote! {
            if partial.#given {
                return ::std::option::Option::Some(#name);
            }
        })
    });
    let flattened_given = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            if let ::std::option::Option::Some(name) =
                <#ty as usage_argv::spec::CommandArgs>::any_given(&partial.#ident)
            {
                return ::std::option::Option::Some(name);
            }
        })
    });
    let selected = cli.fields.iter().find_map(|field| {
        if !matches!(field.kind, Kind::Subcommand { .. }) {
            return None;
        }
        let name = &field.name;
        Some(quote! {
            if partial.__usage_selected.is_some() {
                return ::std::option::Option::Some(#name);
            }
        })
    });
    let direct_exclusive = cli.fields.iter().filter_map(|field| {
        if !field.exclusive {
            return None;
        }
        let given = format_ident!("__given_{}", field.ident);
        let name = &field.name;
        Some(quote! {
            if partial.#given {
                return ::std::option::Option::Some(#name);
            }
        })
    });
    let flattened_exclusive = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            if let ::std::option::Option::Some(name) =
                <#ty as usage_argv::spec::CommandArgs>::exclusive_given(&partial.#ident)
            {
                return ::std::option::Option::Some(name);
            }
        })
    });
    let selected_exclusive = cli.fields.iter().find_map(|field| {
        let Kind::Subcommand { ty, .. } = &field.kind else {
            return None;
        };
        Some(quote! {
            if let ::std::option::Option::Some(name) =
                <#ty as usage_argv::spec::Subcommands>::exclusive_given(
                    &partial.__usage_sub,
                    partial.__usage_selected,
                )
            {
                return ::std::option::Option::Some(name);
            }
        })
    });

    quote! {
        fn any_given(partial: &Self::Partial) -> ::std::option::Option<&'static str> {
            #(#direct_given)*
            #(#flattened_given)*
            #selected
            ::std::option::Option::None
        }

        fn exclusive_given(partial: &Self::Partial) -> ::std::option::Option<&'static str> {
            #(#direct_exclusive)*
            #(#flattened_exclusive)*
            #selected_exclusive
            ::std::option::Option::None
        }

        fn argument_state(
            partial: &Self::Partial,
            selector: &str,
        ) -> ::std::option::Option<usage_argv::spec::ArgumentState> {
            argument_state(partial, selector)
        }

        fn argument_matches(
            partial: &Self::Partial,
            selector: &str,
            value: &[u8],
        ) -> ::std::option::Option<bool> {
            argument_matches(partial, selector, value)
        }

        fn displace(partial: &mut Self::Partial, selector: &str) -> bool {
            displace(partial, selector)
        }

        fn event_matches(
            event: &usage_argv::Event<'_, '_, '_>,
            selector: &str,
        ) -> bool {
            event_matches(event, selector)
        }
    }
}

fn field_selectors(field: &Field) -> Vec<String> {
    match &field.kind {
        Kind::Flag {
            longs,
            shorts,
            negate,
            ..
        } => longs
            .iter()
            .map(|long| format!("--{long}"))
            .chain(shorts.iter().map(|short| format!("-{short}")))
            .chain(negate.iter().map(|long| format!("--{long}")))
            .collect(),
        Kind::Arg { .. } => {
            let inferred = to_kebab(&field.ident.to_string());
            if inferred == field.name {
                vec![field.name.clone()]
            } else {
                vec![field.name.clone(), inferred]
            }
        }
        _ => Vec::new(),
    }
}

/// Selector lookup composed across flattened argument groups.
///
/// A parent cannot inspect another derive expansion's fields, so the flattened type answers
/// through `CommandArgs`. Keeping the lookup beside the partial avoids building a dynamic
/// command graph or allocating on the successful parse path.
fn argument_lookup_functions(cli: &Cli) -> TokenStream {
    let state_arms = cli.fields.iter().filter_map(|field| {
        if matches!(
            field.kind,
            Kind::Flatten { .. } | Kind::Subcommand { .. } | Kind::Skip
        ) {
            return None;
        }
        let selectors = field_selectors(field);
        let ident = &field.ident;
        let given = format_ident!("__given_{}", ident);
        let name = &field.name;
        // Look through conditional defaults as well as unconditional ones. This lookup
        // runs after `apply_defaults` during relationship validation, so a predicate
        // that filled the field is still observable even though defaults deliberately
        // do not set `__given_*`.
        let defaulted = !field.default.is_empty();
        let conditionally_defaulted =
            default_if_would_apply(cli, field).unwrap_or_else(|| quote!(false));
        Some(quote! {
            #(#selectors)|* => return ::std::option::Option::Some(
                usage_argv::spec::ArgumentState {
                    name: #name,
                    given: partial.#given,
                    satisfied: partial.#given || #defaulted || (#conditionally_defaulted),
                },
            ),
        })
    });
    let state_flattened = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            if let ::std::option::Option::Some(state) =
                <#ty as usage_argv::spec::CommandArgs>::argument_state(
                    &partial.#ident,
                    selector,
                )
            {
                return ::std::option::Option::Some(state);
            }
        })
    });
    let match_arms = cli.fields.iter().filter_map(|field| {
        if matches!(
            field.kind,
            Kind::Flatten { .. } | Kind::Subcommand { .. } | Kind::Skip
        ) {
            return None;
        }
        let selectors = field_selectors(field);
        let ident = &field.ident;
        let given = format_ident!("__given_{}", ident);
        let matches = match field.shape {
            Shape::Optional => quote!(partial.#ident.as_deref().is_some_and(|v| v == value)),
            Shape::Required => quote!(partial.#ident.as_slice() == value),
            Shape::Many => quote!(partial.#ident.iter().any(|v| v.as_slice() == value)),
            Shape::Bool => quote!(
                (value == b"true" && partial.#ident) || (value == b"false" && !partial.#ident)
            ),
            Shape::Count => quote!(
                (value == b"true" && partial.#ident > 0)
                    || (value == b"false" && partial.#ident == 0)
            ),
        };
        Some(quote!(#(#selectors)|* => return ::std::option::Option::Some(partial.#given && #matches),))
    });
    let match_flattened = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            if let ::std::option::Option::Some(matches) =
                <#ty as usage_argv::spec::CommandArgs>::argument_matches(
                    &partial.#ident,
                    selector,
                    value,
                )
            {
                return ::std::option::Option::Some(matches);
            }
        })
    });
    let displace_arms = cli.fields.iter().filter_map(|field| {
        if !matches!(field.kind, Kind::Flag { .. }) || !is_displaceable(cli, field) {
            return None;
        }
        let selectors = field_selectors(field);
        let statement = displace_statement(field);
        Some(quote! {
            #(#selectors)|* => {
                #statement
                return true;
            }
        })
    });
    let displace_flattened = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            if <#ty as usage_argv::spec::CommandArgs>::displace(
                &mut partial.#ident,
                selector,
            ) {
                return true;
            }
        })
    });
    let flags: Vec<&Field> = cli
        .fields
        .iter()
        .filter(|field| matches!(field.kind, Kind::Flag { .. }))
        .collect();
    let event_arms = flags.iter().enumerate().map(|(i, field)| {
        let key = key_ident("FLAG", Some(i));
        let table = format_ident!("FLAG_{i}");
        let selectors = field_selectors(field);
        quote! {
            #key if ::core::ptr::eq(*flag, &#table) => {
                matches!(selector, #(#selectors)|*)
            }
        }
    });
    let event_flattened = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        Some(quote! {
            if <#ty as usage_argv::spec::CommandArgs>::event_matches(event, selector) {
                return true;
            }
        })
    });
    quote! {
        #[allow(dead_code)]
        pub fn argument_state(
            partial: &Partial,
            selector: &str,
        ) -> ::std::option::Option<usage_argv::spec::ArgumentState> {
            match selector {
                #(#state_arms)*
                _ => {}
            }
            #(#state_flattened)*
            ::std::option::Option::None
        }

        #[allow(dead_code)]
        pub fn argument_matches(
            partial: &Partial,
            selector: &str,
            value: &[u8],
        ) -> ::std::option::Option<bool> {
            match selector {
                #(#match_arms)*
                _ => {}
            }
            #(#match_flattened)*
            ::std::option::Option::None
        }

        #[allow(dead_code)]
        pub fn displace(partial: &mut Partial, selector: &str) -> bool {
            match selector {
                #(#displace_arms)*
                _ => {}
            }
            #(#displace_flattened)*
            false
        }

        #[allow(dead_code)]
        pub fn event_matches(
            event: &usage_argv::Event<'_, '_, '_>,
            selector: &str,
        ) -> bool {
            if let usage_argv::Event::Flag { flag, .. } = event {
                let matched = match flag.key {
                    #(#event_arms)*
                    _ => false,
                };
                if matched {
                    return true;
                }
            }
            #(#event_flattened)*
            false
        }
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
        if matches!(f.kind, Kind::Subcommand { .. } | Kind::Skip) {
            // Its values live in the enum's own partial. A skipped field is not parsed
            // at all, so it has nothing to accumulate.
            return None;
        }
        // A flattened struct accumulates into its own partial, whose shape only its derive
        // knows — reached through the trait, like everything else about it.
        if let Kind::Flatten { ty } = &f.kind {
            let ident = &f.ident;
            return Some(quote! {
                pub #ident: <#ty as usage_argv::spec::CommandArgs>::Partial,
            });
        }
        let ident = &f.ident;
        let ty = match f.shape {
            Shape::Bool => quote!(bool),
            Shape::Count => {
                let ty = &f.ty;
                quote!(#ty)
            }
            // The bytes as typed, rather than text: `apply` cannot fail — it answers
            // whether an event was this command's — so a word that is not valid UTF-8
            // cannot be reported when it arrives. Keeping the bytes lets `build` report it,
            // and it means no value is quietly mangled on the way in.
            Shape::Optional => quote!(::std::option::Option<::std::vec::Vec<u8>>),
            Shape::Required => quote!(::std::vec::Vec<u8>),
            Shape::Many => quote!(::std::vec::Vec<::std::vec::Vec<u8>>),
        };
        let given = format_ident!("__given_{}", ident);
        // Whether a token supplied this, as opposed to a default sitting in it: an
        // empty string is a value somebody typed, and `--jobs=` has to be able to
        // mean it.
        // Only for a flag that can lose an override: every other field would carry a
        // `bool` nothing ever reads.
        let overridden = is_displaceable(cli, f).then(|| {
            let overridden = format_ident!("__overridden_{}", ident);
            quote!(pub #overridden: bool,)
        });
        let duplicated = rejects_duplicate(f).then(|| {
            let duplicated = format_ident!("__duplicated_{}", ident);
            quote!(pub #duplicated: bool,)
        });
        // Given at *this* level, for a global — see `duplicates_per_level`. Only for those
        // fields, so nothing else carries a `bool` no code reads.
        let here = duplicates_per_level(f).then(|| {
            let here = format_ident!("__here_{}", ident);
            quote!(pub #here: bool,)
        });
        let negated = has_negate(f).then(|| {
            let negated = format_ident!("__negated_{}", ident);
            quote!(pub #negated: bool,)
        });
        Some(quote!(pub #ident: #ty, pub #given: bool, #overridden #duplicated #here #negated))
    });

    // No derived `Default`: `start` is what produces a fresh partial, because a
    // declared default has to be in place before parsing begins and nested state has
    // its own starting values.
    quote! {
        pub struct Partial {
            #(#fields)*
            #sub
        }
    }
}

/// The pieces one command contributes to a settings resolution.
///
/// Two of them, and the split is what lets a group declare a setting at all. `given` and
/// `bindings` name nothing outside `usage-argv`, so every command can carry them — a flattened
/// group hands its parent what it was given, in a vocabulary the parser crate owns. Only the root
/// turns that into a `usage_config::CliLayer`, which is why a program with no settings still never
/// mentions the config crate.
struct Settings {
    given: TokenStream,
    bindings: TokenStream,
}

/// One field's value, in usage-argv's vocabulary.
///
/// Text, because that is what the partial holds and what a declared type expects to read:
/// rendering it to something typed here would be a second coercion, disagreeing with the
/// registry's own at the first setting whose type this cannot see. Bytes that are not text are
/// said rather than rendered — an argument can hold them and a setting cannot.
fn given_value(field: &Field) -> TokenStream {
    let ident = &field.ident;
    match field.shape {
        // The bool the parser landed on, which is `false` for a negation and `true` for the flag
        // itself: what the user said, rather than that they said something.
        Shape::Bool => quote! {
            usage_argv::spec::SettingGiven::Bool(partial.#ident)
        },
        Shape::Count => quote! {
            usage_argv::spec::SettingGiven::Int(
                ::std::convert::TryFrom::try_from(partial.#ident)
                    .unwrap_or(::std::primitive::i64::MAX),
            )
        },
        Shape::Optional => quote! {
            match ::std::str::from_utf8(partial.#ident.as_deref().unwrap_or_default()) {
                ::std::result::Result::Ok(__usage_text) => {
                    usage_argv::spec::SettingGiven::Text(
                        ::std::string::ToString::to_string(__usage_text),
                    )
                }
                ::std::result::Result::Err(_) => usage_argv::spec::SettingGiven::NotText,
            }
        },
        Shape::Required => quote! {
            match ::std::str::from_utf8(&partial.#ident) {
                ::std::result::Result::Ok(__usage_text) => {
                    usage_argv::spec::SettingGiven::Text(
                        ::std::string::ToString::to_string(__usage_text),
                    )
                }
                ::std::result::Result::Err(_) => usage_argv::spec::SettingGiven::NotText,
            }
        },
        // Item by item, rather than joined and re-split: an item holding the separator would come
        // back as two. One item that is not text costs the whole list, since contributing the
        // others would quietly drop the one the user typed.
        Shape::Many => quote! {
            match partial
                .#ident
                .iter()
                .map(|__usage_item| {
                    ::std::str::from_utf8(__usage_item)
                        .ok()
                        .map(::std::string::ToString::to_string)
                })
                .collect::<::std::option::Option<::std::vec::Vec<_>>>()
            {
                ::std::option::Option::Some(__usage_items) => {
                    usage_argv::spec::SettingGiven::List(__usage_items)
                }
                ::std::option::Option::None => usage_argv::spec::SettingGiven::NotText,
            }
        },
    }
}

/// Every spelling a bound flag answers to, paired with what it sets.
///
/// A positional has no flag, so it contributes to the layer and not to this: there is nothing for
/// a spec's `cli` node to have declared, and nothing for `drift` to compare.
fn own_bindings(bound: &[&Field]) -> Vec<TokenStream> {
    bound
        .iter()
        .flat_map(|field| {
            let key = field.setting.clone().unwrap_or_default();
            let mut spellings = Vec::new();
            if let Kind::Flag {
                longs,
                shorts,
                negate,
                ..
            } = &field.kind
            {
                spellings.extend(longs.iter().map(|long| format!("--{long}")));
                spellings.extend(shorts.iter().map(|short| format!("-{short}")));
                if let Some(negate) = negate {
                    spellings.push(format!("--{negate}"));
                }
            }
            spellings.into_iter().map(move |flag| quote!((#flag, #key)))
        })
        .collect()
}

/// The commands whose settings this one carries: each flattened group, and the subcommands.
///
/// As `(bindings, given)` pairs, because the two questions differ for a subcommand: its bindings
/// are every variant's, since a table says what the CLI *can* do, and its values are the selected
/// variant's, since those are about one invocation.
fn children(cli: &Cli) -> Vec<(TokenStream, TokenStream)> {
    cli.fields
        .iter()
        .filter_map(|field| {
            let ident = &field.ident;
            match &field.kind {
                Kind::Flatten { ty } => Some((
                    quote!(<#ty as usage_argv::spec::CommandArgs>::SETTINGS_BINDINGS),
                    quote! {
                        <#ty as usage_argv::spec::CommandArgs>::settings_given(
                            &partial.#ident,
                        )
                    },
                )),
                Kind::Subcommand { ty, .. } => Some((
                    quote!(<#ty as usage_argv::spec::Subcommands>::SETTINGS_BINDINGS),
                    quote! {
                        <#ty as usage_argv::spec::Subcommands>::settings_given(
                            &partial.__usage_sub,
                            partial.__usage_selected,
                        )
                    },
                )),
                _ => None,
            }
        })
        .collect()
}

/// Several binding tables as one, joined at compile time.
///
/// The plain slice when there is nothing to join, which is every CLI that flattens nothing: the
/// joined form costs a const block and an array, and reads worse in an expansion.
fn joined_bindings(own: &[TokenStream], children: &[TokenStream]) -> TokenStream {
    if children.is_empty() {
        return quote!(&[#(#own),*]);
    }
    quote! {
        {
            const OWN: &[(&'static str, &'static str)] = &[#(#own),*];
            const PARTS: &[&'static [(&'static str, &'static str)]] = &[OWN #(, #children)*];
            const N: usize = OWN.len() #(+ #children.len())*;
            const JOINED: [(&'static str, &'static str); N] =
                usage_argv::spec::concat_bindings(PARTS);
            &JOINED
        }
    }
}

/// What this command says about settings, or `None` when it has nothing to say.
///
/// A group with no bindings of its own still has something to say when it flattens one that does,
/// which is why this is not simply "does any field declare `setting`".
fn settings(cli: &Cli) -> Option<Settings> {
    let bound: Vec<&Field> = cli.fields.iter().filter(|f| f.setting.is_some()).collect();
    let children = children(cli);
    if bound.is_empty() && children.is_empty() {
        return None;
    }

    // Built from the *partial* rather than from the struct, and gated on `__given_`, because that
    // is the only place the difference between "the flag was not passed" and "the flag was passed
    // and said no" survives. Reading the struct, `--no-colour` is a `false` indistinguishable from
    // absence — and the command line outranks every file on the machine, so guessing either way is
    // a value the user never asked for.
    let contributions = bound.iter().map(|field| {
        let given = format_ident!("__given_{}", &field.ident);
        let key = field.setting.as_deref().unwrap_or_default();
        let value = given_value(field);
        quote! {
            if partial.#given {
                __usage_given.push((#key, #value));
            }
        }
    });
    let from_children = children
        .iter()
        .map(|(_, given)| quote!(__usage_given.extend(#given);));

    let given = quote! {
        /// The settings this command line gave values for.
        pub fn settings_given(
            partial: &Partial,
        ) -> ::std::vec::Vec<(&'static str, usage_argv::spec::SettingGiven)> {
            let mut __usage_given = ::std::vec::Vec::new();
            #(#contributions)*
            #(#from_children)*
            __usage_given
        }
    };

    let child_bindings: Vec<TokenStream> = children.into_iter().map(|(b, _)| b).collect();
    let pairs = own_bindings(&bound);
    let table = joined_bindings(&pairs, &child_bindings);
    let bindings = quote! {
        /// Every flag this CLI reads into a setting, and the setting it sets.
        ///
        /// What `usage_config::Registry::drift` compares against the flags the spec *declares*, so
        /// a documented flag nothing reads — hk has thirteen — fails a test rather than a user.
        pub const SETTINGS_BINDINGS: &'static [(&'static str, &'static str)] = #table;
    };

    Some(Settings { given, bindings })
}

/// The one place a `usage_config` type is named: the root's conversion.
///
/// One loop over what every command contributed, so a flattened group's value and the root's own
/// become entries the same way. A second conversion is the thing this whole stack keeps deleting.
fn settings_layer() -> TokenStream {
    quote! {
        /// This command line as a layer, for `usage_config::resolve`.
        pub fn settings_layer(partial: &Partial) -> ::usage_config::CliLayer {
            let mut __usage_layer = ::usage_config::CliLayer::new(
                ::std::iter::empty::<(::std::string::String, ::std::string::String)>(),
            );
            for (__usage_key, __usage_given) in settings_given(partial) {
                __usage_layer = match __usage_given {
                    usage_argv::spec::SettingGiven::Bool(__usage_value) => {
                        __usage_layer.with_value(__usage_key, ::usage_config::Value::Bool(__usage_value))
                    }
                    usage_argv::spec::SettingGiven::Int(__usage_value) => {
                        __usage_layer.with_value(__usage_key, ::usage_config::Value::Int(__usage_value))
                    }
                    usage_argv::spec::SettingGiven::Text(__usage_value) => {
                        __usage_layer
                            .with_value(__usage_key, ::usage_config::Value::String(__usage_value))
                    }
                    usage_argv::spec::SettingGiven::List(__usage_items) => __usage_layer.with_value(
                        __usage_key,
                        ::usage_config::Value::List(
                            __usage_items
                                .into_iter()
                                .map(::usage_config::Value::String)
                                .collect(),
                        ),
                    ),
                    usage_argv::spec::SettingGiven::NotText => {
                        __usage_layer.with_unrepresentable(__usage_key)
                    }
                };
            }
            __usage_layer
        }
    }
}

/// The check that stands in for a root that never said it has settings.
///
/// A root cannot see another struct's fields, so a CLI whose only bound flags live in a flattened
/// group would generate no settings entry points at all — the flag would parse and set nothing,
/// which is the silence this attribute exists to prevent. Generating them for every CLI instead is
/// not an option: that would make a program with subcommands and no settings depend on
/// `usage-config`. So the root asks each child, at compile time, whether it binds anything it is
/// not going to collect.
fn settings_guard(cli: &Cli) -> Option<TokenStream> {
    let children = children(cli);
    if children.is_empty() {
        return None;
    }
    let checks = children.into_iter().map(|(bindings, _)| {
        quote! {
            const _: () = assert!(
                #bindings.len() == 0,
                "this command flattens or nests a group that binds a setting, and does not \
                 collect it: add `#[usage(settings)]` to the struct deriving `Cli`",
            );
        }
    });
    Some(quote!(#(#checks)*))
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
        if matches!(f.kind, Kind::Subcommand { .. } | Kind::Skip) {
            return None;
        }
        // `start()` rather than `Default`, so the flattened struct's own defaults are in
        // place before parsing — the same reason this function exists at all.
        if let Kind::Flatten { ty } = &f.kind {
            let ident = &f.ident;
            return Some(quote! {
                #ident: <#ty as usage_argv::spec::CommandArgs>::start(),
            });
        }
        let ident = &f.ident;
        let given = format_ident!("__given_{}", ident);
        let overridden = is_displaceable(cli, f).then(|| {
            let overridden = format_ident!("__overridden_{}", ident);
            quote!(#overridden: false,)
        });
        let duplicated = rejects_duplicate(f).then(|| {
            let duplicated = format_ident!("__duplicated_{}", ident);
            quote!(#duplicated: false,)
        });
        let here = duplicates_per_level(f).then(|| {
            let here = format_ident!("__here_{}", ident);
            quote!(#here: false,)
        });
        let negated = has_negate(f).then(|| {
            let negated = format_ident!("__negated_{}", ident);
            quote!(#negated: false,)
        });
        Some(quote! {
            #ident: ::std::default::Default::default(),
            #given: false,
            #overridden
            #duplicated
            #here
            #negated
        })
    });
    // Only the fields that declare one: `Partial`'s own initializer has already put
    // everything else at `Default::default()`, and a subcommand field holds a partial
    // that `#sub_starts` builds rather than a value.
    quote! {
        let mut partial = Partial {
            #(#plain)*
            #sub_starts
        };
    }
}

/// One field of the finished struct, converted from the text the parse collected.
///
/// The partial holds words, because binding decides where a word lands and not what it
/// means. This is where meaning arrives: a `String` field takes the text as it is, and
/// anything else is built with `FromStr` — which is what lets a field be a `PathBuf`, a
/// number, or a type of the adopter's own.
fn field_final(field: &Field) -> TokenStream {
    let ident = &field.ident;
    let name = &field.name;
    if matches!(field.kind, Kind::Skip) {
        // clap's skip: not parsed, filled from Default when the struct is built.
        return quote!(#ident: ::std::default::Default::default());
    }
    if let Kind::Flatten { ty } = &field.kind {
        // Built by its own derive, which is also what makes a nested flatten work: this is
        // the same call at every level.
        //
        return quote! {
            #ident: <#ty as usage_argv::spec::CommandArgs>::build(partial.#ident)?
        };
    }
    let Some(ty) = field.value_ty.as_ref() else {
        // A switch or a count: nothing was parsed from a word.
        return quote!(#ident: partial.#ident);
    };

    // Every type converts, `String` included: the partial holds the bytes that were typed,
    // and `String::from_utf8` is where a word that is not UTF-8 is reported rather than
    // quietly replaced. That also retires the old hazard of recognising `String` by how it
    // was written — there is no identity case left to recognise, so an adopter who shadows
    // the name is no longer a problem.
    //
    // `from_utf8` takes the `Vec` by value and does not copy, so this costs a check.
    // `String` still skips the *second* step, since `from_utf8` has already produced one.
    // Recognising it by spelling is safe now: if an adopter's own `String` were mistaken for
    // this one, the mismatch is a compile error rather than a value quietly mangled — and
    // the check that matters, the UTF-8 one, happens either way.
    let rendered = rendered_path(ty);
    let is_std_string = matches!(
        rendered.as_str(),
        "String" | "std::string::String" | "::std::string::String" | "alloc::string::String"
    );

    // A field that can hold any byte sequence skips UTF-8 entirely, because for these types
    // rejecting a word would be the wrong answer: the operating system accepts `/tmp/\xff`
    // as a filename, so a CLI has to be able to receive one. Everything else still converts
    // through text, since `FromStr` is the only thing an arbitrary type offers.
    //
    // Recognised by spelling, with the same reasoning as `String` above: an adopter's own
    // `PathBuf` would fail to compile rather than quietly take a mangled value, because what
    // is handed to it is an `OsString` and not a `&str`.
    let os_target = match rendered.as_str() {
        "PathBuf" | "std::path::PathBuf" | "::std::path::PathBuf" => {
            Some(quote!(::std::path::PathBuf::from))
        }
        "OsString" | "std::ffi::OsString" | "::std::ffi::OsString" => {
            Some(quote!(::std::convert::identity))
        }
        _ => None,
    };
    if let Some(build) = os_target {
        // Lossless on Unix, where any byte sequence is a filename. On Windows the encoding
        // is WTF-8 and the conversion is partial, so a value that will not convert is
        // reported the same way any other unconvertible value is — never dropped, and never
        // replaced by a different filename.
        let one = |value: TokenStream| {
            quote! {
                match usage_argv::os_string_from_bytes(#value) {
                    ::std::result::Result::Ok(__usage_os) => #build(__usage_os),
                    ::std::result::Result::Err(__usage_bytes) => {
                        return ::std::result::Result::Err(
                            usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                                usage_argv::InvalidValue {
                                    name: #name,
                                    value: ::std::string::String::from_utf8_lossy(
                                        &__usage_bytes,
                                    )
                                    .into_owned(),
                                    reason: ::std::string::ToString::to_string(
                                        &"this platform cannot hold these bytes in a path",
                                    ),
                                },
                            )),
                        );
                    }
                }
            }
        };
        let converted = one;
        return match field.shape {
            // Unreachable: a switch and a count have no `value_ty`, so the early return
            // above already handled them.
            Shape::Bool | Shape::Count => quote!(#ident: partial.#ident),
            Shape::Required => {
                let value = converted(quote!(partial.#ident));
                quote!(#ident: #value)
            }
            // A `match` rather than `.map`, and a loop rather than `.collect`, for the same
            // reason the text path below uses them: the conversion can fail, and a `return`
            // inside a closure would leave the error in the closure's own return type.
            Shape::Optional => {
                let value = converted(quote!(__usage_value));
                quote! {
                    #ident: match partial.#ident {
                        ::std::option::Option::Some(__usage_value) => {
                            ::std::option::Option::Some(#value)
                        }
                        ::std::option::Option::None => ::std::option::Option::None,
                    }
                }
            }
            Shape::Many => {
                let value = converted(quote!(__usage_value));
                let collected = quote! {{
                    let mut __usage_values =
                        ::std::vec::Vec::with_capacity(partial.#ident.len());
                    for __usage_value in partial.#ident {
                        __usage_values.push(#value);
                    }
                    __usage_values
                }};
                if field.optional_collection {
                    let given = format_ident!("__given_{}", ident);
                    let defaulted = !field.default.is_empty();
                    // Same as below: whether anything arrived is what tells "never given"
                    // from "given nothing", which the `Vec` itself cannot. A `default_if`
                    // that fired has already pushed, so a non-empty vec is a value too.
                    quote! {
                        #ident: if partial.#given || #defaulted || !partial.#ident.is_empty() {
                            ::std::option::Option::Some(#collected)
                        } else {
                            ::std::option::Option::None
                        }
                    }
                } else {
                    quote!(#ident: #collected)
                }
            }
        };
    }

    let converted = |value: TokenStream| {
        let text = quote! {
            match ::std::string::String::from_utf8(#value) {
                ::std::result::Result::Ok(text) => text,
                ::std::result::Result::Err(bad) => {
                    return ::std::result::Result::Err(
                        usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                            usage_argv::InvalidValue {
                                name: #name,
                                value: ::std::string::String::from_utf8_lossy(
                                    bad.as_bytes(),
                                )
                                .into_owned(),
                                reason: ::std::string::ToString::to_string(&bad.utf8_error()),
                            },
                        )),
                    );
                }
            }
        };
        if is_std_string {
            return text;
        }
        quote! {{
            let __usage_text = #text;
            match ::std::str::FromStr::from_str(&__usage_text) {
                ::std::result::Result::Ok(parsed) => parsed,
                ::std::result::Result::Err(reason) => {
                    return ::std::result::Result::Err(
                        usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                            usage_argv::InvalidValue {
                                name: #name,
                                value: __usage_text,
                                reason: ::std::string::ToString::to_string(&reason),
                            },
                        )),
                    );
                }
            }
        }}
    };

    match field.shape {
        Shape::Bool | Shape::Count => quote!(#ident: partial.#ident),
        Shape::Required => {
            let one = converted(quote!(partial.#ident));
            quote!(#ident: #one)
        }
        Shape::Optional => {
            let one = converted(quote!(__usage_value));
            quote! {
                #ident: match partial.#ident {
                    ::std::option::Option::Some(__usage_value) => {
                        ::std::option::Option::Some(#one)
                    }
                    ::std::option::Option::None => ::std::option::Option::None,
                }
            }
        }
        Shape::Many => {
            let one = converted(quote!(__usage_value));
            // Built by hand rather than with `collect`, so the error can carry the value
            // that failed rather than only that one did.
            // A `Vec<String>` is moved whole. Rebuilding it element by element allocated a
            // second `Vec` to hold what the first already held, which is one allocation per
            // collecting field — and mise's commands collect a lot.
            // Built by hand rather than with `collect`, so the error can carry the value
            // that failed rather than only that one did.
            let collected = quote! {{
                let mut __usage_values = ::std::vec::Vec::with_capacity(partial.#ident.len());
                for __usage_value in partial.#ident {
                    __usage_values.push(#one);
                }
                __usage_values
            }};
            if field.optional_collection {
                let given = format_ident!("__given_{}", ident);
                // A declared default is a value, so a field that has one is never `None`. It
                // does not set `__given_*` — the environment still has to be able to replace it
                // — so the answer has to come from the declaration rather than from the partial.
                // A `default_if` that matched has already pushed, which a leftover empty vec
                // would not.
                let defaulted = !field.default.is_empty();
                // `Option<Vec<T>>` distinguishes "never given" from "given nothing", which
                // no `Vec` can — so the answer comes from whether anything arrived.
                quote! {
                    #ident: if partial.#given || #defaulted || !partial.#ident.is_empty() {
                        ::std::option::Option::Some(#collected)
                    } else {
                        ::std::option::Option::None
                    }
                }
            } else {
                quote!(#ident: #collected)
            }
        }
    }
}

/// Put a field back the way `start()` left it.
///
/// Used twice, and the second use is why it is worth naming: a flag displaced by an
/// `overrides` has to look untouched, and "untouched" means its declared default rather
/// than blank. clap does the same — a boolean that loses an override reads as `false`,
/// not as absent.
fn reset_to_default(field: &Field) -> TokenStream {
    let ident = &field.ident;
    // A collection is cleared rather than replaced, so the field keeps whatever capacity it
    // already allocated — and clearing is also the first half of seeding it, since a default
    // means *these values and no others* however many were bound before.
    let cleared = match field.shape {
        Shape::Many => quote!(partial.#ident.clear();),
        _ => quote!(partial.#ident = ::std::default::Default::default();),
    };
    if field.default.is_empty() {
        return cleared;
    }
    if let Some(value) = &field.default_value_t {
        let value_ty = field
            .value_ty
            .as_ref()
            .expect("a typed default belongs to a value-taking field");
        let bytes = quote!({
            let __usage_default: #value_ty = #value;
            ::std::string::ToString::to_string(&__usage_default).into_bytes()
        });
        return match field.shape {
            Shape::Optional => quote! {
                partial.#ident = ::std::option::Option::Some(#bytes);
            },
            Shape::Required => quote!(partial.#ident = #bytes;),
            // The model rejects these shapes before codegen.
            _ => cleared,
        };
    }
    // Every shape but a collection was checked in the model to have at most one.
    assign_literal(field, &field.default[0], field.default.as_slice())
}

/// Put `value` into the field the way a declared default does.
fn assign_literal(field: &Field, first: &str, all: &[String]) -> TokenStream {
    let ident = &field.ident;
    let cleared = match field.shape {
        Shape::Many => quote!(partial.#ident.clear();),
        _ => quote!(),
    };
    match field.shape {
        Shape::Bool => {
            let on = matches!(first, "1" | "true" | "True" | "TRUE");
            quote!(partial.#ident = #on;)
        }
        Shape::Optional => quote! {
            partial.#ident = ::std::option::Option::Some(#first.as_bytes().to_vec());
        },
        Shape::Required => quote!(partial.#ident = #first.as_bytes().to_vec();),
        Shape::Count => quote!(partial.#ident = ::std::default::Default::default();),
        Shape::Many => {
            let defaults = all;
            match field.delimiter {
                Some(delimiter) => {
                    let byte = u8::try_from(u32::from(delimiter))
                        .expect("the model rejects non-ASCII delimiters");
                    quote! {
                        #cleared
                        #(
                            for part in #defaults.as_bytes().split(|b| *b == #byte) {
                                partial.#ident.push(part.to_vec());
                            }
                        )*
                    }
                }
                None => quote! {
                    #cleared
                    #(partial.#ident.push(#defaults.as_bytes().to_vec());)*
                },
            }
        }
    }
}

fn default_if_predicate(cli: &Cli, condition: &ConditionalDefault) -> TokenStream {
    let Some(other) = cli.field_for_selector(&condition.selector) else {
        let selector = &condition.selector;
        return match &condition.when {
            None => quote!(argument_state(partial, #selector).is_some_and(|state| state.given)),
            Some(when) => quote!(
                argument_matches(partial, #selector, #when.as_bytes())
                    == ::std::option::Option::Some(true)
            ),
        };
    };
    let other_given = format_ident!("__given_{}", other.ident);
    let ident = &other.ident;
    match &condition.when {
        None => quote!(partial.#other_given),
        Some(when) => {
            let matches = match other.shape {
                Shape::Optional => quote!(
                    partial.#ident.as_deref().is_some_and(|v| v == #when.as_bytes())
                ),
                Shape::Required => quote!(partial.#ident.as_slice() == #when.as_bytes()),
                Shape::Many => quote!(
                    partial.#ident.iter().any(|v| v.as_slice() == #when.as_bytes())
                ),
                Shape::Bool => match when.as_str() {
                    "true" => quote!(partial.#ident),
                    "false" => quote!(!partial.#ident),
                    _ => quote!(false),
                },
                Shape::Count => match when.as_str() {
                    "true" => quote!(partial.#ident > 0),
                    "false" => quote!(partial.#ident == 0),
                    _ => quote!(false),
                },
            };
            quote!(partial.#other_given && #matches)
        }
    }
}

fn default_if_would_apply(cli: &Cli, field: &Field) -> Option<TokenStream> {
    if field.default_if.is_empty() {
        return None;
    }
    let preds: Vec<TokenStream> = field
        .default_if
        .iter()
        .map(|condition| default_if_predicate(cli, condition))
        .collect();
    Some(quote!(#(#preds)||*))
}

/// Take one event and say whether it belonged to this command.
fn apply_fn(cli: &Cli) -> TokenStream {
    let route = subcommand_parts(cli).map(|p| p.route).unwrap_or_default();
    let per_level_resets = reset_per_level(cli);
    // A flattened struct's flags are in this command's table, but its *keys* were minted in
    // its own expansion — so they cannot be matched here. Its `apply` recognises them, and
    // says whether it took the event.
    let flattened = cli.fields.iter().filter_map(|f| {
        let Kind::Flatten { ty } = &f.kind else {
            return None;
        };
        let ident = &f.ident;
        let reverse_displacements = cli.fields.iter().flat_map(|field| {
            field
                .overrides
                .iter()
                .filter(|selector| cli.field_for_selector(selector).is_none())
                .map(move |selector| {
                    let statement = displace_statement(field);
                    quote! {
                        if <#ty as usage_argv::spec::CommandArgs>::event_matches(
                            event,
                            #selector,
                        ) {
                            #statement
                        }
                    }
                })
        });
        Some(quote! {
            if <#ty as usage_argv::spec::CommandArgs>::apply(&mut partial.#ident, event) {
                #(#reverse_displacements)*
                return true;
            }
        })
    });
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
    let flag_arms = flags.iter().enumerate().map(|(i, f)| flag_arm(cli, i, f));
    let arg_arms = args.iter().enumerate().map(|(i, f)| arg_arm(i, f));

    quote! {
        pub fn apply(
            partial: &mut Partial,
            event: &usage_argv::Event<'_, '_, '_>,
        ) -> bool {
            use usage_argv::Event;
            #route
            #(#flattened)*
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
                Event::Arg {
                    arg,
                    value,
                    delimit,
                } => {
                    let (value, delimit) = (*value, *delimit);
                    let _ = (value, delimit);
                    match arg.key {
                        #(#arg_arms)*
                        _ => false,
                    }
                }
                // Descending is the caller's business: it is what decides which
                // command's fields the following events belong to. But any command
                // word — not only a child of ours — begins a level at which this
                // struct's globals may be given again, and this arm is the one
                // place every partial sees the word, whether it belongs to a root,
                // a subcommand, or a flattened group.
                Event::Command(_) => {
                    #per_level_resets
                    false
                }
                // Forwarded argv belongs to the catch-all variant, which the
                // subcommand route claims; this command's own flags do not.
                Event::External { .. } => false,
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
    /// `default_subcommand:` for the `Command` table, when one is declared.
    default: TokenStream,
    /// `external_subcommand:` for the `Command` table.
    external: TokenStream,
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

    let selected = quote! {
        match partial.__usage_selected {
            ::std::option::Option::Some(__usage_at) => {
                <#ty as usage_argv::spec::Subcommands>::select(
                    partial.__usage_sub,
                    __usage_at,
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
                        usage_argv::Error::MissingSubcommand,
                    );
                }
            },
        }
    };

    Some(SubcommandParts {
        commands: quote!(subcommands: <#ty as usage_argv::spec::Subcommands>::COMMANDS,),
        external: quote!(
            external_subcommand: <#ty as usage_argv::spec::Subcommands>::HAS_EXTERNAL,
        ),
        // Resolved from the name at compile time. The variants are another expansion, so the
        // name is all there is to go on here — but `find_subcommand` searches the list during
        // const evaluation, which means a name no subcommand answers to fails to compile
        // rather than being emitted into a spec that nothing checks.
        default: match cli.default_subcommand.as_deref() {
            ::std::option::Option::Some(name) => quote! {
                default_subcommand: ::std::option::Option::Some(
                    usage_argv::find_subcommand(
                        <#ty as usage_argv::spec::Subcommands>::COMMANDS,
                        #name,
                    ),
                ),
            },
            ::std::option::Option::None => TokenStream::new(),
        },
        metas: quote!(subcommands: <#ty as usage_argv::spec::Subcommands>::METAS,),
        partial_fields: quote! {
            pub __usage_sub: <#ty as usage_argv::spec::Subcommands>::Partial,
            /// Which of this command's subcommands was reached, as a variant index.
            /// Found from the table's own address, then mapped through `VARIANT_OF`
            /// when a catch-all sits among the named commands.
            pub __usage_selected: ::std::option::Option<usize>,
        },
        partial_starts: quote! {
            __usage_sub: ::std::default::Default::default(),
            __usage_selected: ::std::option::Option::None,
        },
        route: quote! {
            // A command word only counts as *this* command's if one of its own
            // subcommands answers to it: a deeper descent belongs to whoever owns
            // that command, and recording it here would make the wrong variant look
            // selected. `COMMANDS` holds named commands only, so the position is
            // mapped through `VARIANT_OF` when a catch-all variant sits beside them.
            if let usage_argv::Event::Command(__usage_cmd) = event {
                if let ::std::option::Option::Some(__usage_named) =
                    <#ty as usage_argv::spec::Subcommands>::COMMANDS
                        .iter()
                        .position(|candidate| ::core::ptr::eq(*candidate, *__usage_cmd))
                {
                    let __usage_at = if <#ty as usage_argv::spec::Subcommands>::HAS_EXTERNAL {
                        <#ty as usage_argv::spec::Subcommands>::VARIANT_OF[__usage_named]
                    } else {
                        __usage_named
                    };
                    partial.__usage_selected = ::std::option::Option::Some(__usage_at);
                    // The one place the selection and the storage are tied together: from
                    // here on, `__usage_selected` naming a variant means `__usage_sub`
                    // holds that variant. Everything downstream relies on it.
                    <#ty as usage_argv::spec::Subcommands>::begin(
                        &mut partial.__usage_sub,
                        __usage_at,
                    );
                }
            }
            if let usage_argv::Event::External { .. } = event {
                if let ::std::option::Option::Some(__usage_at) =
                    <#ty as usage_argv::spec::Subcommands>::EXTERNAL
                {
                    partial.__usage_selected = ::std::option::Option::Some(__usage_at);
                    <#ty as usage_argv::spec::Subcommands>::begin(
                        &mut partial.__usage_sub,
                        __usage_at,
                    );
                }
            }
            // Only the selected one is asked — see `Subcommands::apply`. The selection is
            // set just above, so a command word reaches the command it named on the same
            // event that selected it.
            if <#ty as usage_argv::spec::Subcommands>::apply(
                &mut partial.__usage_sub,
                partial.__usage_selected,
                event,
            ) {
                return true;
            }
        },
        check: quote! {
            if let ::std::option::Option::Some(__usage_at) = partial.__usage_selected {
                <#ty as usage_argv::spec::Subcommands>::check(
                    &mut partial.__usage_sub,
                    __usage_at,
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
    let runtime = runtime_path();
    let validation = validation_path();
    let validation_import = cli
        .fields
        .iter()
        .any(|field| field.validate.is_some())
        .then(|| quote!(use #validation as usage_validation;));
    let presence = presence_methods(cli);
    let apply_defaults = declared_defaults(cli);
    let apply_env = env_fallbacks(cli);
    // A group carries settings the same way a root does, minus the layer: `SettingGiven` is
    // usage-argv's own vocabulary, so a flattened group can hand its parent what it was given
    // without either of them naming the config crate. Emitted whenever it has anything to say —
    // its own bindings, or a group of its own that has some.
    let parts = settings(cli);
    let settings_defs = parts.as_ref().map(|s| {
        let given = &s.given;
        let bindings = &s.bindings;
        quote!(#given #bindings)
    });
    let settings_impl = parts.as_ref().map(|_| {
        quote! {
            const SETTINGS_BINDINGS: &'static [(&'static str, &'static str)] =
                SETTINGS_BINDINGS;

            fn settings_given(
                partial: &Self::Partial,
            ) -> ::std::vec::Vec<(&'static str, usage_argv::spec::SettingGiven)> {
                settings_given(partial)
            }
        }
    });
    let command_key = key_ident("COMMAND", None);
    let restart_token = option_str(cli.restart_token.as_deref());
    let mount = option_str(cli.mount.as_deref());
    // A bare `T` subcommand field says the command cannot run alone; an `Option<T>` says it
    // can. The parser already refuses the invocation from the type — this is so the emitted
    // spec says it too, since help, docs and completions read that rather than the type.
    let flatten_checks = flatten_checks(cli);
    let subcommand_required = cli.fields.iter().any(|f| {
        matches!(
            f.kind,
            Kind::Subcommand {
                optional: false,
                ..
            }
        )
    });
    let unknown_flags = unknown_flags_tokens(cli);
    let arg_required_else_help = cli.arg_required_else_help;
    let dont_delimit_trailing_values = cli.dont_delimit_trailing_values;
    let before_help = option_expr(cli.before_help.as_ref());
    let before_long_help = option_expr(cli.before_long_help.as_ref());
    let after_help = option_expr(cli.after_help.as_ref());
    let after_long_help = option_expr(cli.after_long_help.as_ref());

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

    let keys = key_consts(&cli.fingerprint, flags.len(), args.len());
    let flag_tables = flags.iter().enumerate().map(|(i, f)| flag_table(i, f));
    let arg_tables = args.iter().enumerate().map(|(i, f)| arg_table(i, f));
    let flag_metas = flags
        .iter()
        .enumerate()
        .map(|(i, f)| flag_meta(cli, i, f, &cli.ident));
    let arg_metas = args
        .iter()
        .enumerate()
        .map(|(i, f)| arg_meta(cli, i, f, &cli.ident));
    // Both the plain slices and, when a field is flattened, the joined arrays.
    let tables = tables(cli);
    let table_decls = &tables.decls;
    let meta_table_decls = &tables.meta_decls;
    let (group_meta_decl, group_meta_table_ref) = group_meta_table(cli);
    let flag_table_ref = &tables.flags;
    let arg_table_ref = &tables.args;
    let flag_meta_table_ref = &tables.flag_metas;
    let arg_meta_table_ref = &tables.arg_metas;

    let name = &cli.name;
    let aliases = cli.aliases.iter().chain(&cli.hidden_aliases);
    let hidden_aliases = &cli.hidden_aliases;
    let about = cli
        .about_attr
        .as_ref()
        .map(|value| option_expr(Some(value)))
        .unwrap_or_else(|| option_str(cli.about.as_deref()));
    let long_about = cli
        .long_about_attr
        .as_ref()
        .map(|value| option_expr(Some(value)))
        .unwrap_or_else(|| option_str(cli.long_about.as_deref()));
    let partial = partial_struct(cli);
    let argument_lookup = argument_lookup_functions(cli);
    let defaults = partial_defaults(cli);
    let apply = apply_fn(cli);
    let post = post_binding(cli);
    let parts = subcommand_parts(cli);
    let sub_commands = parts
        .as_ref()
        .map(|p| p.commands.clone())
        .unwrap_or_default();
    let sub_external = parts
        .as_ref()
        .map(|p| p.external.clone())
        .unwrap_or_default();
    let sub_metas = parts.as_ref().map(|p| p.metas.clone()).unwrap_or_default();
    let sub_build = parts.as_ref().map(|p| p.build.clone()).unwrap_or_default();
    // The same conversion the root gets. Two emitters producing one `build` is what let
    // this diverge: a typed field on a subcommand compiled here and not there, which is
    // every command mise has.
    let field_finals: Vec<_> = cli
        .fields
        .iter()
        .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
        .map(field_final)
        .collect();
    let built = built_value(cli, &sub_build, &field_finals);

    // The root cannot carry one — the spec writer asserts it — so this is the non-root
    // path's alone, which is also the only path a command's own declaration reaches.
    let effect = cli
        .effect
        .clone()
        .unwrap_or_else(|| quote!(::core::option::Option::None));
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
        const _: () = {
            use #runtime as usage_argv;
            #validation_import

            #flatten_checks
            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #table_decls

            pub static COMMAND: usage_argv::Command = usage_argv::Command {
                name: #name,
                aliases: &[#(#aliases),*],
                key: #command_key,
                unknown_flags: #unknown_flags,
                arg_required_else_help: #arg_required_else_help,
                dont_delimit_trailing_values: #dont_delimit_trailing_values,
                flags: #flag_table_ref,
                args: #arg_table_ref,
                #sub_commands
                #sub_external
                ..usage_argv::Command::EMPTY
            };

            #(#flag_metas)*
            #(#arg_metas)*
            #meta_table_decls

            #group_meta_decl

            pub static COMMAND_META: usage_argv::spec::CommandMeta = usage_argv::spec::CommandMeta {
                cmd: &COMMAND,
                effect: #effect,
                about: #about,
                long_about: #long_about,
                hidden_aliases: &[#(#hidden_aliases),*],
                restart_token: #restart_token,
                subcommand_required: #subcommand_required,
                mount: #mount,
                before_help: #before_help,
                before_long_help: #before_long_help,
                after_help: #after_help,
                after_long_help: #after_long_help,
                flags: #flag_meta_table_ref,
                args: #arg_meta_table_ref,
                groups: #group_meta_table_ref,
                #sub_metas
                ..usage_argv::spec::CommandMeta::EMPTY
            };

            pub fn __usage_text(value: &[u8]) -> ::std::vec::Vec<u8> {
                value.to_vec()
            }

            pub fn __usage_value_text(
                value: ::std::option::Option<&[u8]>,
            ) -> ::std::vec::Vec<u8> {
                value.map(__usage_text).unwrap_or_default()
            }

            #partial
            #apply
            #argument_lookup

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
            ) -> ::std::result::Result<(), usage_argv::Error<'t, 'v>> {
                // Read unconditionally: a command that declares nothing to check would
                // otherwise leave the parameter unused in the user's crate, where
                // nobody can silence it.
                let _ = &partial;
                #post
                ::std::result::Result::Ok(())
            }

            #settings_defs

            impl usage_argv::spec::CommandArgs for #ident {
                type Partial = Partial;

                const COMMAND: &'static usage_argv::Command<'static> = &COMMAND;
                const META: &'static usage_argv::spec::CommandMeta<'static> =
                    &COMMAND_META;

                fn start() -> Self::Partial {
                    start()
                }

                fn apply(
                    partial: &mut Self::Partial,
                    event: &usage_argv::Event<'_, '_, '_>,
                ) -> bool {
                    apply(partial, event)
                }

                fn check<'t, 'v>(
                    partial: &mut Self::Partial,
                ) -> ::std::result::Result<(), usage_argv::Error<'t, 'v>> {
                    check(partial)
                }

                #presence

                fn apply_defaults(partial: &mut Self::Partial) {
                    #apply_defaults
                }

                fn apply_env(partial: &mut Self::Partial) {
                    #apply_env
                }

                #settings_impl

                fn build<'t, 'v>(
                    partial: Self::Partial,
                ) -> ::std::result::Result<Self, usage_argv::Error<'t, 'v>> {
                    ::std::result::Result::Ok(#built)
                }
            }
        };
    }
}

/// The enum a `subcommand` field holds: its variants' tables, and the trait a
/// parent uses to route events into them.
fn rewrite_inline_arg_meta(meta: &mut syn::Meta) {
    if matches!(meta, syn::Meta::Path(path) if path.is_ident("arg")) {
        *meta = syn::parse_quote!(usage(arg));
        return;
    }
    let path = match meta {
        syn::Meta::Path(path) => path,
        syn::Meta::List(list) => &mut list.path,
        syn::Meta::NameValue(value) => &mut value.path,
    };
    if path.is_ident("arg") {
        *path = syn::parse_quote!(usage);
        return;
    }
    if !path.is_ident("cfg_attr") {
        return;
    }
    let syn::Meta::List(list) = meta else {
        return;
    };
    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    let Ok(mut nested) = syn::parse::Parser::parse2(parser, list.tokens.clone()) else {
        return;
    };
    for value in nested.iter_mut().skip(1) {
        rewrite_inline_arg_meta(value);
    }
    list.tokens = quote!(#nested);
}

fn inline_field_meta(meta: &syn::Meta) -> Option<syn::Meta> {
    if meta.path().is_ident("arg") {
        let mut meta = meta.clone();
        rewrite_inline_arg_meta(&mut meta);
        return Some(meta);
    }
    if meta.path().is_ident("usage") || meta.path().is_ident("doc") || meta.path().is_ident("cfg") {
        return Some(meta.clone());
    }
    let syn::Meta::List(list) = meta else {
        return None;
    };
    if !list.path.is_ident("cfg_attr") {
        return None;
    }
    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    let nested = syn::parse::Parser::parse2(parser, list.tokens.clone()).ok()?;
    let mut nested = nested.into_iter();
    let condition = nested.next()?;
    let kept = nested
        .filter_map(|meta| inline_field_meta(&meta))
        .collect::<Vec<_>>();
    if kept.is_empty() {
        return None;
    }
    syn::parse2(quote!(cfg_attr(#condition, #(#kept),*))).ok()
}

fn inline_field_attr(attr: syn::Attribute) -> Option<syn::Attribute> {
    let meta = inline_field_meta(&attr.meta)?;
    Some(syn::Attribute { meta, ..attr })
}

fn meta_controls_field_presence(meta: &syn::Meta) -> bool {
    if meta.path().is_ident("cfg") {
        return true;
    }
    let syn::Meta::List(list) = meta else {
        return false;
    };
    if !list.path.is_ident("cfg_attr") {
        return false;
    }
    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    syn::parse::Parser::parse2(parser, list.tokens.clone())
        .is_ok_and(|nested| nested.iter().skip(1).any(meta_controls_field_presence))
}

pub fn emit_subcommands(subs: &Subcommands) -> TokenStream {
    let ident = &subs.ident;
    let runtime = runtime_path();
    let derive = derive_path();

    // The structs bare and inline variants imply, written here so everything downstream keeps
    // speaking to one Args struct. Clap-shaped `arg` attributes are rewritten to the native
    // spelling while copying inline fields, which lets a migration keep its enum layout.
    let generated_structs = subs
        .variants
        .iter()
        .filter(|v| v.unit || v.inline_fields.is_some())
        .map(|v| {
            let name = &v.ty;
            // Whatever the variant said about the command, written where the command's
            // metadata is actually built — and read there by the same code that reads it on
            // any other `Args`.
            let effect = v
                .effect
                .as_ref()
                .map(|word| quote!(#[usage(effect = #word)]));
            let fields = v.inline_fields.iter().flatten().cloned().map(|mut field| {
                field.attrs = field
                    .attrs
                    .into_iter()
                    .filter_map(inline_field_attr)
                    .collect();
                field
            });
            quote! {
                #[doc(hidden)]
                #[derive(#derive::Args)]
                #effect
                pub struct #name {
                    #(#fields),*
                }
            }
        })
        .collect::<Vec<_>>();
    let generated_structs = generated_structs.into_iter();

    // One *variant* per subcommand, not one field: a parse fills exactly one of them, and a
    // struct with room for all of them is the whole CLI's accumulator whichever command ran —
    // 11KB of it at mise's scale, of which 210 commands' worth is never touched. The variant
    // comes into being when a command word selects it, in `begin`.
    let partial_variants = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote!(#variant(::std::vec::Vec<::std::vec::Vec<u8>>),)
        } else {
            let ty = &v.ty;
            quote!(#variant(<#ty as usage_argv::spec::CommandArgs>::Partial),)
        }
    });
    // Idempotent, as the trait requires: a restart token can re-announce the command that is
    // already selected, and starting it again there would throw away the parse so far.
    let begins = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                #i => {
                    if !::std::matches!(partial, Partial::#variant(_)) {
                        *partial = Partial::#variant(::std::vec::Vec::new());
                    }
                }
            }
        } else {
            let ty = &v.ty;
            quote! {
                #i => {
                    if !::std::matches!(partial, Partial::#variant(_)) {
                        *partial = Partial::#variant(
                            <#ty as usage_argv::spec::CommandArgs>::start(),
                        );
                    }
                }
            }
        }
    });
    let applies = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                ::std::option::Option::Some(#i) => {
                    if let Partial::#variant(__usage_p) = partial {
                        if let usage_argv::Event::External { values } = event {
                            __usage_p.extend(
                                values.iter().map(|v| v.as_encoded_bytes().to_vec()),
                            );
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            }
        } else {
            let ty = &v.ty;
            quote! {
                ::std::option::Option::Some(#i) => {
                    if let Partial::#variant(__usage_p) = partial {
                        <#ty as usage_argv::spec::CommandArgs>::apply(__usage_p, event)
                    } else {
                        // `begin` put this variant here on the event that selected it, so the
                        // partial always matches the selection. An event with nowhere to go was
                        // not this command's.
                        false
                    }
                }
            }
        }
    });
    // The enum is where the command set is declared, so the variant names the
    // command — its own kebab-case name, or whatever `name` says. Splicing the
    // struct's table unchanged would have used the *struct's* name instead, which
    // only looks right when the two happen to match.
    let command_overrides = subs
        .variants
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.external)
        .map(|(i, v)| {
            let name = format_ident!("COMMAND_{i}");
            let alias_groups = format_ident!("ALIAS_GROUPS_{i}");
            let aliases_name = format_ident!("ALIASES_{i}");
            let ty = &v.ty;
            let cmd_name = &v.name;
            // Both kinds of alias go in the table, because the parser matches both; which of
            // them help and completions mention is the metadata's business, below.
            let aliases = v.aliases.iter().chain(&v.hidden_aliases);
            quote! {
                const #alias_groups: &[&[&str]] = &[
                    <#ty as usage_argv::spec::CommandArgs>::COMMAND.aliases,
                    &[#(#aliases),*],
                ];
                static #aliases_name: [&str; usage_argv::table_len(#alias_groups)] =
                    usage_argv::spec::concat_aliases(#alias_groups);
                pub static #name: usage_argv::Command = usage_argv::Command {
                    name: #cmd_name,
                    aliases: &#aliases_name,
                    ..*<#ty as usage_argv::spec::CommandArgs>::COMMAND
                };
            }
        });
    let commands = subs
        .variants
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.external)
        .map(|(i, _)| {
            let name = format_ident!("COMMAND_{i}");
            quote!(&#name)
        });
    let unique_commands = subs
        .variants
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.external)
        .map(|(i, _)| {
            let name = format_ident!("COMMAND_{i}");
            quote!(&#name)
        });
    let variant_of = subs
        .variants
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.external)
        .map(|(i, _)| quote!(#i));
    let has_external = subs.variants.iter().any(|v| v.external);
    let external_const = match subs.variants.iter().position(|v| v.external) {
        Some(i) => quote!(::std::option::Option::Some(#i)),
        None => quote!(::std::option::Option::None),
    };
    // A doc comment on the variant wins over the struct's, since that is where a
    // reader of the enum expects to describe the command — and ignoring it would lose
    // the description without saying so. Overriding one field of the struct's
    // metadata is possible in a const, so the tables stay static.
    let meta_overrides = subs
        .variants
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.external)
        .map(|(i, v)| {
            let name = format_ident!("META_{i}");
            let cmd = format_ident!("COMMAND_{i}");
            let hidden_groups = format_ident!("HIDDEN_ALIAS_GROUPS_{i}");
            let hidden_name = format_ident!("HIDDEN_ALIASES_{i}");
            let ty = &v.ty;
            // A doc comment on the variant wins over the struct's, since that is where a
            // reader of the enum expects to describe the command. Absent one, the
            // struct's own description carries through.
            // Each falls back on its own. A variant that gives a short description was suppressing
            // the struct's long one, which is exactly how a generated CLI is shaped: the enum says
            // what the command is for in a line, and the struct's own comment carries the rest. The
            // long form went missing from help for every command written that way.
            let about = match v.help.as_ref() {
                Some(help) => option_expr(Some(help)),
                None => quote!(<#ty as usage_argv::spec::CommandArgs>::META.about),
            };
            let long_about = match v.long_help.as_ref() {
                Some(long) => option_expr(Some(long)),
                None => quote!(<#ty as usage_argv::spec::CommandArgs>::META.long_about),
            };
            let before_help = v
                .before_help
                .as_ref()
                .map(|value| option_expr(Some(value)))
                .unwrap_or_else(
                    || quote!(<#ty as usage_argv::spec::CommandArgs>::META.before_help),
                );
            let before_long_help = v
                .before_long_help
                .as_ref()
                .map(|value| option_expr(Some(value)))
                .unwrap_or_else(
                    || quote!(<#ty as usage_argv::spec::CommandArgs>::META.before_long_help),
                );
            let after_help = v
                .after_help
                .as_ref()
                .map(|value| option_expr(Some(value)))
                .unwrap_or_else(|| quote!(<#ty as usage_argv::spec::CommandArgs>::META.after_help));
            let after_long_help = v
                .after_long_help
                .as_ref()
                .map(|value| option_expr(Some(value)))
                .unwrap_or_else(
                    || quote!(<#ty as usage_argv::spec::CommandArgs>::META.after_long_help),
                );
            // Which of the table's aliases are hidden. The visible ones are not listed
            // anywhere: `cmd.aliases` minus these is what help and completions show.
            let hidden = &v.hidden_aliases;
            // A hidden command still answers to its name; it is simply not offered. Declared on
            // the variant, which is where the command itself is declared.
            let hide = v.hide;
            quote! {
                const #hidden_groups: &[&[&str]] = &[
                    <#ty as usage_argv::spec::CommandArgs>::META.hidden_aliases,
                    &[#(#hidden),*],
                ];
                static #hidden_name: [&str; usage_argv::table_len(#hidden_groups)] =
                    usage_argv::spec::concat_aliases(#hidden_groups);
                pub static #name: usage_argv::spec::CommandMeta =
                    usage_argv::spec::CommandMeta {
                        cmd: &#cmd,
                        about: #about,
                        long_about: #long_about,
                        before_help: #before_help,
                        before_long_help: #before_long_help,
                        after_help: #after_help,
                        after_long_help: #after_long_help,
                        hide: #hide,
                        hidden_aliases: &#hidden_name,
                        ..*<#ty as usage_argv::spec::CommandArgs>::META
                    };
            }
        });
    let metas = subs
        .variants
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.external)
        .map(|(i, _)| {
            let name = format_ident!("META_{i}");
            quote!(&#name)
        });
    // Matched on the command's key rather than its name, so selecting a variant is
    // an integer comparison and cannot be confused by an alias.
    let checks = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                #i => ::std::result::Result::Ok(()),
            }
        } else {
            let ty = &v.ty;
            quote! {
                #i => match partial {
                    Partial::#variant(__usage_p) => {
                        <#ty as usage_argv::spec::CommandArgs>::check(__usage_p)
                    }
                    // Selected but unfilled cannot happen — see `Subcommands::begin`.
                    _ => ::std::result::Result::Ok(()),
                },
            }
        }
    });
    // Every variant's bindings, because a table says what the CLI *can* do and is compared
    // against a spec that documents all of them — but only the selected variant's values, since
    // those are about one invocation. A command nobody ran did not give anything.
    let binding_parts = subs.variants.iter().filter(|v| !v.external).map(|v| {
        let ty = &v.ty;
        quote!(<#ty as usage_argv::spec::CommandArgs>::SETTINGS_BINDINGS)
    });
    let binding_lens = binding_parts.clone().map(|part| quote!(+ #part.len()));
    let givens = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                ::std::option::Option::Some(#i) => ::std::vec::Vec::new(),
            }
        } else {
            let ty = &v.ty;
            quote! {
                ::std::option::Option::Some(#i) => {
                    if let Partial::#variant(__usage_p) = partial {
                        <#ty as usage_argv::spec::CommandArgs>::settings_given(__usage_p)
                    } else {
                        ::std::vec::Vec::new()
                    }
                }
            }
        }
    });
    // The partial holds only the selected subcommand's own, so every one of these asks the
    // variant rather than a field: an unselected arm has nothing to have been given.
    let any_givens = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                ::std::option::Option::Some(#i) => ::std::option::Option::None,
            }
        } else {
            let ty = &v.ty;
            quote! {
                ::std::option::Option::Some(#i) => {
                    if let Partial::#variant(__usage_p) = partial {
                        <#ty as usage_argv::spec::CommandArgs>::any_given(__usage_p)
                    } else {
                        ::std::option::Option::None
                    }
                }
            }
        }
    });
    let exclusive_givens = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                ::std::option::Option::Some(#i) => ::std::option::Option::None,
            }
        } else {
            let ty = &v.ty;
            quote! {
                ::std::option::Option::Some(#i) => {
                    if let Partial::#variant(__usage_p) = partial {
                        <#ty as usage_argv::spec::CommandArgs>::exclusive_given(__usage_p)
                    } else {
                        ::std::option::Option::None
                    }
                }
            }
        }
    });
    let apply_envs = subs.variants.iter().enumerate().map(|(i, v)| {
        let variant = format_ident!("V{i}");
        if v.external {
            quote! {
                ::std::option::Option::Some(#i) => {}
            }
        } else {
            let ty = &v.ty;
            quote! {
                ::std::option::Option::Some(#i) => {
                    if let Partial::#variant(__usage_p) = partial {
                        <#ty as usage_argv::spec::CommandArgs>::apply_env(__usage_p);
                    }
                }
            }
        }
    });
    let selects = subs.variants.iter().enumerate().map(|(i, v)| {
        let held = format_ident!("V{i}");
        let variant = &v.ident;
        if v.external {
            let name = &v.name;
            let collected = if v.external_os {
                quote! {{
                    let mut __usage_values = ::std::vec::Vec::with_capacity(__usage_p.len());
                    for __usage_item in __usage_p {
                        match usage_argv::os_string_from_bytes(__usage_item) {
                            ::std::result::Result::Ok(__usage_os) => {
                                __usage_values.push(__usage_os)
                            }
                            ::std::result::Result::Err(__usage_bytes) => {
                                return ::std::result::Result::Err(
                                    usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                                        usage_argv::InvalidValue {
                                            name: #name,
                                            value: ::std::string::String::from_utf8_lossy(
                                                &__usage_bytes,
                                            )
                                            .into_owned(),
                                            reason: ::std::string::ToString::to_string(
                                                &"this platform cannot hold these bytes",
                                            ),
                                        },
                                    )),
                                );
                            }
                        }
                    }
                    __usage_values
                }}
            } else {
                quote! {{
                    let mut __usage_values = ::std::vec::Vec::with_capacity(__usage_p.len());
                    for __usage_item in __usage_p {
                        match ::std::string::String::from_utf8(__usage_item) {
                            ::std::result::Result::Ok(__usage_text) => {
                                __usage_values.push(__usage_text)
                            }
                            ::std::result::Result::Err(__usage_bad) => {
                                return ::std::result::Result::Err(
                                    usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                                        usage_argv::InvalidValue {
                                            name: #name,
                                            value: ::std::string::String::from_utf8_lossy(
                                                __usage_bad.as_bytes(),
                                            )
                                            .into_owned(),
                                            reason: ::std::string::ToString::to_string(
                                                &__usage_bad.utf8_error(),
                                            ),
                                        },
                                    )),
                                );
                            }
                        }
                    }
                    __usage_values
                }}
            };
            let made = quote!(#ident::#variant(#collected));
            quote! {
                #i => match partial {
                    Partial::#held(__usage_p) => {
                        ::std::result::Result::Ok(::std::option::Option::Some(#made))
                    }
                    _ => ::std::result::Result::Ok(::std::option::Option::None),
                },
            }
        } else {
            let ty = &v.ty;
            // The one place the box matters: everything else — tables, partial, `build` —
            // speaks to the struct itself.
            let built = quote!(<#ty as usage_argv::spec::CommandArgs>::build(__usage_p)?);
            let built = if v.boxed {
                quote!(::std::boxed::Box::new(#built))
            } else {
                built
            };
            // A bare variant has nowhere to put what was built, and nothing was declared to go in
            // it — but the build still runs, because that is where the command's own checks live.
            let made = if v.unit {
                quote! {{
                    let _ = #built;
                    #ident::#variant
                }}
            } else if let Some(fields) = &v.inline_fields {
                let assignments = fields.iter().map(|field| {
                    let name = field
                        .ident
                        .as_ref()
                        .expect("named variant fields have names");
                    let cfg_attrs = field
                        .attrs
                        .iter()
                        .filter(|attr| meta_controls_field_presence(&attr.meta));
                    quote! {
                        #(#cfg_attrs)*
                        #name: __usage_built.#name
                    }
                });
                quote! {{
                    let __usage_built = #built;
                    // An empty named variant, or one whose fields were all removed by cfg,
                    // still has to run `build` for the command's checks. Mark the result used
                    // without changing the field moves below.
                    let _ = &__usage_built;
                    #ident::#variant {
                        #(#assignments),*
                    }
                }}
            } else {
                quote!(#ident::#variant(#built))
            };
            quote! {
                #i => match partial {
                    Partial::#held(__usage_p) => {
                        ::std::result::Result::Ok(::std::option::Option::Some(#made))
                    }
                    // Selected but unfilled cannot happen — see `Subcommands::begin`.
                    _ => ::std::result::Result::Ok(::std::option::Option::None),
                },
            }
        }
    });

    quote! {
        // Beside the enum rather than inside the generated module: the variants name these
        // types, and a type a variant cannot see is no use to it.
        #(#generated_structs)*

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
        const _: () = {
            use #runtime as usage_argv;

            pub enum Partial {
                /// No command word has selected a variant yet, so there is nothing to fill.
                Unselected,
                #(#partial_variants)*
            }

            impl ::std::default::Default for Partial {
                fn default() -> Self {
                    Partial::Unselected
                }
            }

            #(#command_overrides)*
            #(#meta_overrides)*

            const _: () = usage_argv::assert_unique_subcommand_names(&[#(#unique_commands),*]);

            impl usage_argv::spec::Subcommands for #ident {
                type Partial = Partial;

                const COMMANDS: &'static [&'static usage_argv::Command<'static>] =
                    &[#(#commands),*];
                const METAS: &'static [&'static usage_argv::spec::CommandMeta<'static>] =
                    &[#(#metas),*];
                const HAS_EXTERNAL: bool = #has_external;
                const EXTERNAL: ::std::option::Option<usize> = #external_const;
                const VARIANT_OF: &'static [usize] = &[#(#variant_of),*];

                fn apply(
                    partial: &mut Self::Partial,
                    selected: ::std::option::Option<usize>,
                    event: &usage_argv::Event<'_, '_, '_>,
                ) -> bool {
                    match selected {
                        #(#applies)*
                        // Nothing selected yet, or a position that cannot be produced: the event
                        // is not one of these commands'.
                        _ => false,
                    }
                }

                fn begin(partial: &mut Self::Partial, selected: usize) {
                    match selected {
                        #(#begins)*
                        // Not a position `COMMANDS` can produce, so there is no variant to
                        // make room for.
                        _ => {}
                    }
                }

                const SETTINGS_BINDINGS: &'static [(&'static str, &'static str)] = {
                    const PARTS: &[&'static [(&'static str, &'static str)]] = &[#(#binding_parts),*];
                    const N: usize = 0 #(#binding_lens)*;
                    const JOINED: [(&'static str, &'static str); N] =
                        usage_argv::spec::concat_bindings(PARTS);
                    &JOINED
                };

                fn settings_given(
                    partial: &Self::Partial,
                    selected: ::std::option::Option<usize>,
                ) -> ::std::vec::Vec<(&'static str, usage_argv::spec::SettingGiven)> {
                    match selected {
                        #(#givens)*
                        // No subcommand was reached, so none of them was given anything.
                        _ => ::std::vec::Vec::new(),
                    }
                }

                fn any_given(
                    partial: &Self::Partial,
                    selected: ::std::option::Option<usize>,
                ) -> ::std::option::Option<&'static str> {
                    match selected {
                        #(#any_givens)*
                        _ => ::std::option::Option::None,
                    }
                }

                fn exclusive_given(
                    partial: &Self::Partial,
                    selected: ::std::option::Option<usize>,
                ) -> ::std::option::Option<&'static str> {
                    match selected {
                        #(#exclusive_givens)*
                        _ => ::std::option::Option::None,
                    }
                }

                fn apply_env(
                    partial: &mut Self::Partial,
                    selected: ::std::option::Option<usize>,
                ) {
                    match selected {
                        #(#apply_envs)*
                        _ => {}
                    }
                }

                fn check<'t, 'v>(
                    partial: &mut Self::Partial,
                    selected: usize,
                ) -> ::std::result::Result<(), usage_argv::Error<'t, 'v>> {
                    match selected {
                        #(#checks)*
                        // A position that is not one of these cannot be produced: it comes
                        // from finding a table's own address in COMMANDS.
                        _ => ::std::result::Result::Ok(()),
                    }
                }

                fn select<'t, 'v>(
                    partial: Self::Partial,
                    selected: usize,
                ) -> ::std::result::Result<
                    ::std::option::Option<Self>,
                    usage_argv::Error<'t, 'v>,
                > {
                    match selected {
                        #(#selects)*
                        _ => ::std::result::Result::Ok(::std::option::Option::None),
                    }
                }
            }
        };
    }
}

/// `&& !partial.__overridden_x`, for a field that can lose an override.
///
/// Empty for every other field, so a CLI that declares no `overrides` generates exactly
/// what it did before.
fn displaced_guard(cli: &Cli, field: &Field) -> TokenStream {
    if !is_displaceable(cli, field) {
        return quote!();
    }
    let overridden = format_ident!("__overridden_{}", field.ident);
    quote!(&& !partial.#overridden)
}

/// The groups a command declares, in the order their first member is written.
///
/// Membership lives on the fields and properties on the struct, so this is where the two
/// are joined — and it is the only place both are visible, which is why the emitted
/// metadata is built here rather than in the model.
fn declared_groups(cli: &Cli) -> Vec<(String, bool, bool, Vec<String>)> {
    let mut groups: Vec<(String, bool, bool, Vec<String>)> = Vec::new();
    for field in &cli.fields {
        let Some(name) = field.group.as_deref() else {
            continue;
        };
        let Some(selector) = Cli::selector_for_field(field) else {
            continue;
        };
        match groups.iter_mut().find(|(n, _, _, _)| n == name) {
            Some((_, _, _, members)) => members.push(selector),
            None => {
                // An undeclared group takes the defaults, which is the common case: "at
                // most one of these" needs no properties, and making it say so anyway
                // would be ceremony.
                let decl = cli.groups.iter().find(|d| d.name == name);
                groups.push((
                    name.to_string(),
                    decl.is_some_and(|d| d.required),
                    decl.is_some_and(|d| d.multiple),
                    vec![selector],
                ));
            }
        }
    }
    groups
}

/// The `static` array of group metadata, and the expression referring to it.
///
/// A flattened struct's groups are joined in, the way its flags and their metadata are:
/// the child enforces them through its own `check`, and its flags are in *this* command's
/// table, so its groups describe this command and belong in this command's emitted KDL.
/// Leaving them out would enforce a rule the spec does not mention — the drift the
/// spec-as-definition rule exists to prevent.
fn group_meta_table(cli: &Cli) -> (TokenStream, TokenStream) {
    let groups = declared_groups(cli);
    // Where each group's first member was written, which is the position `declared_groups`
    // already orders them by. A group whose members straddle a flattened field still belongs
    // where it *starts*, so it keeps its whole member list rather than being split in two.
    let first_member_at: Vec<usize> = groups
        .iter()
        .map(|(name, _, _, _)| {
            cli.fields
                .iter()
                .position(|f| {
                    f.group.as_deref() == Some(name.as_str())
                        && Cli::selector_for_field(f).is_some()
                })
                .unwrap_or(usize::MAX)
        })
        .collect();
    let entry = |(name, required, multiple, members): &(String, bool, bool, Vec<String>)| {
        quote! {
            usage_argv::spec::GroupMeta {
                name: #name,
                members: &[#(#members),*],
                required: #required,
                multiple: #multiple,
            }
        }
    };

    // One walk over the fields, so a flattened struct's groups land where the field was
    // written rather than after everything this struct declares — the same interleaving the
    // flag and argument tables are built with, and visible in the same places their order is.
    let mut parts: Vec<TokenStream> = Vec::new();
    let mut run: Vec<TokenStream> = Vec::new();
    let mut emitted = vec![false; groups.len()];
    let mut any_flattened = false;
    for (i, field) in cli.fields.iter().enumerate() {
        let Kind::Flatten { ty } = &field.kind else {
            continue;
        };
        any_flattened = true;
        for (g, group) in groups.iter().enumerate() {
            if !emitted[g] && first_member_at[g] < i {
                emitted[g] = true;
                run.push(entry(group));
            }
        }
        if !run.is_empty() {
            let entries = std::mem::take(&mut run);
            parts.push(quote!(&[#(#entries),*]));
        }
        // Named directly, as the flag and argument tables beside this one are: the
        // generated items live in the user's own scope now rather than in a module
        // above it, so there is no path to rewrite.
        parts.push(quote!(<#ty as usage_argv::spec::CommandArgs>::META.groups));
    }
    for (g, group) in groups.iter().enumerate() {
        if !emitted[g] {
            run.push(entry(group));
        }
    }
    if !run.is_empty() {
        parts.push(quote!(&[#(#run),*]));
    }

    if parts.is_empty() {
        return (quote!(), quote!(&[]));
    }
    if !any_flattened {
        let len = groups.len();
        let entries = groups.iter().map(entry);
        return (
            quote! {
                pub static GROUP_METAS: [usage_argv::spec::GroupMeta; #len] = [#(#entries),*];
            },
            quote!(&GROUP_METAS),
        );
    }
    (
        quote! {
            const GROUP_META_GROUPS: &[&[usage_argv::spec::GroupMeta<'static>]] =
                &[#(#parts),*];
            static GROUP_METAS: [usage_argv::spec::GroupMeta<'static>;
                usage_argv::table_len(GROUP_META_GROUPS)] =
                usage_argv::spec::concat_group_metas(GROUP_META_GROUPS);
        },
        quote!(&GROUP_METAS),
    )
}

/// Apply declared defaults to this command and every argument group flattened into it.
///
/// Separate from the rest of `check` because exclusivity suppresses requiredness, not values a
/// CLI promised to provide by default. A parent can therefore prepare an opaque flattened partial
/// without also asking it to report a missing sibling.
fn declared_defaults(cli: &Cli) -> TokenStream {
    let own = cli.fields.iter().filter_map(|f| {
        if matches!(f.kind, Kind::Subcommand { .. } | Kind::Skip) {
            return None;
        }
        if f.default.is_empty() && f.default_if.is_empty() {
            return None;
        }
        let given = format_ident!("__given_{}", f.ident);
        let standing = displaced_guard(cli, f);
        let mut fills: Vec<TokenStream> = f
            .default_if
            .iter()
            .map(|condition| {
                let pred = default_if_predicate(cli, condition);
                let assign =
                    assign_literal(f, &condition.value, std::slice::from_ref(&condition.value));
                quote!(if !__usage_filled && (#pred) {
                    #assign
                    __usage_filled = true;
                })
            })
            .collect();
        if !f.default.is_empty() {
            let assign = reset_to_default(f);
            fills.push(quote!(if !__usage_filled {
                #assign
            }));
        }
        Some(quote! {
            if !partial.#given #standing {
                let mut __usage_filled = false;
                #(#fills)*
            }
        })
    });
    let flattened = cli.fields.iter().filter_map(|f| {
        let Kind::Flatten { ty } = &f.kind else {
            return None;
        };
        let ident = &f.ident;
        Some(quote! {
            <#ty as usage_argv::spec::CommandArgs>::apply_defaults(&mut partial.#ident);
        })
    });
    quote! {
        #(#own)*
        #(#flattened)*
    }
}

/// Environment fallbacks for this command and every argument group flattened into it.
///
/// Kept separate from the rest of post-binding validation because a parent must apply these
/// before it can enforce a relationship whose other side lives across a flatten boundary.
fn env_fallbacks(cli: &Cli) -> TokenStream {
    let own = cli.fields.iter().filter_map(|f| {
        let ident = &f.ident;
        let given = format_ident!("__given_{}", ident);
        let var = f.env.as_deref()?;
        let assign = match f.shape {
            // `env::var` gives text, which is right for an environment variable: the
            // partial holds bytes because *argv* may not be UTF-8, and this is not argv.
            Shape::Optional => quote! {
                partial.#ident = ::std::option::Option::Some(value.into_bytes());
            },
            Shape::Required => quote!(partial.#ident = value.into_bytes();),
            // Cleared first, so the environment *replaces* a declared default instead of
            // adding to it — which is what every other shape does by assigning.
            Shape::Many => match f.delimiter {
                Some(delimiter) => {
                    let byte = u8::try_from(u32::from(delimiter))
                        .expect("the model rejects non-ASCII delimiters");
                    quote! {
                        partial.#ident.clear();
                        for part in value.as_bytes().split(|b| *b == #byte) {
                            partial.#ident.push(part.to_vec());
                        }
                    }
                }
                None => quote! {
                    partial.#ident.clear();
                    partial.#ident.push(value.into_bytes());
                },
            },
            Shape::Bool => quote! {
                partial.#ident = !matches!(
                    value.as_str(),
                    "" | "0" | "false" | "no" | "off"
                );
            },
            // An unparseable count leaves the field alone rather than counting as given.
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
        // A flag that lost an override is not merely unset: filling it from the
        // environment would undo the last-one-wins the command line asked for.
        let standing = displaced_guard(cli, f);
        Some(quote! {
            if !partial.#given #standing {
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
    let flattened = cli.fields.iter().filter_map(|f| {
        let Kind::Flatten { ty } = &f.kind else {
            return None;
        };
        let ident = &f.ident;
        Some(quote! {
            <#ty as usage_argv::spec::CommandArgs>::apply_env(&mut partial.#ident);
        })
    });
    let selected = cli.fields.iter().find_map(|f| {
        let Kind::Subcommand { ty, .. } = &f.kind else {
            return None;
        };
        Some(quote! {
            <#ty as usage_argv::spec::Subcommands>::apply_env(
                &mut partial.__usage_sub,
                partial.__usage_selected,
            );
        })
    });
    quote! {
        #(#own)*
        #(#flattened)*
        #selected
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
    let direct_exclusive_present = cli.fields.iter().filter_map(|field| {
        if !field.exclusive {
            return None;
        }
        let given = format_ident!("__given_{}", field.ident);
        Some(quote!(partial.#given))
    });
    let flattened_exclusive_present = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            <#ty as usage_argv::spec::CommandArgs>::exclusive_given(&partial.#ident).is_some()
        })
    });
    let selected_exclusive_present = cli.fields.iter().filter_map(|field| {
        let Kind::Subcommand { ty, .. } = &field.kind else {
            return None;
        };
        Some(quote! {
            <#ty as usage_argv::spec::Subcommands>::exclusive_given(
                &partial.__usage_sub,
                partial.__usage_selected,
            ).is_some()
        })
    });
    let exclusive_present = quote! {
        false
            #(|| #direct_exclusive_present)*
            #(|| #flattened_exclusive_present)*
            #(|| #selected_exclusive_present)*
    };
    // A flattened struct declares its own required-ness and choices, and only it knows them.
    //
    // Run before this command's own required-ness, on the same principle that puts conflicts
    // first: what the user typed wrong is more useful to hear about than what they left out.
    // `config --format yaml` should say `yaml` is not one of the choices, even if `--file` is
    // also missing.
    //
    // No finer promise than that. These checks are grouped by kind rather than by field, so
    // there is no "in declaration order" to offer — a flattened group's errors interleave with
    // this command's by kind, not by where the field was written.
    let flattened_checks = cli.fields.iter().filter_map(|f| {
        let Kind::Flatten { ty } = &f.kind else {
            return None;
        };
        let ident = &f.ident;
        Some(quote! {
            if !__usage_exclusive_present
                || <#ty as usage_argv::spec::CommandArgs>::exclusive_given(&partial.#ident)
                    .is_some()
            {
                <#ty as usage_argv::spec::CommandArgs>::check(&mut partial.#ident)?;
            }
        })
    });
    let duplicate_checks = cli.fields.iter().filter(|f| rejects_duplicate(f)).map(|f| {
        let duplicated = format_ident!("__duplicated_{}", f.ident);
        let name = &f.name;
        quote! {
            if partial.#duplicated {
                return ::std::result::Result::Err(
                    usage_argv::Error::DuplicateFlag { name: #name },
                );
            }
        }
    });
    // Applied here rather than in `start`, and this is not a detail: `start` builds the
    // partial for *every* command in the CLI, selected or not, so a declared default was
    // costing a `String` per default per command — 60 allocations to parse a bare `mise`,
    // which is the CLI's size leaking into the invocation. `check` runs for the selected
    // command only. The helper also prepares flattened defaults before an exclusive flag can
    // suppress those groups' requiredness checks.
    let declared_defaults = declared_defaults(cli);

    let env_fallbacks = env_fallbacks(cli);

    let required_checks = cli.fields.iter().filter_map(|f| {
        // A `String` has nowhere to put "absent", so the type is the declaration; a collection
        // has nothing in its type to say it and declares `required` instead.
        //
        // The same expression the metadata is built from, deliberately: checking only the shape
        // meant a `Vec` marked `required` was reported as one-or-more by the spec, the help, the
        // manpage and the completions, and accepted zero values from the CLI that actually ran.
        // One expression cannot disagree with itself.
        if !(f.shape == Shape::Required || f.required_collection) || !f.default.is_empty() {
            return None;
        }
        let given = format_ident!("__given_{}", f.ident);
        let name = &f.name;
        // Same reason as the environment: a displaced flag was answered by the one that
        // displaced it, so it is not missing.
        let standing = displaced_guard(cli, f);
        Some(quote! {
            if !partial.#given #standing {
                return ::std::result::Result::Err(
                    usage_argv::Error::MissingRequired { name: #name },
                );
            }
        })
    });

    let choice_checks = cli.fields.iter().filter_map(|f| {
        if f.choices.is_empty() && !f.value_enum {
            return None;
        }
        let ident = &f.ident;
        let name = &f.name;
        // A `value_enum`'s words live on the type. Checking against them here rather than
        // letting the conversion fail is what makes a wrong word an `InvalidChoice` that
        // lists what was expected, instead of a message about a type the user did not name.
        let choices: TokenStream = match (f.value_enum, f.value_ty.as_ref()) {
            (true, Some(ty)) => {
                quote!(<#ty as usage_argv::spec::ValueEnum>::CHOICES)
            }
            _ => {
                let list = &f.choices;
                quote!(&[#(#list),*])
            }
        };
        let accepted_choices: TokenStream = match (f.value_enum, f.value_ty.as_ref()) {
            (true, Some(ty)) => quote!(<#ty as usage_argv::spec::ValueEnum>::ACCEPTED_CHOICES),
            _ => quote!(#choices),
        };
        let ignore_case: TokenStream = match (f.value_enum, f.value_ty.as_ref()) {
            (true, Some(ty)) => quote!(<#ty as usage_argv::spec::ValueEnum>::IGNORE_CASE),
            _ => quote!(false),
        };
        let values = match f.shape {
            Shape::Optional => quote!(partial.#ident.iter()),
            Shape::Required => quote!(::std::iter::once(&partial.#ident)),
            Shape::Many => quote!(partial.#ident.iter()),
            // Rejected in the model: there is no value to check.
            Shape::Bool | Shape::Count => return None,
        };
        Some(quote! {
            for value in #values {
                // Compared as text, since a choice is a word.
                //
                // Bytes that are not UTF-8 are passed over rather than reported here. They
                // are not any of the choices, but saying so would answer the wrong question:
                // `InvalidChoice` lists words, and this value is not a word at all. Left
                // alone, it reaches `build`, which reports the UTF-8 failure with the value
                // in it. Comparing the empty string instead — which is what `unwrap_or_default`
                // did — made every such value collide with the choices check first.
                let ::std::result::Result::Ok(__usage_text) = ::std::str::from_utf8(value)
                else {
                    continue;
                };
                if !usage_argv::spec::choice_matches(#accepted_choices, __usage_text, #ignore_case) {
                    return ::std::result::Result::Err(
                        usage_argv::Error::InvalidChoice {
                            name: #name,
                            choices: #choices,
                        },
                    );
                }
            }
        })
    });

    let validation_checks = cli.fields.iter().filter_map(|f| {
        let expression = f.validate.as_ref()?;
        let ident = &f.ident;
        let name = &f.name;
        let message = f
            .validate_error
            .as_deref()
            .unwrap_or("does not satisfy the validation expression");
        let values = match f.shape {
            Shape::Optional => quote!(partial.#ident.iter()),
            Shape::Required => quote!(::std::iter::once(&partial.#ident)),
            Shape::Many => quote!(partial.#ident.iter()),
            Shape::Bool | Shape::Count => return None,
        };
        Some(quote! {
            for value in #values {
                let ::std::result::Result::Ok(__usage_text) = ::std::str::from_utf8(value)
                else {
                    // The field conversion reports non-UTF-8 with the original bytes. An expr
                    // variable is text, so claiming the validation failed would hide that more
                    // precise error.
                    continue;
                };
                let __usage_reason = match usage_validation::validate(#expression, __usage_text) {
                    ::std::result::Result::Ok(true) => continue,
                    ::std::result::Result::Ok(false) => #message.to_string(),
                    ::std::result::Result::Err(error) =>
                        ::std::format!("validation expression failed: {error}"),
                };
                return ::std::result::Result::Err(
                    usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                        usage_argv::InvalidValue {
                            name: #name,
                            value: __usage_text.to_string(),
                            reason: __usage_reason,
                        },
                    )),
                );
            }
        })
    });

    let bound_checks = cli.fields.iter().filter_map(|f| {
        let (var_min, var_max) = if matches!(f.kind, Kind::Flag { variadic: true, .. }) {
            (f.value_var_min, f.value_var_max)
        } else {
            (f.var_min, f.var_max)
        };
        if (var_min.is_none() && var_max.is_none()) || f.shape != Shape::Many {
            return None;
        }
        let ident = &f.ident;
        let name = &f.name;
        let min = match var_min {
            Some(min) => quote! {
                if got < #min {
                    return ::std::result::Result::Err(
                        usage_argv::Error::VarTooFew { name: #name, min: #min, got },
                    );
                }
            },
            None => quote!(),
        };
        // `var_max` is a binding limit for anything that *collects*: a variadic stops at it
        // and the next field takes the rest, so a total above it cannot arise and this
        // check would be unreachable. Worse than unreachable for a repeatable flag whose
        // argument is variadic — each occurrence respects the limit, and the total across
        // occurrences would fail a check the invocation never broke.
        //
        // What is left is the repeatable flag with a single-value argument, where the bound
        // counts occurrences: nothing about one token can decide that, so it stays here.
        let counts_occurrences = matches!(
            &f.kind,
            Kind::Flag {
                variadic: false,
                ..
            }
        ) && f.repeatable;
        let max = match var_max.filter(|_| counts_occurrences) {
            Some(max) => quote! {
                if got > #max {
                    return ::std::result::Result::Err(
                        usage_argv::Error::VarTooMany { name: #name, max: #max, got },
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

    // A conflict asks whether two flags both ended up with a value, which is why it
    // reads `__given_*` rather than the fields themselves: a `bool` flag that was given
    // as `false` is still a flag the user asked for. Env fallback has already run, so a
    // value from the environment counts the same as a typed one — clap does that too,
    // and an asymmetric rule would make the same pair a conflict or not depending on
    // which side happened to be typed.
    // Judge flag/flag conflicts before relationships involving positionals. clap reports the
    // two switches the user explicitly combined before a positional that happens to conflict
    // with either one, regardless of where the positional field was declared. That is also the
    // actionable pair: `start --all --local id` should point at `--all` and `--local`, not let
    // the trailing `id` hide their contradiction.
    let conflict_fields = cli
        .fields
        .iter()
        .filter(|field| matches!(field.kind, Kind::Flag { .. }))
        .chain(
            cli.fields
                .iter()
                .filter(|field| !matches!(field.kind, Kind::Flag { .. })),
        );
    let conflict_checks = conflict_fields.flat_map(move |f| {
        let given = format_ident!("__given_{}", f.ident);
        let name = &f.name;
        f.conflicts.iter().map(move |selector| {
            if let Some(other) = cli.field_for_selector(selector) {
                let other_given = format_ident!("__given_{}", other.ident);
                let other_name = &other.name;
                quote! {
                    if partial.#given && partial.#other_given {
                        return ::std::result::Result::Err(
                            usage_argv::Error::ConflictingFlags {
                                name: #name,
                                other: #other_name,
                            },
                        );
                    }
                }
            } else {
                quote! {
                    if partial.#given {
                        if let ::std::option::Option::Some(other) = argument_state(partial, #selector) {
                            if other.given {
                                return ::std::result::Result::Err(
                                    usage_argv::Error::ConflictingFlags {
                                        name: #name,
                                        other: other.name,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        })
    });

    // The positive form, and the same `__given_*` reading for the same reasons: a `bool`
    // given as `false` is still a flag the user asked for, and env fallback has already
    // run, so a requirement satisfied from the environment is satisfied.
    //
    // Reported as the *other* flag being missing rather than as something wrong with the
    // one that named it, which is what clap says: an unmet `requires` is a required
    // argument that was not provided. That also means no new `Error` variant — the hot
    // path's `Result` does not grow to carry a check that only ever fires on the cold one.
    //
    // A target with a default is skipped outright, since it can never be missing.
    let requirement_checks = cli.fields.iter().flat_map(move |f| {
        let given = format_ident!("__given_{}", f.ident);
        f.requires.iter().map(move |selector| {
            let Some(other) = cli.field_for_selector(selector) else {
                return quote! {
                    if partial.#given {
                        match argument_state(partial, #selector) {
                            ::std::option::Option::Some(other) if other.satisfied => {}
                            ::std::option::Option::Some(other) => {
                                return ::std::result::Result::Err(
                                    usage_argv::Error::MissingRequired { name: other.name },
                                );
                            }
                            ::std::option::Option::None => {}
                        }
                    }
                };
            };
            // A flag with a default always has a value, so the requirement it satisfies
            // can never fail — the same reason plain required-ness skips such a field.
            // Decided at compile time, so the check is not merely always-true at run
            // time, it is not there.
            if !other.default.is_empty() {
                return quote!();
            }
            let other_given = format_ident!("__given_{}", other.ident);
            let other_name = &other.name;
            let missing = quote! {
                return ::std::result::Result::Err(
                    usage_argv::Error::MissingRequired { name: #other_name },
                );
            };
            match default_if_would_apply(cli, other) {
                Some(pred) => quote! {
                    if partial.#given && !partial.#other_given && !(#pred) {
                        #missing
                    }
                },
                None => quote! {
                    if partial.#given && !partial.#other_given {
                        #missing
                    }
                },
            }
        })
    });

    // Only an explicit matching value activates this form. Defaults are applied above
    // without setting `__given_*`; argv and environment fallback set it. The partial
    // still holds bytes, so matching does not parse or otherwise reinterpret a value.
    let conditional_requirement_checks = cli.fields.iter().flat_map(move |f| {
        let given = format_ident!("__given_{}", f.ident);
        let ident = &f.ident;
        f.requires_if.iter().map(move |condition| {
            let external = cli.field_for_selector(&condition.requires).is_none();
            let other = cli.field_for_selector(&condition.requires);
            if external {
                let selector = &condition.requires;
                let value = &condition.value;
                let matches = match f.shape {
                    Shape::Optional => {
                        quote!(partial.#ident.as_deref().is_some_and(|v| v == #value.as_bytes()))
                    }
                    Shape::Required => quote!(partial.#ident.as_slice() == #value.as_bytes()),
                    Shape::Many => {
                        quote!(partial.#ident.iter().any(|v| v.as_slice() == #value.as_bytes()))
                    }
                    Shape::Bool => match value.as_str() {
                        "true" => quote!(partial.#ident),
                        "false" => quote!(!partial.#ident),
                        _ => quote!(false),
                    },
                    Shape::Count => match value.as_str() {
                        "true" => quote!(partial.#ident > 0),
                        "false" => quote!(partial.#ident == 0),
                        _ => quote!(false),
                    },
                };
                return quote! {
                    if partial.#given && #matches {
                        match argument_state(partial, #selector) {
                            ::std::option::Option::Some(other) if other.satisfied => {}
                            ::std::option::Option::Some(other) => {
                                return ::std::result::Result::Err(
                                    usage_argv::Error::MissingRequired { name: other.name },
                                );
                            }
                            ::std::option::Option::None => {}
                        }
                    }
                };
            }
            let other = other.expect("the local relationship was resolved above");
            if !other.default.is_empty() {
                return quote!();
            }
            let other_given = format_ident!("__given_{}", other.ident);
            let other_name = &other.name;
            let value = &condition.value;
            let matches = match f.shape {
                Shape::Optional => quote!(
                    partial.#ident.as_deref().is_some_and(|v| v == #value.as_bytes())
                ),
                Shape::Required => quote!(partial.#ident.as_slice() == #value.as_bytes()),
                Shape::Many => quote!(
                    partial.#ident.iter().any(|v| v.as_slice() == #value.as_bytes())
                ),
                Shape::Bool => match value.as_str() {
                    "true" => quote!(partial.#ident),
                    "false" => quote!(!partial.#ident),
                    _ => quote!(false),
                },
                Shape::Count => match value.as_str() {
                    "true" => quote!(partial.#ident > 0),
                    "false" => quote!(partial.#ident == 0),
                    _ => quote!(false),
                },
            };
            let unless_default_if = match default_if_would_apply(cli, other) {
                Some(pred) => quote!(&& !(#pred)),
                None => quote!(),
            };
            quote! {
                if partial.#given && #matches && !partial.#other_given #unless_default_if {
                    return ::std::result::Result::Err(
                        usage_argv::Error::MissingRequired { name: #other_name },
                    );
                }
            }
        })
    });

    // An exclusive flag is a conflict with the whole command rather than with a named
    // flag, so it is written as one check per *other* declaration — positionals included,
    // which is what makes it more than being in a group with every other flag.
    //
    // Only what was given counts, as `conflicts` reads it: a defaulted field standing
    // beside an exclusive flag is nobody saying anything, and counting it would make the
    // flag unusable on any command that has a default.
    let exclusive_checks = cli
        .fields
        .iter()
        .filter(|f| f.exclusive)
        .flat_map(move |f| {
            let given = format_ident!("__given_{}", f.ident);
            let name = &f.name;
            cli.fields
                .iter()
                .filter(move |other| other.ident != f.ident)
                .filter(|other| {
                    !matches!(
                        other.kind,
                        Kind::Subcommand { .. } | Kind::Flatten { .. } | Kind::Skip
                    )
                })
                .map(move |other| {
                    let other_given = format_ident!("__given_{}", other.ident);
                    let other_name = &other.name;
                    quote! {
                        if partial.#given && partial.#other_given {
                            return ::std::result::Result::Err(
                                usage_argv::Error::ConflictingFlags {
                                    name: #other_name,
                                    other: #name,
                                },
                            );
                        }
                    }
                })
                .collect::<Vec<_>>()
        });

    // A flattened partial is intentionally opaque to its parent. Ask through
    // `CommandArgs` whether either side of an exclusive relationship was given, so the
    // relationship does not disappear merely because the declarations live in reusable
    // `Args`. A selected subcommand is itself another declaration in this command; its
    // contents remain in the child command's own scope.
    let direct_given = cli
        .fields
        .iter()
        .filter(|field| {
            !matches!(
                field.kind,
                Kind::Flatten { .. } | Kind::Subcommand { .. } | Kind::Skip
            )
        })
        .rev()
        .fold(quote!(::std::option::Option::None), |rest, field| {
            let given = format_ident!("__given_{}", field.ident);
            let name = &field.name;
            quote!(if partial.#given { ::std::option::Option::Some(#name) } else { #rest })
        });
    let direct_exclusive = cli
        .fields
        .iter()
        .filter(|field| field.exclusive)
        .rev()
        .fold(quote!(::std::option::Option::None), |rest, field| {
            let given = format_ident!("__given_{}", field.ident);
            let name = &field.name;
            quote!(if partial.#given { ::std::option::Option::Some(#name) } else { #rest })
        });
    let has_flatten = cli
        .fields
        .iter()
        .any(|field| matches!(field.kind, Kind::Flatten { .. }));
    let flattened_segments = cli.fields.iter().filter_map(|field| {
        let Kind::Flatten { ty } = &field.kind else {
            return None;
        };
        let ident = &field.ident;
        Some(quote! {
            (
                <#ty as usage_argv::spec::CommandArgs>::any_given(&partial.#ident),
                <#ty as usage_argv::spec::CommandArgs>::exclusive_given(&partial.#ident),
            ),
        })
    });
    let subcommand_segment = cli.fields.iter().find_map(|field| {
        let Kind::Subcommand { ty, .. } = &field.kind else {
            return None;
        };
        let name = &field.name;
        Some(quote! {
            (
                partial.__usage_selected.map(|_| #name),
                <#ty as usage_argv::spec::Subcommands>::exclusive_given(
                    &partial.__usage_sub,
                    partial.__usage_selected,
                ),
            ),
        })
    });
    let exclusive_cross_checks = (has_flatten || subcommand_segment.is_some()).then(|| {
        quote! {
            let __usage_exclusive_segments = [
                (#direct_given, #direct_exclusive),
                #(#flattened_segments)*
                #subcommand_segment
            ];
            for __usage_i in 0..__usage_exclusive_segments.len() {
                if let ::std::option::Option::Some(exclusive) =
                    __usage_exclusive_segments[__usage_i].1
                {
                    for __usage_j in 0..__usage_exclusive_segments.len() {
                        if __usage_i == __usage_j {
                            continue;
                        }
                        if let ::std::option::Option::Some(other) =
                            __usage_exclusive_segments[__usage_j].0
                        {
                            return ::std::result::Result::Err(
                                usage_argv::Error::ConflictingFlags {
                                    name: other,
                                    other: exclusive,
                                },
                            );
                        }
                    }
                }
            }
        }
    });

    // Groups, checked once per group rather than per member: both questions a group asks
    // — how many members were given, and whether that is enough — are about the set.
    //
    // The two halves read a default differently, deliberately, and the same way
    // usage-lib does. Exclusivity counts what was supplied, or a defaulted member would
    // collide with the sibling the user typed; requiredness asks whether a member ended
    // up with a value, and a default is a value.
    let group_checks = declared_groups(cli)
        .into_iter()
        .map(|(name, required, multiple, members)| {
            let fields: Vec<&Field> = members
                .iter()
                .filter_map(|selector| cli.field_for_selector(selector))
                .collect();
            let given: Vec<TokenStream> = fields
                .iter()
                .map(|f| {
                    let given = format_ident!("__given_{}", f.ident);
                    quote!(partial.#given)
                })
                .collect();
            // A member with a default always has a value, so the group can never be
            // unsatisfied. Decided here rather than at run time, as `requires` is.
            let always_filled = fields.iter().any(|f| !f.default.is_empty());
            let exclusivity = (!multiple).then(|| {
                // Reported as the first two that were given, which is the pair the user has
                // to choose between. `ConflictingFlags` rather than a group-shaped error:
                // what went wrong is that two flags were given together, which is exactly
                // what that error says.
                let names: Vec<&String> = fields.iter().map(|f| &f.name).collect();
                let pairs = (0..fields.len()).flat_map(|i| {
                    let (later, earlier) = (given.clone(), given.clone());
                    let (later_names, earlier_names) = (names.clone(), names.clone());
                    ((i + 1)..fields.len())
                        .map(move |j| {
                            let (a, b) = (&earlier[i], &later[j]);
                            let (name_a, name_b) = (earlier_names[i], later_names[j]);
                            quote! {
                                if #a && #b {
                                    return ::std::result::Result::Err(
                                        usage_argv::Error::ConflictingFlags {
                                            name: #name_b,
                                            other: #name_a,
                                        },
                                    );
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                });
                quote!(#(#pairs)*)
            });
            let requiredness = (required && !always_filled).then(|| {
                let selectors = &members;
                quote! {
                    if !(#(#given)||*) {
                        return ::std::result::Result::Err(
                            usage_argv::Error::MissingGroup {
                                group: #name,
                                members: &[#(#selectors),*],
                            },
                        );
                    }
                }
            });
            (exclusivity, requiredness)
        })
        .collect::<Vec<_>>();
    // Two passes rather than one block per group, because the order between *kinds* of
    // check is the one this function promises: what the user typed wrong before what
    // they left out. Emitted together, an earlier group's `MissingGroup` would answer
    // before a later group's `ConflictingFlags` — and before a flattened child's, since
    // those run later still.
    let group_exclusivity_checks: Vec<TokenStream> =
        group_checks.iter().filter_map(|(e, _)| e.clone()).collect();
    let group_required_checks: Vec<TokenStream> =
        group_checks.iter().filter_map(|(_, r)| r.clone()).collect();

    // `required_if` and `required_unless` are the same question asked two ways: which
    // other flags decide whether this one had to be given. Neither needs to know the
    // order they arrived in — only whether they arrived — so both are answered here,
    // beside plain required-ness, from the same `__given_*` flags.
    let relationship_required_checks = cli.fields.iter().filter_map(move |f| {
        if f.required_if.is_empty()
            && f.required_if_eq.is_empty()
            && f.required_if_eq_all.is_empty()
            && f.required_unless.is_empty()
            && f.required_unless_all.is_empty()
        {
            return None;
        }
        // A field with a default is already filled, so no condition can make it
        // missing. Plain required-ness skips these too, and so does usage-lib.
        if !f.default.is_empty() {
            return None;
        }
        let given = format_ident!("__given_{}", f.ident);
        let name = &f.name;
        let selector_given = |selector: &String| {
            Some(match cli.field_for_selector(selector) {
                Some(other) => {
                    let other_given = format_ident!("__given_{}", other.ident);
                    quote!(partial.#other_given)
                }
                None => quote!(
                    argument_state(partial, #selector).is_some_and(|state| state.given)
                ),
            })
        };
        let if_given: Vec<_> = f.required_if.iter().filter_map(selector_given).collect();
        let unless_given: Vec<_> = f
            .required_unless
            .iter()
            .filter_map(selector_given)
            .collect();
        let if_eq: Vec<_> = f
            .required_if_eq
            .iter()
            .map(|condition| {
                let selector = cli
                    .field_for_selector(&condition.selector)
                    .and_then(Cli::selector_for_field)
                    .unwrap_or_else(|| condition.selector.clone());
                let value = &condition.value;
                quote!(argument_matches(partial, #selector, #value.as_bytes()).unwrap_or(false))
            })
            .collect();
        let if_eq_all: Vec<_> = f
            .required_if_eq_all
            .iter()
            .map(|condition| {
                let selector = cli
                    .field_for_selector(&condition.selector)
                    .and_then(Cli::selector_for_field)
                    .unwrap_or_else(|| condition.selector.clone());
                let value = &condition.value;
                quote!(argument_matches(partial, #selector, #value.as_bytes()).unwrap_or(false))
            })
            .collect();
        let unless_all_given: Vec<_> = f
            .required_unless_all
            .iter()
            .filter_map(selector_given)
            .collect();
        // Absent, with nothing standing in for it: a default or an environment
        // variable has already filled the field and set `__given_*`.
        let missing = quote! {
            return ::std::result::Result::Err(
                usage_argv::Error::MissingRequired { name: #name },
            );
        };
        let required_if = (!if_given.is_empty()).then(|| {
            quote! {
                if #(#if_given)||* {
                    #missing
                }
            }
        });
        let required_if_eq = (!if_eq.is_empty()).then(|| {
            quote! {
                if #(#if_eq)||* {
                    #missing
                }
            }
        });
        let required_if_eq_all = (!if_eq_all.is_empty()).then(|| {
            quote! {
                if #(#if_eq_all)&&* {
                    #missing
                }
            }
        });
        let required_unless =
            (!unless_given.is_empty() || !unless_all_given.is_empty()).then(|| {
                let any = if unless_given.is_empty() {
                    quote!(false)
                } else {
                    quote!(#(#unless_given)||*)
                };
                let all = if unless_all_given.is_empty() {
                    quote!(false)
                } else {
                    quote!(#(#unless_all_given)&&*)
                };
                quote! {
                    if !(#any || #all) {
                        #missing
                    }
                }
            });
        Some(quote! {
            if !partial.#given {
                #required_if
                #required_if_eq
                #required_if_eq_all
                #required_unless
            }
        })
    });

    quote! {
        // Environment first, so a `default_if` can see a sibling filled from
        // env, and so the environment still overrides an unconditional default.
        #env_fallbacks
        #declared_defaults
        let __usage_exclusive_present = #exclusive_present;
        #(#duplicate_checks)*
        // Before required-ness: "you gave two flags that cannot go together" is the
        // more useful of the two answers when a conflict has also left something
        // unfilled, and it is the one usage-lib reports.
        #(#conflict_checks)*
        #(#exclusive_checks)*
        #exclusive_cross_checks
        #(#group_exclusivity_checks)*
        #(#flattened_checks)*
        // An exclusive occurrence is the command's escape from requiredness, just as in clap:
        // `--version` remains usable on a command that otherwise requires an input. Conflicts
        // and other validation still ran above; only errors about absent siblings are skipped.
        if !__usage_exclusive_present {
            #(#requirement_checks)*
            #(#conditional_requirement_checks)*
            #(#required_checks)*
            #(#group_required_checks)*
            #(#relationship_required_checks)*
        }
        #(#choice_checks)*
        #(#validation_checks)*
        #(#bound_checks)*
        #sub_check
    }
}

/// The word list for a value enum.
///
/// Conversion remains the type's own `FromStr` implementation, like every other typed
/// field. This derive owns only CLI metadata, so it can coexist with a domain parser whose
/// accepted syntax or error type is broader than the choices presented in help.
pub fn emit_value_enum(value_enum: &ValueEnum) -> TokenStream {
    let ident = &value_enum.ident;
    let runtime = runtime_path();
    let words = value_enum.variants.iter().flat_map(|value| {
        let cfg = &value.cfg_attrs;
        ::std::iter::once((!value.hide).then_some(&value.name))
            .chain(
                value
                    .aliases
                    .iter()
                    .map(|alias| (!alias.hide).then_some(&alias.name)),
            )
            .flatten()
            .map(move |word| quote!(#(#cfg)* #word))
    });
    let accepted = value_enum.variants.iter().flat_map(|value| {
        let cfg = &value.cfg_attrs;
        ::std::iter::once(&value.name)
            .chain(value.aliases.iter().map(|alias| &alias.name))
            .map(move |word| quote!(#(#cfg)* #word))
    });
    let aliases = value_enum.variants.iter().flat_map(|value| {
        let canonical = &value.name;
        let cfg = &value.cfg_attrs;
        value.aliases.iter().map(move |alias| {
            let alias = &alias.name;
            quote!(#(#cfg)* (#canonical, #alias))
        })
    });
    let details = value_enum.variants.iter().map(|value| {
        let canonical = &value.name;
        let help = option_str(value.help.as_deref());
        let hide = value.hide;
        let cfg = &value.cfg_attrs;
        let aliases = value.aliases.iter().map(|alias| {
            let name = &alias.name;
            let hide = alias.hide;
            quote!(usage_argv::spec::ChoiceAliasMeta { value: #name, hide: #hide })
        });
        quote!(
            #(#cfg)* usage_argv::spec::ChoiceMeta {
                value: #canonical,
                help: #help,
                hide: #hide,
                aliases: &[#(#aliases),*],
            }
        )
    });
    let ignore_case = value_enum.ignore_case;

    quote! {
        #[doc(hidden)]
        const _: () = {
            use #runtime as usage_argv;

            impl usage_argv::spec::ValueEnum for #ident {
                const CHOICES: &'static [&'static str] = &[#(#words),*];
                const ACCEPTED_CHOICES: &'static [&'static str] = &[#(#accepted),*];
                const ALIASES: &'static [(&'static str, &'static str)] = &[#(#aliases),*];
                const DETAILS: &'static [usage_argv::spec::ChoiceMeta<'static>] = &[#(#details),*];
                const IGNORE_CASE: bool = #ignore_case;
            }
        };
    }
}
