//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --features better_syntax_highlighting,svg,fetch`
//!
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is system theme. `cargo r --example scroll --all-features dark`

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
            CommonMarkViewer::new()
                .max_image_width(Some(512))
                .show_scrollable("viewer", ui, &mut self.cache, &text);
        });
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args();
    args.next();

    eframe::run_native(
        "Markdown viewer",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            if let Some(theme) = args.next() {
                if theme == "light" {
                    cc.egui_ctx.set_visuals(egui::Visuals::light());
                } else if theme == "dark" {
                    cc.egui_ctx.set_visuals(egui::Visuals::dark());
                }
            }

            cc.egui_ctx.style_mut(|style| {
                // Show the url of a hyperlink on hover
                style.url_in_tooltip = true;
            });

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
            }))
        }),
    )
}
