//! Demonstrates linked badge/shield images (`[![alt](img)](url)`) rendering
//! side by side without overlapping.
//!
//! Make sure to run this example from the repo root:
//! `cargo r --example shields --features svg,fetch,egui_extras/svg_text`
//!
//! Add `light` or `dark` to set the theme explicitly:
//! `cargo r --example shields --features svg,fetch,egui_extras/svg_text -- dark`

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new().show(
                    ui,
                    &mut self.cache,
                    include_str!("markdown/shields.md"),
                );
            });
        });
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args();
    args.next();

    eframe::run_native(
        "Badges / Shields",
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
                style.url_in_tooltip = true;
            });

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
            }))
        }),
    )
}
