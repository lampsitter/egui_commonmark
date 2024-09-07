//! Compile time evaluation of markdown that generates egui widgets
//!
//! It is recommended to use this crate through the parent crate
//! [egui_commonmark](https://docs.rs/crate/egui_commonmark/latest).
//! If you for some reason don't want to use it you must also import
//! [egui_commonmark_backend](https://docs.rs/crate/egui_commonmark_backend/latest)
//! directly from your crate to get access to `CommonMarkCache` and internals that
//! the macros require for the final generated code.
//!
//! ## API
//! ### Embedding markdown text directly
//!
//! The macro has the following format:
//!
//! commonmark!(ui, cache, text);
//!
//! #### Example
//!
//! ```
//! # // If used through egui_commonmark the backend crate does not need to be relied upon
//! # use egui_commonmark_backend::CommonMarkCache;
//! # use egui_commonmark_macros::commonmark;
//! # egui::__run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! let _response = commonmark!(ui, &mut cache, "# ATX Heading Level 1");
//! # });
//! ```
//!
//! As you can see it also returns a response like most other egui widgets.
//!
//! ### Embedding markdown file
//!
//! The macro has the exact same format as the `commonmark!` macro:
//!
//! commonmark_str!(ui, cache, file_path);
//!
//! #### Example
//!
// Unfortunately can't depend on an actual file in the doc test so it must be
// disabled
//! ```rust,ignore
//! # use egui_commonmark_backend::CommonMarkCache;
//! # use egui_commonmark_macros::commonmark_str;
//! # egui::__run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! commonmark_str!(ui, &mut cache, "foo.md");
//! # });
//! ```
//!
//! One drawback is that the file cannot be tracked by rust on stable so the
//! program won't recompile if you only change the content of the file. To
//! work around this you can use a nightly compiler and enable the
//! `nightly` feature when iterating on your markdown files.
//!
//! ## Limitations
//!
//! Compared to it's runtime counterpart egui_commonmark it currently does not
//! offer customization. This is something that will be addressed eventually once
//! a good API has been chosen.
//!
//! ## What this crate is not
//!
//! This crate does not have as a goal to make widgets that can be interacted with
//! through code.
//!
//! ```rust,ignore
//! let ... = commonmark!(ui, &mut cache, "- [ ] Task List");
//! task_list.set_checked(true); // No !!
//! ```
//!
//! For that you should fall back to normal egui widgets
#![cfg_attr(feature = "document-features", doc = "# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![cfg_attr(feature = "nightly", feature(track_path))]

mod generator;
use generator::*;

use quote::quote_spanned;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, Expr, LitStr, Token};

struct Parameters {
    ui: Expr,
    cache: Expr,
    markdown: LitStr,
}

impl Parse for Parameters {
    fn parse(input: ParseStream) -> Result<Self> {
        let ui: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let cache: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let markdown: LitStr = input.parse()?;

        Ok(Parameters {
            ui,
            cache,
            markdown,
        })
    }
}

fn commonmark_impl(ui: Expr, cache: Expr, text: String) -> proc_macro2::TokenStream {
    let stream = CommonMarkViewerInternal::new().show(ui, cache, &text);

    #[cfg(feature = "dump-macro")]
    {
        // Wrap within a function to allow rustfmt to format it
        println!("fn main() {{");
        println!("{}", stream.to_string());
        println!("}}");
    }

    // false positive due to feature gate
    #[allow(clippy::let_and_return)]
    stream
}

#[proc_macro]
pub fn commonmark(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Parameters {
        ui,
        cache,
        markdown,
    } = parse_macro_input!(input as Parameters);

    commonmark_impl(ui, cache, markdown.value()).into()
}

#[proc_macro]
pub fn commonmark_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Parameters {
        ui,
        cache,
        markdown,
    } = parse_macro_input!(input as Parameters);

    let path = markdown.value();
    #[cfg(feature = "nightly")]
    {
        // Tell rust to track the file so that the macro will regenerate when the
        // file changes
        proc_macro::tracked_path::path(&path);
    }

    let Ok(md) = std::fs::read_to_string(path) else {
        return quote_spanned!(markdown.span()=>
            compile_error!("Could not find markdown file");
        )
        .into();
    };

    commonmark_impl(ui, cache, md).into()
}

fn resolve_backend_crate_import() -> proc_macro2::TokenStream {
    // The purpose of this is to ensure that when used through egui_commonmark
    // the generated code can always find egui_commonmark_backend without the
    // user having to import themselves.
    //
    // There are other ways to do this that does not depend on an external crate
    // such as exposing a feature flag in this crate that egui_commonmark can set.
    // This works for users, however it is a pain to use in this workspace as
    // the macro tests won't work when run from the workspace directory. So instead
    // they must be run from this crate's workspace. I don't want to rely on that mess
    // so this is the solution. I have also tried some other solutions with no success
    // or they had drawbacks that I did not like.
    //
    // With all that said the resolution is the following:
    //
    // Try egui_commonmark_backend first. This ensures that the tests will run from
    // the main workspace despite egui_commonmark being present. However if only
    // egui_commonmark is present then a `use egui_commonmark::egui_commonmark_backend;`
    // will be inserted into the generated code.
    //
    // If none of that work's then the user is missing some crates

    let backend_crate = proc_macro_crate::crate_name("egui_commonmark_backend");
    let main_crate = proc_macro_crate::crate_name("egui_commonmark");

    if backend_crate.is_ok() {
        proc_macro2::TokenStream::new()
    } else if let Ok(found_crate) = main_crate {
        let crate_name = match found_crate {
            proc_macro_crate::FoundCrate::Itself => return proc_macro2::TokenStream::new(),
            proc_macro_crate::FoundCrate::Name(name) => name,
        };

        let crate_name_lit = proc_macro2::Ident::new(&crate_name, proc_macro2::Span::call_site());
        quote::quote!(
            use #crate_name_lit::egui_commonmark_backend;
        )
    } else {
        proc_macro2::TokenStream::new()
    }
}

#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/pass/*.rs");
    t.compile_fail("tests/fail/*.rs");
}
