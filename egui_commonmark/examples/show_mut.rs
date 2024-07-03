//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --example show_mut -- dark`

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
    text_buffer: String,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.text_buffer)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
                CommonMarkViewer::new("viewer")
                    .max_image_width(Some(512))
                    .show_mut(ui, &mut self.cache, &mut self.text_buffer);
            });
        });
    }
}

#[cfg(feature = "comrak")]
const BACKEND: &str = "comrak";
#[cfg(feature = "pulldown_cmark")]
const BACKEND: &str = "pulldown_cmark";

fn main() -> eframe::Result {
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

            cc.egui_ctx.style_mut(|style| {
                // Show the url of a hyperlink on hover
                style.url_in_tooltip = true;
            });

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
                text_buffer: EXAMPLE_TEXT.into(),
            }))
        }),
    )
}

const EXAMPLE_TEXT: &str = "
# Todo list
- [x] Exist
- [ ] Visit [`egui_commonmark` repo](https://github.com/lampsitter/egui_commonmark)
- [ ] Notice how the top markdown text changes in response to clicking the checkmarks.
    - [ ] Make up your own list items, by using the editor on the top.
";
