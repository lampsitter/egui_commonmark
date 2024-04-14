//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --features better_syntax_highlighting,svg,fetch`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --features better_syntax_highlighting,svg,fetch -- dark`
//!
//! Shows a simple way to use the crate to implement a book like view.

use eframe::egui;
use egui::ImageSource;
use egui_commonmark::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

struct Page {
    name: String,
    content: String,
}

struct App {
    cache: CommonMarkCache,
    curr_tab: Option<usize>,
    pages: Vec<Page>,
    math_to_svg: Rc<RefCell<HashMap<String, Arc<[u8]>>>>,
}

fn render_math(math: &str, inline: bool) -> Arc<[u8]> {
    println!("rendering math: {math}");
    if inline {
        mathjax_svg::convert_to_svg_inline(math).unwrap()
    } else {
        mathjax_svg::convert_to_svg(math).unwrap()
    }
    .into_bytes()
    .into()
}

impl App {
    fn sidepanel(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::left("left_documentation_panel")
            .resizable(false)
            .default_width(100.0)
            .show_inside(ui, |ui| {
                let style = ui.style_mut();
                style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
                style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                style.visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    for (i, p) in self.pages.iter().enumerate() {
                        if Some(i) == self.curr_tab {
                            let _ = ui.selectable_label(true, &p.name);
                        } else if ui.selectable_label(false, &p.name).clicked() {
                            self.curr_tab = Some(i);
                        }
                        ui.separator();
                    }
                });
            });
    }

    fn content_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Add a frame with margin to prevent the content from hugging the sidepanel
            let math_to_svg = self.math_to_svg.clone();
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(5.0, 0.0))
                .show(ui, |ui| {
                    CommonMarkViewer::new("viewer")
                        .default_width(Some(200))
                        .max_image_width(Some(512))
                        .render_math_fn(Some(&move |ui, math, inline| {
                            let mut map = math_to_svg.borrow_mut();
                            let svg = map
                                .entry(math.to_string())
                                .or_insert_with(|| render_math(math, inline));

                            let uri = format!("{}.svg", egui::Id::from(math.to_string()).value());
                            ui.add(
                                egui::Image::new(ImageSource::Bytes {
                                    uri: uri.into(),
                                    bytes: egui::load::Bytes::Shared(svg.clone()),
                                })
                                .fit_to_original_size(1.0),
                            );
                        }))
                        .show(
                            ui,
                            &mut self.cache,
                            &self.pages[self.curr_tab.unwrap_or(0)].content,
                        );
                });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.sidepanel(ui);
            self.content_panel(ui);
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

    let _ = eframe::run_native(
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
                curr_tab: Some(0),
                pages: vec![
                    Page {
                        name: "Hello World".to_owned(),
                        content: include_str!("markdown/hello_world.md").to_owned(),
                    },
                    Page {
                        name: "Headers".to_owned(),
                        content: include_str!("markdown/headers.md").to_owned(),
                    },
                    Page {
                        name: "Lists".to_owned(),
                        content: include_str!("markdown/lists.md").to_owned(),
                    },
                    Page {
                        name: "Code blocks".to_owned(),
                        content: include_str!("markdown/code-blocks.md").to_owned(),
                    },
                    Page {
                        name: "Block Quotes".to_owned(),
                        content: include_str!("markdown/blockquotes.md").to_owned(),
                    },
                    Page {
                        name: "Tables".to_owned(),
                        content: include_str!("markdown/tables.md").to_owned(),
                    },
                    Page {
                        name: "Math".to_owned(),
                        content: include_str!("markdown/math.md").to_owned(),
                    },
                ],
                math_to_svg: Rc::new(RefCell::new(HashMap::new())),
            })
        }),
    );
}
