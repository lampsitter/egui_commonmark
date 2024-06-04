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
            CommonMarkViewer::new("viewer")
                .max_image_width(Some(512))
                .show_scrollable(ui, &mut self.cache, &text);
        });
    }
}

#[cfg(feature = "comrak")]
const BACKEND: &str = "comrak";
#[cfg(feature = "pulldown_cmark")]
const BACKEND: &str = "pulldown_cmark";

fn main() {
    let mut args = std::env::args();
    args.next();

    eframe::run_native(
        &format!("Markdown viewer (backend '{}')", BACKEND),
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            if let Some(theme) = args.next() {
                if theme == "light" {
                    cc.egui_ctx.set_visuals(egui::Visuals::light());
                } else if theme == "dark" {
                    cc.egui_ctx.set_visuals(egui::Visuals::dark());
                }
            }

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
            }))
        }),
    )
    .unwrap();
}
