# A commonmark viewer for [egui](https://github.com/emilk/egui)

[![CI](https://github.com/lampsitter/egui_commonmark/actions/workflows/rust.yml/badge.svg)](https://github.com/lampsitter/egui_commonmark/actions/workflows/rust.yml)

<img src="https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example.png" alt="showcase" width=300/>

## Usage

```rust
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";
// Stores image handles between each frame
let mut cache = CommonMarkCache::default();
CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
```

## Features

* `syntax_highlighting`: Syntax highlighting inside code blocks with
  [`syntect`](https://crates.io/crates/syntect)
* `svg`: Support for viewing svg images
* `fetch`: Images with urls will be downloaded and displayed

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
