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
let x = 3;
vec.push(5);
```

> Some smart quote here

# Plans
* Make a sandwich
* Bake a cake
* Conquer the world

![Rust logo](examples/rust-logo-128x128.png)

        "#;
        text += &repeating.repeat(80);

        egui::SidePanel::left("aa").show(ctx, |ui| {
            CommonMarkViewer::new("viewer")
                .max_image_width(Some(512))
                .show_scrollable(ui, &mut self.cache, &text);
        });

        use pulldown_cmark::Options;
        fn parser_options() -> pulldown_cmark::Options {
            Options::ENABLE_TABLES
                | Options::ENABLE_TASKLISTS
                | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_FOOTNOTES
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut events = pulldown_cmark::Parser::new_ext(&text, parser_options());
            egui::ScrollArea::vertical().show(ui, |ui| {
                while let Some(e) = events.next() {
                    ui.label(format!("{:#?}", e));
                }
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
