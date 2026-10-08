//! Where the generated code finds usage's runtime.
//!
//! Every derive names its runtime, validation, and config crates through the `usage-rs`
//! facade, so the paths are fixed: `::usage_rs` unless the container says otherwise with
//! `#[usage(crate = path)]`, serde's `crate` escape hatch. Nothing here reads the adopter's
//! `Cargo.toml`; a derive that had to would put a manifest parser in every adopter's build.

use proc_macro2::TokenStream;
use quote::quote;
use std::cell::RefCell;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Expr, Meta, Token};

thread_local! {
    static FACADE: RefCell<Option<syn::Path>> = const { RefCell::new(None) };
}

/// The facade the generated code is written against.
pub fn facade() -> TokenStream {
    FACADE.with(|facade| match &*facade.borrow() {
        Some(path) => quote!(#path),
        None => quote!(::usage_rs),
    })
}

/// Run `expand` with `path` as the facade, restoring the previous one afterwards.
pub fn with_facade<R>(path: Option<syn::Path>, expand: impl FnOnce() -> R) -> R {
    let previous = FACADE.with(|facade| facade.replace(path));
    let result = expand();
    FACADE.with(|facade| *facade.borrow_mut() = previous);
    result
}

/// Remove `crate = path` from the container's `#[usage(...)]` attributes and return it.
///
/// Taking it out before the model is read keeps the model's option lists free of a key that
/// is not part of any command's description.
pub fn take_crate(input: &mut DeriveInput) -> syn::Result<Option<syn::Path>> {
    let mut found = None;
    let mut kept = Vec::with_capacity(input.attrs.len());
    for mut attr in std::mem::take(&mut input.attrs) {
        if !attr.path().is_ident("usage") {
            kept.push(attr);
            continue;
        }
        let Ok(list) = attr.meta.require_list() else {
            kept.push(attr);
            continue;
        };
        let metas = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
        let Ok(metas) = metas else {
            // Leave the malformed attribute for the model to report at its own span.
            kept.push(attr);
            continue;
        };
        let (crate_metas, rest): (Vec<_>, Vec<_>) = metas
            .into_iter()
            .partition(|meta| meta.path().is_ident("crate"));
        for meta in crate_metas {
            let Meta::NameValue(name_value) = meta else {
                return Err(syn::Error::new_spanned(
                    meta,
                    "`crate` takes a path, as in `crate = usage`",
                ));
            };
            let Expr::Path(path) = name_value.value else {
                return Err(syn::Error::new_spanned(
                    name_value.value,
                    "`crate` takes a path, as in `crate = usage`",
                ));
            };
            if found.replace(path.path).is_some() {
                return Err(syn::Error::new_spanned(
                    name_value.path,
                    "`crate` is given more than once",
                ));
            }
        }
        if rest.is_empty() {
            continue;
        }
        attr.meta = Meta::List(syn::MetaList {
            path: attr.path().clone(),
            delimiter: syn::MacroDelimiter::Paren(Default::default()),
            tokens: quote!(#(#rest),*),
        });
        kept.push(attr);
    }
    input.attrs = kept;
    Ok(found)
}

/// Parse the input's container attributes, then expand with its facade in effect.
pub fn expand(
    mut input: DeriveInput,
    emit: impl FnOnce(&DeriveInput) -> syn::Result<TokenStream>,
) -> proc_macro::TokenStream {
    let path = match take_crate(&mut input) {
        Ok(path) => path,
        Err(e) => return e.to_compile_error().into(),
    };
    with_facade(path, || match emit(&input) {
        Ok(tokens) => tokens,
        Err(e) => e.to_compile_error(),
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take(input: &str) -> (Option<String>, String) {
        let mut input: DeriveInput = syn::parse_str(input).unwrap();
        let path = take_crate(&mut input)
            .unwrap()
            .map(|p| quote!(#p).to_string());
        let attrs = input.attrs.iter().map(|a| quote!(#a).to_string()).collect();
        (path, attrs)
    }

    #[test]
    fn no_attribute_means_the_default_facade() {
        assert_eq!(take("struct A;"), (None, String::new()));
        assert_eq!(facade().to_string(), ":: usage_rs");
    }

    #[test]
    fn crate_is_removed_and_the_rest_kept() {
        assert_eq!(
            take("#[usage(crate = usage, bin = \"ex\")] struct A;"),
            (Some("usage".into()), "# [usage (bin = \"ex\")]".into())
        );
        assert_eq!(
            take("#[usage(crate = ::my::usage)] struct A;"),
            (Some(":: my :: usage".into()), String::new())
        );
    }

    #[test]
    fn crate_must_be_a_path_given_once() {
        for bad in [
            "#[usage(crate)] struct A;",
            "#[usage(crate = \"usage\")] struct A;",
            "#[usage(crate = a, crate = b)] struct A;",
        ] {
            let mut input: DeriveInput = syn::parse_str(bad).unwrap();
            assert!(take_crate(&mut input).is_err(), "{bad}");
        }
    }

    #[test]
    fn facade_override_is_scoped() {
        let inside = with_facade(Some(syn::parse_str("usage").unwrap()), || {
            facade().to_string()
        });
        assert_eq!(inside, "usage");
        assert_eq!(facade().to_string(), ":: usage_rs");
    }
}
