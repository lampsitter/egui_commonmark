A commonmark viewer for [egui](https://github.com/emilk/egui)

[![CI](https://github.com/lampsitter/egui_commonmark/actions/workflows/rust.yml/badge.svg)](https://github.com/lampsitter/egui_commonmark/actions/workflows/rust.yml)

# Usage
```rust
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";
let options = CommonMarkOptions::default();
// Stores image handles between each frame
let mut cache = CommonMarkCache::default();
CommonMarkViewer::show(ui, &mut cache, &options, markdown);
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
