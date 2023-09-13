//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --example hello_world --all-features`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --example hello_world --all-features dark`

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let text = r#"# Commonmark Viewer Example

A *bunch* ~~of~~ __different__ `text` styles.

| __A table!__ |
| -------- |
| ![Rust logo](file://examples/rust-logo-128x128.png) |
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
    let mut args = std::env::args();
    args.next();
    let use_dark_theme = if let Some(theme) = args.next() {
        if theme == "light" {
            false
        } else {
            theme == "dark"
        }
    } else {
        false
    };

    eframe::run_native(
        "Markdown viewer",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(if use_dark_theme {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            Box::new(App {
                cache: CommonMarkCache::default(),
            })
        }),
    )
    .unwrap();
}
