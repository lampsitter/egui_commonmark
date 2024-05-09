//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --features better_syntax_highlighting,svg,fetch`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --features better_syntax_highlighting,svg,fetch -- dark`

use eframe::egui;
use egui_commonmark::*;
use egui_commonmark_macro::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui_not_called_ui| {
                // Embed text directly
                commonmark!("n1", ui_not_called_ui, &mut self.cache, "Hello, world");

                // or from a file like include_str! NOTE: This does not cause a recompile when the
                // file has changed!

                // TODO: This will probably break if this is not called ui
                commonmark_str!(
                    "n2",
                    ui_not_called_ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/hello_world.md"
                );
                commonmark!("n4", ui_not_called_ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n3",
                    ui_not_called_ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/headers.md"
                );
                commonmark!("n5", ui_not_called_ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n6",
                    ui_not_called_ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/lists.md"
                );

                commonmark!("n6", ui_not_called_ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n7",
                    ui_not_called_ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/code-blocks.md"
                );

                commonmark!("n4", ui_not_called_ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n9",
                    ui_not_called_ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/blockquotes.md"
                );

                commonmark!("n10", ui_not_called_ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n11",
                    ui_not_called_ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/tables.md"
                );
            });
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
        &format!("Markdown viewer (backend '{}')", BACKEND),
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
