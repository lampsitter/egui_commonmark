# A commonmark viewer for [egui](https://github.com/emilk/egui)

[![Crate](https://img.shields.io/crates/v/egui_commonmark.svg)](https://crates.io/crates/egui_commonmark)
[![Documentation](https://docs.rs/egui_commonmark/badge.svg)](https://docs.rs/egui_commonmark)

<img src="https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v3.png" alt="showcase" width=280/>

While this crate's main focus is commonmark, it also supports a subset of
Github's markdown syntax: tables, strikethrough, tasklists and footnotes.

## Usage

In `Cargo.toml` add the following:

```toml
egui_commonmark = "0.8"

# If you don't need image loading you can ignore the dependencies below.

# Specify the the ways you want to load images, check egui_extras features
# for more info. Or use the feature "all-loaders" if you don't care.
egui_extras = { version = "<to-be-released>", features = ["image", "files"] }

# Opt into the image formats you want to load
image = { version = "0.24", features = ["png"] }
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

* `syntax_highlighting`: Syntax highlighting inside code blocks with
  [`syntect`](https://crates.io/crates/syntect)

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
