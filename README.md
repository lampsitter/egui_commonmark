# A commonmark viewer for [egui](https://github.com/emilk/egui)

[![Crate](https://img.shields.io/crates/v/egui_commonmark.svg)](https://crates.io/crates/egui_commonmark)
[![Documentation](https://docs.rs/egui_commonmark/badge.svg)](https://docs.rs/egui_commonmark)

<img src="https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v3.png" alt="showcase" width=280/>

While this crate's main focus is commonmark, it also supports a subset of
Github's markdown syntax: tables, strikethrough, tasklists and footnotes.

## Usage

In Cargo.toml:

```toml
egui_commonmark = "0.11"
# Specify what image formats you want to use
image = { version = "0.24", default-features = false, features = ["png"] }
```

```rust
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";

let mut cache = CommonMarkCache::default();
CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
```

## Features

* `better_syntax_highlighting`: Syntax highlighting inside code blocks with
  [`syntect`](https://crates.io/crates/syntect)
* `svg`: Support for viewing svg images
* `fetch`: Images with urls will be downloaded and displayed

## Parsing backends

egui_commonmark offers __experimental__ support for using [comrak](https://crates.io/crates/comrak)
as parsing backend. By default pulldown_cmark is used. To use comrak instead do:

```toml
egui_commonmark = { version = "0.11", default-features = false, features = ["load-images", "comrak"] }
```

### Why two parsing backends?

egui_commonmark was originally implemented using pulldown_cmark, however comrak
has stricter commonmark/github style markdown support. In addition it allows the
crate to support more features than pulldown_cmark can offer with regards to github
style markdown.

pulldown_cmark has fewer dependencies and should theoretically be faster.

Due to these reasons both backends are supported. At least for now.


### Differences in support

Currently both support the same feature set

### Known rendering differences

| Type    | pulldown_cmark | comrak |
|---------|----------------|--------|
| Footers | Placed when they appear | Placed at the end |
| Spec incompatibilies | Blocks such as images can be rendered inside tables. This is against the spec | Disallowed |



## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
