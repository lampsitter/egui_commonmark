//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --features better_syntax_highlighting,svg,fetch`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is system theme. `cargo r --features better_syntax_highlighting,svg,fetch -- dark`
//!
//! An easy way to visualize rendered markdown interactively

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
    markdown: String,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::SidePanel::left("left_panel")
                .show_inside(ui, |ui| ui.text_edit_multiline(&mut self.markdown));
            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    CommonMarkViewer::new().show(ui, &mut self.cache, &self.markdown);
                });
            });
        });
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args();
    args.next();

    eframe::run_native(
        "Interactive markdown viewer",
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
                markdown: r#"# Heading

text with a \
break

text with a large

separator

```python
if __name__ == "__main__":
    pass
```"#
                    .to_owned(),
                cache: CommonMarkCache::default(),
            }))
        }),
    )
}
