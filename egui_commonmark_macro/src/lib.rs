mod generator;
use generator::*;

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, Expr, LitStr, Token};

struct Parameters {
    ui: Expr,
    cache: Expr,
    // options: Expr,
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
            // options,
            markdown,
        })
    }
}

fn commonmark_impl(ui: Expr, cache: Expr, text: String) -> TokenStream {
    let stream = CommonMarkViewerInternal::new("id aaaaa".into()).show(ui, cache, &text);
    println!("fn main() {{");
    println!("{}", stream.to_string());
    println!("}}");

    stream
}

#[proc_macro]
pub fn commonmark(input: TokenStream) -> TokenStream {
    let Parameters {
        ui,
        cache,
        // options,
        markdown,
    } = parse_macro_input!(input as Parameters);

    commonmark_impl(ui, cache, markdown.value())
}

#[proc_macro]
pub fn commonmark_str(input: TokenStream) -> TokenStream {
    let Parameters {
        ui,
        cache,
        // options,
        markdown,
    } = parse_macro_input!(input as Parameters);

    let path = markdown.value();

    let Ok(md) = std::fs::read_to_string(path) else {
        return quote_spanned!(markdown.span()=>
            compile_error!("Could not find markdown file");
        )
        .into();
    };

    commonmark_impl(ui, cache, md)
}
