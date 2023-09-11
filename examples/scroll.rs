//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --example scroll --all-features`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --example scroll --all-features dark`

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut text = r#"# Commonmark Viewer Example
        This is a fairly large markdown file showcasing scroll.
                    "#
        .to_string();

        let repeating = r#"
This section will be repeated

```rs
let mut vec = Vec::new();
vec.push(5);
```
 
# Plans
* Make a sandwich
* Bake a cake
* Conquer the world
        "#;
        text += &repeating.repeat(1024);

        egui::CentralPanel::default().show(ctx, |ui| {
            CommonMarkViewer::new("viewer")
                .max_image_width(Some(512))
                .show_scrollable(ui, &mut self.cache, &text);
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
                cache: CommonMarkCache::new(&cc.egui_ctx),
            })
        }),
    )
    .unwrap();
}
