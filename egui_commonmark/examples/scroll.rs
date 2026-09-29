//! Make sure to run this example from the repo directory and not the example
//! directory. Run with:
//! `cargo r --example scroll --features better_syntax_highlighting,svg [light|dark]`
//! Run with `CACHE=false` to disable viewport caching for comparison.
//!
//! Keyboard shortcuts (when no text field has focus):
//! Up/Down arrows scroll one line; Page Up/Down scroll one page;
//! Home/End (or Cmd+Up/Down on macOS) jump to the top or bottom.
use std::env;

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// Salt used to derive a stable, context-scoped [`egui::Id`] for this viewer
/// via [`egui::Ui::make_persistent_id`]. Defined here so it is named in one
/// place rather than repeated as a magic string.
const VIEWER_ID_SALT: &str = "scroll_example";

struct App {
    cache: CommonMarkCache,
    viewport_cache: bool,
    content: String,
}

impl App {}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_min_height(512.0);

        // Derive the viewer Id from the Ui context each frame. This scopes it
        // to the widget hierarchy (egui's preferred pattern) and avoids
        // global hash collisions. Stable as long as the widget tree is stable.
        let id = ui.make_persistent_id(VIEWER_ID_SALT);

        egui::Panel::top("search_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui.text_edit_singleline(self.cache.search_query_mut(&id));
                if response.changed() {
                    self.cache.update_search_matches(&id, &self.content);
                }
                // Re-request focus so that repeated Enter presses keep working without having to
                // click back into the box each time.
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter_pressed {
                    response.request_focus();
                }

                let match_count = self.cache.search_ranges(&id).len();
                ui.label(match self.cache.active_match(&id) {
                    Some(i) if match_count > 0 => format!("{}/{match_count}", i + 1),
                    _ => format!("0/{match_count}"),
                });

                if ui.button("Previous").clicked()
                    || (enter_pressed && ui.input(|i| i.modifiers.shift))
                {
                    self.cache.go_to_match(&id, -1);
                }
                if ui.button("Next").clicked()
                    || (enter_pressed && !ui.input(|i| i.modifiers.shift))
                {
                    self.cache.go_to_match(&id, 1);
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::thin();

            // Handle any keyboard scrolling requests.
            let user_scrolled = self.cache.handle_keyboard_scrolling(&id, ui);

            // `show_scrollable` will automatically scroll by any accumulated scroll amount
            // before rendering
            CommonMarkViewer::new()
                .max_image_width(Some(512))
                .enable_scroll_to_heading(true)
                .viewport_cache(self.viewport_cache)
                .show_scrollable(id, ui, &mut self.cache, &self.content);

            // Optionally anchor any current search to the current viewport so that Next/Previous will
            // continue from there instead of from its previous location.
            // This call does not affect new searches. In `show-scrollable` mode these will always be
            // anchored to the current viewport because the id is stored in the cache.
            self.cache
                .sync_scrollable_active_match(&id, self.viewport_cache, user_scrolled);
        });
    }
}

fn main() -> eframe::Result {
    let mut args = env::args();
    args.next();

    let viewport_cache = env::var("CACHE")
        .map(|v| v.to_lowercase() != "false" && v != "0")
        .unwrap_or(true);

    let content = build_document();
    eprintln!("Document size: {} bytes", content.len());

    eframe::run_native(
        "Markdown scroll example",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            if let Some(theme) = args.next() {
                if theme == "light" {
                    cc.egui_ctx.set_theme(egui::Theme::Light);
                } else if theme == "dark" {
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                }
            }
            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
                viewport_cache,
                content,
            }))
        }),
    )
}

fn build_document() -> String {
    let mut text = String::from(
        "# Markdown Scroll Example\n\
         \n\
         Jump to: [Section 500](#section-500)\n\
         \n\
         This document demonstrates `show_scrollable`: only the visible slice is \
         rendered each frame, so scrolling stays fast regardless of document length. \
         The TOC link above jumps to section 500, far outside the initial viewport, \
         to demonstrate that heading navigation works across the full document.\
         \n\
         Keyboard shortcuts: Up/Down arrows scroll one line; Page Up/Down scroll \
         one page; Home/End (or Cmd+Up/Down on macOS) jump to the top or bottom.\
         \n",
    );

    for i in 1..=1024_usize {
        let id = if i == 500 { " {#section-500}" } else { "" };
        text += &format!(
            r#"
## Section {i}{id}

This is section {i}. Each section contains a short code block and an image.

```rs
let mut vec = Vec::new();
vec.push({i});
```

* Make a sandwich
* Bake a cake
* Conquer the world

![Ferris the Rust mascot](egui_commonmark/examples/cuddlyferris.png)

"#
        );
    }
    text
}
