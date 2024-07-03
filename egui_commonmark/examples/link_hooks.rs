//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is system theme. `cargo r --example link_hooks -- dark`

use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
    curr_page: usize,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let p1 = r#"# Page 1
Check out the [next](#next) page."#;
        let p2 = r#"# Page 2
Check out the [previous](#prev) page.

Notice how the destination is not shown on [hover](#prev) unlike with [urls](https://www.example.org)
"#;

        let p = [p1, p2];
        if self.cache.get_link_hook("#next").unwrap() {
            self.curr_page = 1;
        } else if self.cache.get_link_hook("#prev").unwrap() {
            self.curr_page = 0;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new("viewer").show(ui, &mut self.cache, p[self.curr_page]);
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
        &format!("Markdown viewer link hooks (backend '{}')", BACKEND),
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
                // Show the url of a hyperlink on hover. The demonstration of
                // the link hooks would be a little pointless without this
                style.url_in_tooltip = true;
            });

            let mut cache = CommonMarkCache::default();
            cache.add_link_hook("#next");
            cache.add_link_hook("#prev");

            Ok(Box::new(App {
                cache,
                curr_page: 0,
            }))
        }),
    )
}
