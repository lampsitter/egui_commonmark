//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --example macro --features macro,better_syntax_highlighting`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --example macro --features macro,better_syntax_highlighting -- dark`

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Embed text directly
                commonmark!("n1", ui, &mut self.cache, "Hello, world");

                // In cases like these it's better to use egui::Separator directly
                commonmark!("n1-1", ui, &mut self.cache, "------------");

                // From a file like include_str! NOTE: This does not cause a recompile when the
                // file has changed!
                commonmark_str!(
                    "n2",
                    ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/hello_world.md"
                );
                commonmark!("n4", ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n3",
                    ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/headers.md"
                );
                commonmark!("n5", ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n6",
                    ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/lists.md"
                );

                commonmark!("n6", ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n7",
                    ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/code-blocks.md"
                );

                commonmark!("n4", ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n9",
                    ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/blockquotes.md"
                );

                commonmark!("n10", ui, &mut self.cache, "------------");

                commonmark_str!(
                    "n11",
                    ui,
                    &mut self.cache,
                    "egui_commonmark/examples/markdown/tables.md"
                );
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
