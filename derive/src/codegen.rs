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

use crate::model::{rendered_path, Cli, Field, Kind, Shape, Subcommands, ValueEnum};

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

    let default_subcommand = option_str(cli.default_subcommand.as_deref());
    let restart_token = option_str(cli.restart_token.as_deref());
    let mount = option_str(cli.mount.as_deref());
    let before_help = option_str(cli.before_help.as_deref());
    let before_long_help = option_str(cli.before_long_help.as_deref());
    let after_help = option_str(cli.after_help.as_deref());
    let after_long_help = option_str(cli.after_long_help.as_deref());
    let root_key = key_ident("COMMAND", None);
    let keys = key_consts(&cli.fingerprint, flags.len(), args.len());
    let flag_tables = flags.iter().enumerate().map(|(i, f)| flag_table(i, f));
    let arg_tables = args.iter().enumerate().map(|(i, f)| arg_table(i, f));
    let flag_metas = flags.iter().enumerate().map(|(i, f)| flag_meta(i, f));
    let arg_metas = args.iter().enumerate().map(|(i, f)| arg_meta(i, f));

    // Both the plain slices and, when a field is flattened, the joined arrays.
    let tables = tables(cli);
    let table_decls = &tables.decls;
    let meta_table_decls = &tables.meta_decls;
    let flag_table_ref = &tables.flags;
    let arg_table_ref = &tables.args;
    let flag_meta_table_ref = &tables.flag_metas;
    let arg_meta_table_ref = &tables.arg_metas;

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
    let sub_default = parts
        .as_ref()
        .map(|p| p.default.clone())
        .unwrap_or_default();
    let sub_metas = parts.as_ref().map(|p| p.metas.clone()).unwrap_or_default();
    let sub_build = parts.as_ref().map(|p| p.build.clone()).unwrap_or_default();

    let partial = partial_struct(cli);
    let defaults = partial_defaults(cli, false);
    let apply = apply_fn(cli);
    let post = post_binding(cli);
    // `field: local` rather than the shorthand, because the locals are prefixed:
    // a field called `text` or `parser` would otherwise collide with something the
    // generated code needs.
    let field_finals = cli
        .fields
        .iter()
        .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
        .map(field_final);

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

            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #table_decls

            pub static ROOT: Command = Command {
                unknown_flags: #unknown_flags,
                name: #name,
                key: #root_key,
                flags: #flag_table_ref,
                args: #arg_table_ref,
                #sub_commands
                #sub_default
                ..Command::EMPTY
            };

            #(#flag_metas)*
            #(#arg_metas)*
            #meta_table_decls

            pub static ROOT_META: CommandMeta = CommandMeta {
                cmd: &ROOT,
                about: #about,
                long_about: #long_about,
                restart_token: #restart_token,
                mount: #mount,
                before_help: #before_help,
                before_long_help: #before_long_help,
                after_help: #after_help,
                after_long_help: #after_long_help,
                flags: #flag_meta_table_ref,
                args: #arg_meta_table_ref,
                #sub_metas
                ..CommandMeta::EMPTY
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

            /// Everything decided after the last token.
            ///
            /// In the module rather than beside the parse, so every reference it makes
            /// to the user's own types sits at one consistent scope — the root and a
            /// nested command generate the same code here.
            pub fn check<'t, 'v>(
                partial: &mut Partial,
            ) -> ::std::result::Result<(), ::usage_argv::Error<'t, 'v>> {
                // Read unconditionally: a command that declares nothing to check would
                // otherwise leave the parameter unused in the user's crate, where
                // nobody can silence it.
                let _ = &partial;
                #post
                ::std::result::Result::Ok(())
            }

            pub static SPEC: Spec = Spec {
                name: #name,
                bin: #bin,
                version: #version,
                about: #about,
                long_about: #long_about,
                default_subcommand: #default_subcommand,
                // The root's surrounding text is also the spec's, because a spec has nowhere
                // else to put the root's: `to_kdl` writes it at the top level, and the
                // reference reads text there as the default for *every* page. Emitting it in
                // both places is what makes the two descriptions of one CLI agree — declared
                // here, or parsed back from the KDL this derive writes.
                before_help: #before_help,
                before_long_help: #before_long_help,
                after_help: #after_help,
                after_long_help: #after_long_help,
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

fn flag_table(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("FLAG_{i}");
    let key = key_ident("FLAG", Some(i));
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
    // The bound on one occurrence's values. A repeatable flag's bound counts occurrences
    // instead, which no single token can decide, so that one stays a post-binding check.
    let var_max = match field.var_max.filter(|_| *variadic) {
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

    quote! {
        pub static #name: Flag = Flag {
            key: #key,
            name: #field_name,
            longs: &[#(#longs),*],
            shorts: &[#(#shorts),*],
            negate: #negate,
            takes_value: #takes_value,
            variadic: #variadic,
            var_max: #var_max,
            global: #global,
        };
    }
}

fn arg_table(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("ARG_{i}");
    let key = key_ident("ARG", Some(i));
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
    // A bound stops the variadic while binding, so the argument after it is reachable.
    let var_max = match field.var_max.filter(|_| var) {
        Some(max) => {
            // Saturating, for the reason given on the flag above.
            let max = u32::try_from(max).unwrap_or(u32::MAX);
            quote!(::std::option::Option::Some(#max))
        }
        None => quote!(::std::option::Option::None),
    };

    quote! {
        pub static #name: Arg = Arg {
            key: #key,
            name: #field_name,
            var: #var,
            var_max: #var_max,
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
    let value_name = option_str(field.value_name.as_deref());
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
    // A collecting field's type cannot say whether one value is needed, so `required` may
    // declare it. Every other shape gets its answer from the type.
    let required = field.shape == Shape::Required || field.required_collection;
    let choices = choices_tokens(field);
    let (var_min, var_max) = bounds_tokens(field);
    // Written as declared, in the spec's own spelling, so the emitted KDL says what
    // the struct says.
    let overrides = &field.overrides;
    let conflicts = &field.conflicts;
    let required_if = &field.required_if;
    let required_unless = &field.required_unless;

    quote! {
        pub static #name: FlagMeta = FlagMeta {
            flag: &#table,
            help: #help,
            long_help: #long_help,
            env: #env,
            default: #default,
            help_heading: #help_heading,
            value_name: #value_name,
            hide: #hide,
            count: #count,
            repeatable: #repeatable,
            required: #required,
            choices: #choices,
            var_min: #var_min,
            var_max: #var_max,
            overrides: &[#(#overrides),*],
            conflicts: &[#(#conflicts),*],
            required_if: &[#(#required_if),*],
            required_unless: &[#(#required_unless),*],
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
    // A collecting field's type cannot say whether one value is needed, so `required` may
    // declare it. Every other shape gets its answer from the type.
    let required = field.shape == Shape::Required || field.required_collection;
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
    // From the type when the field says `value_enum`, so the spec, the help and the check
    // all read the list the type declares rather than a copy of it.
    if let (true, Some(ty)) = (field.value_enum, field.value_ty.as_ref()) {
        let ty = in_module(ty);
        return quote!(<#ty as ::usage_argv::spec::ValueEnum>::CHOICES);
    }
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
            ::usage_argv::key_base(::core::module_path!(), #declaration);
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

/// A user's type, named from inside a generated module.
///
/// The generated `mod` is a child of wherever the derive was written, and a name
/// from the parent scope is not in scope inside it — so a reference to the user's own
/// type has to say `super::`. An absolute path already resolves from anywhere and is
/// left alone.
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
                let ty = in_module(ty);
                flag_groups.push(quote!(<#ty as ::usage_argv::spec::CommandArgs>::COMMAND.flags));
                arg_groups.push(quote!(<#ty as ::usage_argv::spec::CommandArgs>::COMMAND.args));
                flag_meta_groups.push(quote!(<#ty as ::usage_argv::spec::CommandArgs>::META.flags));
                arg_meta_groups.push(quote!(<#ty as ::usage_argv::spec::CommandArgs>::META.args));
            }
            Kind::Subcommand { .. } => {}
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
            const FLAG_GROUPS: &[&[&::usage_argv::Flag<'static>]] = &[#(#flag_groups),*];
            const ARG_GROUPS: &[&[&::usage_argv::Arg<'static>]] = &[#(#arg_groups),*];
            static FLAGS: [&::usage_argv::Flag<'static>;
                ::usage_argv::table_len(FLAG_GROUPS)] =
                ::usage_argv::concat_flags(FLAG_GROUPS);
            static ARGS: [&::usage_argv::Arg<'static>; ::usage_argv::table_len(ARG_GROUPS)] =
                ::usage_argv::concat_args(ARG_GROUPS);
        },
        meta_decls: quote! {
            const FLAG_META_GROUPS: &[&[::usage_argv::spec::FlagMeta<'static>]] =
                &[#(#flag_meta_groups),*];
            const ARG_META_GROUPS: &[&[::usage_argv::spec::ArgMeta<'static>]] =
                &[#(#arg_meta_groups),*];
            static FLAG_METAS: [::usage_argv::spec::FlagMeta<'static>;
                ::usage_argv::table_len(FLAG_META_GROUPS)] =
                ::usage_argv::spec::concat_flag_metas(FLAG_META_GROUPS);
            static ARG_METAS: [::usage_argv::spec::ArgMeta<'static>;
                ::usage_argv::table_len(ARG_META_GROUPS)] =
                ::usage_argv::spec::concat_arg_metas(ARG_META_GROUPS);
        },
        flags: quote!(&FLAGS),
        args: quote!(&ARGS),
        flag_metas: quote!(&FLAG_METAS),
        arg_metas: quote!(&ARG_METAS),
    }
}

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

fn flag_arm(cli: &Cli, i: usize, field: &Field) -> TokenStream {
    let key = key_ident("FLAG", Some(i));
    let ident = &field.ident;
    let given = format_ident!("__given_{}", ident);
    let displaced = displacements(cli, field);
    let undisplaced = is_displaceable(cli, field).then(|| {
        let overridden = format_ident!("__overridden_{}", ident);
        quote!(partial.#overridden = false;)
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
        Shape::Many => quote!(partial.#ident.push(__usage_value_text(value));),
    };
    let table = format_ident!("FLAG_{i}");
    quote! {
        // The key gets us to the right arm in one jump; the identity check makes a
        // collision harmless rather than wrong. Two identical declarations in
        // different modules hash alike — a macro cannot see a module path — and
        // without this, one command's flag would fill another's field. `static` items
        // have distinct addresses, so this is exact.
        #key if ::core::ptr::eq(*flag, &#table) => {
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

/// Undoing the flags a flag displaces, as statements to run once it has been bound.
///
/// Both directions. `overrides` is declared on one flag and holds between the two:
/// clap resolves `--file a --stdin` and `--stdin --file a` the same way whichever side
/// declared it, and so does usage-lib.
fn displacements(cli: &Cli, field: &Field) -> Vec<TokenStream> {
    displaced_by(cli, field)
        .into_iter()
        .map(|other| {
            let reset = reset_to_default(other);
            let given = format_ident!("__given_{}", other.ident);
            let overridden = format_ident!("__overridden_{}", other.ident);
            quote! {
                #reset
                partial.#given = false;
                // Remembered, not just cleared: without this the environment fallback
                // would refill the flag that lost and mark it given again, and a
                // displaced `String` would be reported missing. usage-lib keeps the
                // same set for the same reason.
                partial.#overridden = true;
            }
        })
        .collect()
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
    !displaced_by(cli, field).is_empty()
}

fn arg_arm(i: usize, field: &Field) -> TokenStream {
    let key = key_ident("ARG", Some(i));
    let ident = &field.ident;
    let given = format_ident!("__given_{}", ident);
    let body = match field.shape {
        Shape::Many => quote!(partial.#ident.push(__usage_text(value));),
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
        // A flattened struct accumulates into its own partial, whose shape only its derive
        // knows — reached through the trait, like everything else about it.
        if let Kind::Flatten { ty } = &f.kind {
            let ident = &f.ident;
            let ty = in_module(ty);
            return Some(quote! {
                pub #ident: <#ty as ::usage_argv::spec::CommandArgs>::Partial,
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
        Some(quote!(pub #ident: #ty, pub #given: bool, #overridden))
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

/// The defaults a partial cannot express as `Default::default()`.
///
/// A declared `default` has to be in place before parsing starts, since nothing
/// later distinguishes "the default" from "what the user typed".
/// `inside_module` says where this will be spliced, which changes how a flattened struct's
/// type has to be named: the root's defaults are built in `parse_from`, in the impl beside the
/// user's struct, while a nested command's are built by `start()` inside the generated module —
/// where the same path needs a `super::`. Nothing else in here cares, and getting it wrong is a
/// compile error in the adopter's crate rather than here, so it is a parameter rather than a
/// guess.
fn partial_defaults(cli: &Cli, inside_module: bool) -> TokenStream {
    let sub_starts = subcommand_parts(cli)
        .map(|p| p.partial_starts)
        .unwrap_or_default();
    let plain = cli.fields.iter().filter_map(|f| {
        if matches!(f.kind, Kind::Subcommand { .. }) {
            return None;
        }
        // `start()` rather than `Default`, so the flattened struct's own defaults are in
        // place before parsing — the same reason this function exists at all.
        if let Kind::Flatten { ty } = &f.kind {
            let ident = &f.ident;
            let ty = if inside_module {
                in_module(ty)
            } else {
                quote!(#ty)
            };
            return Some(quote! {
                #ident: <#ty as ::usage_argv::spec::CommandArgs>::start(),
            });
        }
        let ident = &f.ident;
        let given = format_ident!("__given_{}", ident);
        let overridden = is_displaceable(cli, f).then(|| {
            let overridden = format_ident!("__overridden_{}", ident);
            quote!(#overridden: false,)
        });
        Some(quote! {
            #ident: ::std::default::Default::default(),
            #given: false,
            #overridden
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
    if let Kind::Flatten { ty } = &field.kind {
        // Built by its own derive, which is also what makes a nested flatten work: this is
        // the same call at every level.
        //
        // Named directly rather than through `in_module`: `build` is emitted in the impl
        // beside the user's struct, not inside the generated module, so a `super::` here
        // would climb one level too far.
        return quote! {
            #ident: <#ty as ::usage_argv::spec::CommandArgs>::build(partial.#ident)?
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
                match ::usage_argv::os_string_from_bytes(#value) {
                    ::std::result::Result::Ok(__usage_os) => #build(__usage_os),
                    ::std::result::Result::Err(__usage_bytes) => {
                        return ::std::result::Result::Err(
                            ::usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                                ::usage_argv::InvalidValue {
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
                    // Same as below: whether anything arrived is what tells "never given"
                    // from "given nothing", which the `Vec` itself cannot.
                    quote! {
                        #ident: if partial.#given {
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
                        ::usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                            ::usage_argv::InvalidValue {
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
                        ::usage_argv::Error::InvalidValue(::std::boxed::Box::new(
                            ::usage_argv::InvalidValue {
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
                // `Option<Vec<T>>` distinguishes "never given" from "given nothing", which
                // no `Vec` can — so the answer comes from whether anything arrived.
                quote! {
                    #ident: if partial.#given {
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
    let Some(default) = field.default.as_deref() else {
        return match field.shape {
            // A collection is cleared rather than replaced, so the field keeps whatever
            // capacity it already allocated.
            Shape::Many => quote!(partial.#ident.clear();),
            _ => quote!(partial.#ident = ::std::default::Default::default();),
        };
    };
    match field.shape {
        Shape::Bool => {
            let on = default == "true";
            quote!(partial.#ident = #on;)
        }
        Shape::Optional => quote! {
            partial.#ident = ::std::option::Option::Some(#default.as_bytes().to_vec());
        },
        Shape::Required => quote!(partial.#ident = #default.as_bytes().to_vec();),
        // Rejected in the model: a count starts at zero, and a default for a collecting
        // field is not applied yet.
        Shape::Count => quote!(partial.#ident = ::std::default::Default::default();),
        Shape::Many => quote!(partial.#ident.clear();),
    }
}

/// Take one event and say whether it belonged to this command.
fn apply_fn(cli: &Cli) -> TokenStream {
    let route = subcommand_parts(cli).map(|p| p.route).unwrap_or_default();
    // A flattened struct's flags are in this command's table, but its *keys* were minted in
    // its own expansion — so they cannot be matched here. Its `apply` recognises them, and
    // says whether it took the event.
    let flattened = cli.fields.iter().filter_map(|f| {
        let Kind::Flatten { ty } = &f.kind else {
            return None;
        };
        let ident = &f.ident;
        let ty = in_module(ty);
        Some(quote! {
            if <#ty as ::usage_argv::spec::CommandArgs>::apply(&mut partial.#ident, event) {
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
            event: &::usage_argv::Event<'_, '_>,
        ) -> bool {
            use ::usage_argv::Event;
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
    /// `default_subcommand:` for the `Command` table, when one is declared.
    default: TokenStream,
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
            ::std::option::Option::Some(__usage_at) => {
                <#ty as ::usage_argv::spec::Subcommands>::select(
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
                        ::usage_argv::Error::MissingSubcommand,
                    );
                }
            },
        }
    };

    Some(SubcommandParts {
        commands: quote!(subcommands: <#in_mod as ::usage_argv::spec::Subcommands>::COMMANDS,),
        // Resolved from the name at compile time. The variants are another expansion, so the
        // name is all there is to go on here — but `find_subcommand` searches the list during
        // const evaluation, which means a name no subcommand answers to fails to compile
        // rather than being emitted into a spec that nothing checks.
        default: match cli.default_subcommand.as_deref() {
            ::std::option::Option::Some(name) => quote! {
                default_subcommand: ::std::option::Option::Some(
                    ::usage_argv::find_subcommand(
                        <#in_mod as ::usage_argv::spec::Subcommands>::COMMANDS,
                        #name,
                    ),
                ),
            },
            ::std::option::Option::None => TokenStream::new(),
        },
        metas: quote!(subcommands: <#in_mod as ::usage_argv::spec::Subcommands>::METAS,),
        partial_fields: quote! {
            pub __usage_sub: <#in_mod as ::usage_argv::spec::Subcommands>::Partial,
            /// Which of this command's subcommands was reached, as a position in
            /// `COMMANDS`. Found from the table's own address, so it cannot be
            /// confused by a key collision.
            pub __usage_selected: ::std::option::Option<usize>,
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
                if let ::std::option::Option::Some(__usage_at) =
                    <#in_mod as ::usage_argv::spec::Subcommands>::COMMANDS
                        .iter()
                        .position(|candidate| ::core::ptr::eq(*candidate, *__usage_cmd))
                {
                    partial.__usage_selected = ::std::option::Option::Some(__usage_at);
                }
            }
            // Only the selected one is asked — see `Subcommands::apply`. The selection is
            // set just above, so a command word reaches the command it named on the same
            // event that selected it.
            if <#in_mod as ::usage_argv::spec::Subcommands>::apply(
                &mut partial.__usage_sub,
                partial.__usage_selected,
                event,
            ) {
                return true;
            }
        },
        check: quote! {
            if let ::std::option::Option::Some(__usage_at) = partial.__usage_selected {
                <#in_mod as ::usage_argv::spec::Subcommands>::check(
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
    let module = format_ident!("__usage_args_{}", ident.to_string().to_lowercase());
    let command_key = key_ident("COMMAND", None);
    let restart_token = option_str(cli.restart_token.as_deref());
    let mount = option_str(cli.mount.as_deref());
    let before_help = option_str(cli.before_help.as_deref());
    let before_long_help = option_str(cli.before_long_help.as_deref());
    let after_help = option_str(cli.after_help.as_deref());
    let after_long_help = option_str(cli.after_long_help.as_deref());

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
    let flag_metas = flags.iter().enumerate().map(|(i, f)| flag_meta(i, f));
    let arg_metas = args.iter().enumerate().map(|(i, f)| arg_meta(i, f));
    // Both the plain slices and, when a field is flattened, the joined arrays.
    let tables = tables(cli);
    let table_decls = &tables.decls;
    let meta_table_decls = &tables.meta_decls;
    let flag_table_ref = &tables.flags;
    let arg_table_ref = &tables.args;
    let flag_meta_table_ref = &tables.flag_metas;
    let arg_meta_table_ref = &tables.arg_metas;

    let name = &cli.name;
    let about = option_str(cli.about.as_deref());
    let long_about = option_str(cli.long_about.as_deref());
    let partial = partial_struct(cli);
    let defaults = partial_defaults(cli, true);
    let apply = apply_fn(cli);
    let post = post_binding(cli);
    let parts = subcommand_parts(cli);
    let sub_commands = parts
        .as_ref()
        .map(|p| p.commands.clone())
        .unwrap_or_default();
    let sub_metas = parts.as_ref().map(|p| p.metas.clone()).unwrap_or_default();
    let sub_build = parts.as_ref().map(|p| p.build.clone()).unwrap_or_default();
    // The same conversion the root gets. Two emitters producing one `build` is what let
    // this diverge: a typed field on a subcommand compiled here and not there, which is
    // every command mise has.
    let field_finals = cli
        .fields
        .iter()
        .filter(|f| !matches!(f.kind, Kind::Subcommand { .. }))
        .map(field_final);

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

            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #table_decls

            pub static COMMAND: Command = Command {
                name: #name,
                key: #command_key,
                flags: #flag_table_ref,
                args: #arg_table_ref,
                #sub_commands
                ..Command::EMPTY
            };

            #(#flag_metas)*
            #(#arg_metas)*
            #meta_table_decls

            pub static COMMAND_META: CommandMeta = CommandMeta {
                cmd: &COMMAND,
                about: #about,
                long_about: #long_about,
                restart_token: #restart_token,
                mount: #mount,
                before_help: #before_help,
                before_long_help: #before_long_help,
                after_help: #after_help,
                after_long_help: #after_long_help,
                flags: #flag_meta_table_ref,
                args: #arg_meta_table_ref,
                #sub_metas
                ..CommandMeta::EMPTY
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
                // Read unconditionally: a command that declares nothing to check would
                // otherwise leave the parameter unused in the user's crate, where
                // nobody can silence it.
                let _ = &partial;
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
            ::std::option::Option::Some(#i) => {
                <#ty as ::usage_argv::spec::CommandArgs>::apply(&mut partial.#field, event)
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
        // Both kinds of alias go in the table, because the parser matches both; which of
        // them help and completions mention is the metadata's business, below.
        let aliases = v.aliases.iter().chain(&v.hidden_aliases);
        quote! {
            pub static #name: ::usage_argv::Command = ::usage_argv::Command {
                name: #cmd_name,
                aliases: &[#(#aliases),*],
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
        // Each falls back on its own. A variant that gives a short description was suppressing
        // the struct's long one, which is exactly how a generated CLI is shaped: the enum says
        // what the command is for in a line, and the struct's own comment carries the rest. The
        // long form went missing from help for every command written that way.
        let about = match v.help.as_deref() {
            Some(help) => option_str(Some(help)),
            None => quote!(<#ty as ::usage_argv::spec::CommandArgs>::META.about),
        };
        let long_about = match v.long_help.as_deref() {
            Some(long) => option_str(Some(long)),
            None => quote!(<#ty as ::usage_argv::spec::CommandArgs>::META.long_about),
        };
        // Which of the table's aliases are hidden. The visible ones are not listed
        // anywhere: `cmd.aliases` minus these is what help and completions show.
        let hidden = &v.hidden_aliases;
        // A hidden command still answers to its name; it is simply not offered. Declared on
        // the variant, which is where the command itself is declared.
        let hide = v.hide;
        quote! {
            pub static #name: ::usage_argv::spec::CommandMeta =
                ::usage_argv::spec::CommandMeta {
                    cmd: &#cmd,
                    about: #about,
                    long_about: #long_about,
                    hide: #hide,
                    hidden_aliases: &[#(#hidden),*],
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
            #i => <#ty as ::usage_argv::spec::CommandArgs>::check(&mut partial.#field),
        }
    });
    let selects = subs.variants.iter().enumerate().map(|(i, v)| {
        let field = format_ident!("v{i}");
        let variant = &v.ident;
        let ty = &v.ty;
        // The one place the box matters: everything else — tables, partial, `build` —
        // speaks to the struct itself.
        let built = quote!(<#ty as ::usage_argv::spec::CommandArgs>::build(partial.#field)?);
        let built = if v.boxed {
            quote!(::std::boxed::Box::new(#built))
        } else {
            built
        };
        quote! {
            #i => ::std::result::Result::Ok(::std::option::Option::Some(
                #ident::#variant(#built),
            )),
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
                selected: ::std::option::Option<usize>,
                event: &::usage_argv::Event<'_, '_>,
            ) -> bool {
                match selected {
                    #(#applies)*
                    // Nothing selected yet, or a position that cannot be produced: the event
                    // is not one of these commands'.
                    _ => false,
                }
            }

            fn check<'t, 'v>(
                partial: &mut Self::Partial,
                selected: usize,
            ) -> ::std::result::Result<(), ::usage_argv::Error<'t, 'v>> {
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
                ::usage_argv::Error<'t, 'v>,
            > {
                match selected {
                    #(#selects)*
                    _ => ::std::result::Result::Ok(::std::option::Option::None),
                }
            }
        }
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

/// Everything decided once the last token has been read.
///
/// Ordered deliberately. The environment fills what argv left out, so it runs
/// before required-ness — a flag with `env` set is not missing. Choices and bounds
/// come last, because they judge a value however it arrived, including one that came
/// from the environment or a default.
fn post_binding(cli: &Cli) -> TokenStream {
    let sub_check = subcommand_parts(cli).map(|p| p.check).unwrap_or_default();
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
        let ty = in_module(ty);
        Some(quote! {
            <#ty as ::usage_argv::spec::CommandArgs>::check(&mut partial.#ident)?;
        })
    });
    // Applied here rather than in `start`, and this is not a detail: `start` builds the
    // partial for *every* command in the CLI, selected or not, so a declared default was
    // costing a `String` per default per command — 60 allocations to parse a bare `mise`,
    // which is the CLI's size leaking into the invocation. `check` runs for the selected
    // command only.
    //
    // Guarded on `__given_*`, which is what makes this safe to move: a negation that set a
    // defaulted `bool` to false during the parse must not be undone here.
    let declared_defaults = cli.fields.iter().filter_map(|f| {
        if f.default.is_none() || matches!(f.kind, Kind::Subcommand { .. }) {
            return None;
        }
        let given = format_ident!("__given_{}", f.ident);
        let assign = reset_to_default(f);
        Some(quote! {
            if !partial.#given {
                #assign
            }
        })
    });

    let env_fallbacks = cli.fields.iter().filter_map(|f| {
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
            Shape::Many => quote!(partial.#ident.push(value.into_bytes());),
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

    let required_checks = cli.fields.iter().filter_map(|f| {
        // A `String` has nowhere to put "absent", so the type is the declaration; a collection
        // has nothing in its type to say it and declares `required` instead.
        //
        // The same expression the metadata is built from, deliberately: checking only the shape
        // meant a `Vec` marked `required` was reported as one-or-more by the spec, the help, the
        // manpage and the completions, and accepted zero values from the CLI that actually ran.
        // One expression cannot disagree with itself.
        if !(f.shape == Shape::Required || f.required_collection) || f.default.is_some() {
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
                    ::usage_argv::Error::MissingRequired { name: #name },
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
                let ty = in_module(ty);
                quote!(<#ty as ::usage_argv::spec::ValueEnum>::CHOICES)
            }
            _ => {
                let list = &f.choices;
                quote!(&[#(#list),*])
            }
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
                if !#choices.contains(&__usage_text) {
                    return ::std::result::Result::Err(
                        ::usage_argv::Error::InvalidChoice {
                            name: #name,
                            choices: #choices,
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
        let max = match f.var_max.filter(|_| counts_occurrences) {
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

    // A conflict asks whether two flags both ended up with a value, which is why it
    // reads `__given_*` rather than the fields themselves: a `bool` flag that was given
    // as `false` is still a flag the user asked for. Env fallback has already run, so a
    // value from the environment counts the same as a typed one — clap does that too,
    // and an asymmetric rule would make the same pair a conflict or not depending on
    // which side happened to be typed.
    let conflict_checks = cli.fields.iter().flat_map(move |f| {
        let given = format_ident!("__given_{}", f.ident);
        let name = &f.name;
        f.conflicts.iter().filter_map(move |selector| {
            // Resolved in the model, which rejects a selector naming nothing.
            let other = cli.field_for_selector(selector)?;
            let other_given = format_ident!("__given_{}", other.ident);
            let other_name = &other.name;
            Some(quote! {
                if partial.#given && partial.#other_given {
                    return ::std::result::Result::Err(
                        ::usage_argv::Error::ConflictingFlags {
                            name: #name,
                            other: #other_name,
                        },
                    );
                }
            })
        })
    });

    // `required_if` and `required_unless` are the same question asked two ways: which
    // other flags decide whether this one had to be given. Neither needs to know the
    // order they arrived in — only whether they arrived — so both are answered here,
    // beside plain required-ness, from the same `__given_*` flags.
    let relationship_required_checks = cli.fields.iter().filter_map(move |f| {
        if f.required_if.is_empty() && f.required_unless.is_empty() {
            return None;
        }
        // A field with a default is already filled, so no condition can make it
        // missing. Plain required-ness skips these too, and so does usage-lib.
        if f.default.is_some() {
            return None;
        }
        let given = format_ident!("__given_{}", f.ident);
        let name = &f.name;
        let selector_given = |selector: &String| {
            let other = cli.field_for_selector(selector)?;
            let other_given = format_ident!("__given_{}", other.ident);
            Some(quote!(partial.#other_given))
        };
        let if_given: Vec<_> = f.required_if.iter().filter_map(selector_given).collect();
        let unless_given: Vec<_> = f
            .required_unless
            .iter()
            .filter_map(selector_given)
            .collect();
        // Absent, with nothing standing in for it: a default or an environment
        // variable has already filled the field and set `__given_*`.
        let missing = quote! {
            return ::std::result::Result::Err(
                ::usage_argv::Error::MissingRequired { name: #name },
            );
        };
        let required_if = (!if_given.is_empty()).then(|| {
            quote! {
                if #(#if_given)||* {
                    #missing
                }
            }
        });
        let required_unless = (!unless_given.is_empty()).then(|| {
            quote! {
                if !(#(#unless_given)||*) {
                    #missing
                }
            }
        });
        Some(quote! {
            if !partial.#given {
                #required_if
                #required_unless
            }
        })
    });

    quote! {
        // Before the environment, which overrides a default when the flag was not given —
        // the order `start` used to give them.
        #(#declared_defaults)*
        #(#env_fallbacks)*
        // Before required-ness: "you gave two flags that cannot go together" is the
        // more useful of the two answers when a conflict has also left something
        // unfilled, and it is the one usage-lib reports.
        #(#conflict_checks)*
        #(#flattened_checks)*
        #(#required_checks)*
        #(#relationship_required_checks)*
        #(#choice_checks)*
        #(#bound_checks)*
        #sub_check
    }
}

/// The word list and the conversion for a value enum.
///
/// Two impls and nothing else: the words as a `const` the spec can read, and the `FromStr`
/// that every typed field already goes through. Deliberately not a bespoke path — a value
/// enum is a type whose values happen to be listed, so it converts the way any other type
/// does, and the check that rejects a wrong word is the same `choices` check as a
/// hand-written list.
pub fn emit_value_enum(value_enum: &ValueEnum) -> TokenStream {
    let ident = &value_enum.ident;
    let words: Vec<&String> = value_enum.variants.iter().map(|(_, name)| name).collect();
    let arms = value_enum
        .variants
        .iter()
        .map(|(variant, name)| quote!(#name => ::std::result::Result::Ok(#ident::#variant),));
    // Listed in the message because a wrong word is the common mistake, and the words are
    // right here. The `choices` check usually reports this first, with the same list; this
    // is what a caller sees who converts one by hand.
    let expected = words
        .iter()
        .map(|w| w.as_str())
        .collect::<::std::vec::Vec<_>>()
        .join(", ");

    quote! {
        impl ::usage_argv::spec::ValueEnum for #ident {
            const CHOICES: &'static [&'static str] = &[#(#words),*];
        }

        impl ::std::str::FromStr for #ident {
            type Err = ::std::string::String;

            fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
                match value {
                    #(#arms)*
                    other => ::std::result::Result::Err(::std::format!(
                        "`{other}` is not one of: {}",
                        #expected
                    )),
                }
            }
        }
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
