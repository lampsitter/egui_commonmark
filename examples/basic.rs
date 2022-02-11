//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --example basic --all-features`

use eframe::{egui, epi};
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
}

impl epi::App for App {
    fn name(&self) -> &str {
        "Markdown viewer"
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let text = r#"# Commonmark Viewer example

A *little* ~~paragraph~~ __with__ `multiple` styles.

| __A table!__ |
| -------- |
| ![Rust logo](examples/rust-logo-128x128.png) |
| Some filler text |
| [Link to repo](https://github.com/lampsitter/egui_commonmark) |

```rs
let mut vec = Vec::new();
vec.push(5);
```

> Some smart quote here

- [ ] A feature[^1]
- [X] A completed feature
    1. Sub item

[^1]: A footnote
            "#;

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new("viewer")
                    .max_image_width(Some(512))
                    .show(ui, &mut self.cache, text);
            });
        });
    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        Box::new(App {
            cache: CommonMarkCache::default(),
        }),
        options,
    );
}
