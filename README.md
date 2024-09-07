# A commonmark viewer for [egui](https://github.com/emilk/egui)

[![Crate](https://img.shields.io/crates/v/egui_commonmark.svg)](https://crates.io/crates/egui_commonmark)
[![Documentation](https://docs.rs/egui_commonmark/badge.svg)](https://docs.rs/egui_commonmark)

<img src="https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v3.png" alt="showcase" width=280/>

While this crate's main focus is commonmark, it also supports a subset of
Github's markdown syntax: tables, strikethrough, tasklists and footnotes.

## Usage

In Cargo.toml:

```toml
egui_commonmark = "0.17"
# Specify what image formats you want to use
image = { version = "0.25", default-features = false, features = ["png"] }
```

```rust
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";

let mut cache = CommonMarkCache::default();
CommonMarkViewer::new().show(ui, &mut cache, markdown);
```


## Compile time evaluation of markdown

If you want to embed markdown directly the binary then you can enable the `macros` feature.
This will do the parsing of the markdown at compile time and output egui widgets.

### Example

```rust
use egui_commonmark::{CommonMarkCache, commonmark};
let mut cache = CommonMarkCache::default();
let _response = commonmark!(ui, &mut cache, "# ATX Heading Level 1");
```

Alternatively you can embed a file

### Example

```rust
use egui_commonmark::{CommonMarkCache, commonmark_str};
let mut cache = CommonMarkCache::default();
commonmark_str!(ui, &mut cache, "content.md");
```


## Features

* `macros`: macros for compile time parsing of markdown
* `better_syntax_highlighting`: Syntax highlighting inside code blocks with
  [`syntect`](https://crates.io/crates/syntect)
* `svg`: Support for viewing svg images
* `fetch`: Images with urls will be downloaded and displayed


## Examples

For an easy intro check out the `hello_world` example. To see all the different
features egui_commonmark has to offer check out the `book` example.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
