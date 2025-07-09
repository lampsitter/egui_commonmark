//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --example html -- dark`

use eframe::egui;
use egui_commonmark::*;
use std::cell::RefCell;
use std::rc::Rc;

struct App {
    cache: CommonMarkCache,
    /// To avoid id collisions
    counter: Rc<RefCell<usize>>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        *self.counter.as_ref().borrow_mut() = 0;

        let counter = Rc::clone(&self.counter);
        let func = move |ui: &mut egui::Ui, html: &str| {
            // For simplicity lets just hide the content regardless of what kind of
            // node it is.
            ui.collapsing(format!("Collapsed {}", counter.as_ref().borrow()), |ui| {
                ui.label(html);
            });

            *counter.as_ref().borrow_mut() += 1;
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new().render_html_fn(Some(&func)).show(
                    ui,
                    &mut self.cache,
                    EXAMPLE_TEXT,
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
                    cc.egui_ctx.set_theme(egui::Theme::Light);
                } else if theme == "dark" {
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                }
            }

            cc.egui_ctx.style_mut(|style| {
                // Show the url of a hyperlink on hover
                style.url_in_tooltip = true;
            });

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
                counter: Rc::new(RefCell::new(0)),
            }))
        }),
    )
}

const EXAMPLE_TEXT: &str = r#"
# Customized rendering using html
<p>
some text
</p>

<p>
some text 2
</p>
"#;
