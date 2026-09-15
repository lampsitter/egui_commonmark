//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --features better_syntax_highlighting,svg,fetch`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is system theme. `cargo r --features better_syntax_highlighting,svg,fetch -- dark`
//!
//! Demonstrates that long table cells wrap so that the table fits the
//! document width, and that tables with too many columns to fit scroll
//! horizontally without affecting text wrapping in the surrounding document.

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let text = include_str!("markdown/wide_table.md");
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new()
                    .max_image_width(Some(512))
                    .show(ui, &mut self.cache, text);
            });
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
                    cc.egui_ctx.set_theme(egui::Theme::Light);
                } else if theme == "dark" {
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                }
            }

            cc.egui_ctx.global_style_mut(|style| {
                // Show the url of a hyperlink on hover
                style.url_in_tooltip = true;
            });

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
            }))
        }),
    )
}
