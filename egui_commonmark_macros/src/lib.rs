//! Compile time evalation of markdown that generates egui widgets
//!
//! It is recommended to use this crate through the parent crate
//! [egui_commonmark](https://docs.rs/crate/egui_commonmark/latest).
//! If you for some reason don't want to use it you must also import
//! [egui_commonmark_shared](https://docs.rs/crate/egui_commonmark_shared/latest)
//! directly from your crate to get access to `CommonMarkCache` and internals that
//! the macros require for the final generated code.
//!
//! ## API
//! ### Embedding markdown text directly
//!
//! The macro has the following format:
//!
//! commonmark!(id, ui, cache, text);
//!
//! #### Example
//!
//! ```
//! // If used through egui_commonmark the shared crate does not need to be relied upon
//! # use egui_commonmark_shared::CommonMarkCache;
//! # use egui_commonmark_macros::commonmark;
//! # egui::__run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! let _response = commonmark!("example", ui, &mut cache, "# ATX Heading Level 1");
//! # });
//! ```
//!
//! As you can see it also returns a response like most other egui widgets.
//!
//! ### Embedding markdown file
//!
//! The macro has the exact same format as the `commonmark!` macro:
//!
//! commonmark_str!(id, ui, cache, file_path);
//!
//! #### Example
//!
// Unfortunately can't depend on an actual file in the doc test so i must be
// disabled
//! ```rust,ignore
//! # use egui_commonmark_shared::CommonMarkCache;
//! # use egui_commonmark_macros::commonmark_str;
//! # egui::__run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! commonmark_str!("example", ui, &mut cache, "foo.md");
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
//! This crate does not have as goal to make widgets that can be interacted with
//! through code.
//!
//! ```rust,ignore
//! let ... = commonmark!("example", ui, &mut cache, "- [ ] Task List");
//! if task_list.is_checked() {
//!   // No!!
//! }
//! ```
//!
//! For that you should fall back to normal egui widgets
#![cfg_attr(feature = "nightly", feature(track_path))]

mod generator;
use generator::*;

use quote::quote_spanned;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, Expr, LitStr, Token};

struct Parameters {
    id: LitStr,
    ui: Expr,
    cache: Expr,
    // options: Expr,
    markdown: LitStr,
}

impl Parse for Parameters {
    fn parse(input: ParseStream) -> Result<Self> {
        let id: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let ui: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let cache: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let markdown: LitStr = input.parse()?;

        Ok(Parameters {
            id,
            ui,
            cache,
            // options,
            markdown,
        })
    }
}

fn commonmark_impl(id: String, ui: Expr, cache: Expr, text: String) -> proc_macro2::TokenStream {
    let stream = CommonMarkViewerInternal::new(id.into()).show(ui, cache, &text);

    #[cfg(feature = "dump-macro")]
    {
        // Wrap within a function to allow rustfmt to format it
        println!("fn main() {{");
        println!("{}", stream.to_string());
        println!("}}");
    }

    stream
}

#[proc_macro]
pub fn commonmark(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Parameters {
        id,
        ui,
        cache,
        // options,
        markdown,
    } = parse_macro_input!(input as Parameters);

    commonmark_impl(id.value(), ui, cache, markdown.value()).into()
}

#[proc_macro]
pub fn commonmark_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Parameters {
        id,
        ui,
        cache,
        // options,
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

    commonmark_impl(id.value(), ui, cache, md).into()
}

#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/pass/*.rs");
    t.compile_fail("tests/fail/*.rs");
}
