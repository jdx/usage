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

use crate::model::{Cli, Field, Kind, Shape};

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

    let flag_tables = flags.iter().enumerate().map(|(i, f)| flag_table(i, f));
    let arg_tables = args.iter().enumerate().map(|(i, f)| arg_table(i, f));
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

    let field_inits = cli.fields.iter().map(field_init);
    let flag_arms = flags.iter().enumerate().map(|(i, f)| flag_arm(i, f));
    let arg_arms = args.iter().enumerate().map(|(i, f)| arg_arm(i, f));
    // `field: local` rather than the shorthand, because the locals are prefixed:
    // a field called `text` or `parser` would otherwise collide with something the
    // generated code needs.
    let field_finals = cli.fields.iter().map(|f| {
        let ident = &f.ident;
        let local = local_ident(f);
        quote!(#ident: #local)
    });

    quote! {
        #[doc(hidden)]
        #[allow(non_upper_case_globals, non_snake_case, unused_imports)]
        mod #module {
            use ::usage_argv::spec::{ArgMeta, CommandMeta, FlagMeta, Spec};
            use ::usage_argv::{Arg, Command, DoubleDash, Flag};

            #(#flag_tables)*
            #(#arg_tables)*

            pub static ROOT: Command = Command {
                name: #name,
                flags: &[#(#flag_refs),*],
                args: &[#(#arg_refs),*],
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
                ..CommandMeta::EMPTY
            };

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

                // Values arrive as the bytes that were on the command line. This
                // version holds them as `String`, and converts lossily, which is
                // what mise already does with its own argv. Rejecting a non-UTF-8
                // value needs an error type for value conversion, and that arrives
                // with typed fields.
                fn __usage_text(value: &[u8]) -> ::std::string::String {
                    ::std::string::String::from_utf8_lossy(value).into_owned()
                }
                fn __usage_value_text(
                    value: ::std::option::Option<&[u8]>,
                ) -> ::std::string::String {
                    value.map(__usage_text).unwrap_or_default()
                }

                #(#field_inits)*

                let mut __usage_parser = ::usage_argv::Parser::new(Self::command(), argv);
                while let ::std::option::Option::Some(__usage_event) =
                    __usage_parser.next_event()
                {
                    match __usage_event? {
                        Event::Flag { flag, value, negated } => {
                            // Both are `Copy`, and a CLI with no boolean flag would
                            // otherwise leave one of them unread.
                            let _ = (value, negated);
                            match flag.key {
                            #(#flag_arms)*
                            // Unreachable: every table entry above was emitted
                            // from a field, and the parser only reports entries
                            // it was given.
                            _ => {}
                            }
                        }
                        Event::Arg { arg, value } => {
                            let _ = value;
                            match arg.key {
                            #(#arg_arms)*
                            _ => {}
                            }
                        }
                        // This version has no subcommands to descend into.
                        Event::Command(_) => {}
                    }
                }

                ::std::result::Result::Ok(Self { #(#field_finals),* })
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
    let key = i as u32;
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

fn arg_table(i: usize, field: &Field) -> TokenStream {
    let name = format_ident!("ARG_{i}");
    // Keys are separate spaces for flags and arguments, since the parser reports
    // which of the two it bound.
    let key = i as u32;
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
            ..ArgMeta::EMPTY
        };
    }
}

/// The name of the local a field accumulates into.
///
/// Prefixed, so a field called `text`, `parser`, or `argv` cannot shadow anything
/// the generated code relies on.
fn local_ident(field: &Field) -> proc_macro2::Ident {
    format_ident!("__usage_field_{}", field.ident)
}

/// The local each field accumulates into while parsing.
fn field_init(field: &Field) -> TokenStream {
    let ident = local_ident(field);
    match field.shape {
        Shape::Bool => {
            // A negatable flag's default is whatever `default` says, since
            // `--no-x` has to be able to turn something off.
            let start = field.default.as_deref() == Some("true");
            quote!(let mut #ident = #start;)
        }
        Shape::Count => {
            // Typed, so `saturating_add` below resolves to the field's own integer.
            let ty = &field.ty;
            quote!(let mut #ident: #ty = 0;)
        }
        Shape::Optional => {
            let default = match field.default.as_deref() {
                Some(d) => quote!(::std::option::Option::Some(#d.to_string())),
                None => quote!(::std::option::Option::None),
            };
            quote!(let mut #ident = #default;)
        }
        Shape::Required => {
            let default = match field.default.as_deref() {
                Some(d) => quote!(#d.to_string()),
                None => quote!(::std::string::String::new()),
            };
            quote!(let mut #ident = #default;)
        }
        Shape::Many => quote!(let mut #ident = ::std::vec::Vec::new();),
    }
}

fn flag_arm(i: usize, field: &Field) -> TokenStream {
    let key = i as u32;
    let ident = local_ident(field);
    let body = match field.shape {
        // `negated` is what distinguishes `--color` from `--no-color`.
        Shape::Bool => quote!(#ident = !negated;),
        // Saturating, because a `u8` field given 256 occurrences would otherwise
        // panic in debug and wrap to zero in release.
        Shape::Count => quote!(#ident = #ident.saturating_add(1);),
        Shape::Optional => quote! {
            #ident = ::std::option::Option::Some(__usage_value_text(value));
        },
        Shape::Required => quote!(#ident = __usage_value_text(value);),
        Shape::Many => quote!(#ident.push(__usage_value_text(value));),
    };
    quote! {
        #key => { #body }
    }
}

fn arg_arm(i: usize, field: &Field) -> TokenStream {
    let key = i as u32;
    let ident = local_ident(field);
    let body = match field.shape {
        Shape::Many => quote!(#ident.push(__usage_text(value));),
        Shape::Optional => quote! {
            #ident = ::std::option::Option::Some(__usage_text(value));
        },
        _ => quote!(#ident = __usage_text(value);),
    };
    quote! {
        #key => { #body }
    }
}

fn option_str(value: Option<&str>) -> TokenStream {
    match value {
        Some(v) => quote!(::std::option::Option::Some(#v)),
        None => quote!(::std::option::Option::None),
    }
}
