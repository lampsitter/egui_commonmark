# A commonmark viewer for [egui](https://github.com/emilk/egui)

[![Crate](https://img.shields.io/crates/v/egui_commonmark_macros.svg)](https://crates.io/crates/egui_commonmark_macros)
[![Documentation](https://docs.rs/egui_commonmark/badge.svg)](https://docs.rs/egui_commonmark)

<img src="https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v3.png" alt="showcase" width=280/>

This crate is `egui_commonmark`'s compile time variant. It is recommended to use
this crate through `egui_commonmark` by enabling the `macro` feature.


## Usage

In Cargo.toml:

```toml
egui_commonmark = "0.15"
# Specify what image formats you want to use
image = { version = "0.24", default-features = false, features = ["png"] }
```

### Example

```rust
use egui_commonmark::{CommonMarkCache, commonmark};
# egui::__run_test_ui(|ui| {
let mut cache = CommonMarkCache::default();
let _response = commonmark!("example", ui, &mut cache, "# ATX Heading Level 1");
# });
```

Alternatively you can embed a file

### Example

```rust
use egui_commonmark::{CommonMarkCache, commonmark_str};
# egui::__run_test_ui(|ui| {
let mut cache = CommonMarkCache::default();
commonmark_str!("example_file", ui, &mut cache, "content.md");
# });
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
