//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --example macro --features macros,better_syntax_highlighting`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is system theme. `cargo r --example macro --features macros,better_syntax_highlighting -- dark`

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
