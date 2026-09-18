//! Demonstrates search-match highlighting with default colors.
//!
//! Typing in the search bar highlights every match. Prev and Next step through
//! matches and scroll the document to centre the active match in the viewport.
//!
//! Run with:
//! `cargo r --example search --features better_syntax_highlighting,svg,fetch,embedded_image,egui_extras/svg_text -- [light|dark]`

use eframe::egui;
use egui::Color32;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer, SearchOptions};

const INTRO: &str = r#"# Search Highlighting

Type text in the search bar above to highlight every occurrence in this document.

Use **Prev (Shift-Enter)** and **Next (Enter)** to step through matches.

> [!TIP]
>    1. Try searching for "crate" or "as" to see image matches on image Alt text as well as search scrolling behavior.
>    2. Try searching text in different text types in the various sections from the included example markdown files below.
>    3. Try the case-sensitive, whole-word and regex searches by toggling their respective icons.

"#;

const SCROLL_TO_HEADING: &str = r"# Contents {#contents}

- [Heading 1](#heading1)
- [Heading 2](#heading2)

# Heading 1 {#heading1}

[back to contents](#contents)

Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

## Heading 2 {#heading2}

[back to contents](#contents)

Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.
";

struct App {
    cache: CommonMarkCache,
    search_focus: bool,
    egui_source_id: String,
    content: String,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_min_height(512.0);

        let (cmd_f, search_escape) = ui.ctx().input(|i| {
            use egui::Key;
            (
                i.modifiers.command && i.key_pressed(Key::F),
                i.key_pressed(Key::Escape),
            )
        });

        egui::Panel::top("search_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let text_color = if self.cache.search_regex_error.is_some() {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().text_color()
                };

                ui.label("Search:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.cache.search_query).text_color(text_color),
                );
                if let Some(error) = &self.cache.search_regex_error {
                    response.clone().on_hover_text(error);
                }

                // Lay out and test search options
                let search_options_changed = self.search_options_changed(ui);

                if search_options_changed || response.changed() {
                    self.cache
                        .update_search_matches(&self.egui_source_id, &self.content);
                }

                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if cmd_f {
                    self.search_focus = !self.search_focus;
                } else if search_escape {
                    self.search_focus = false;
                } else if response.lost_focus() {
                    // The user clicked away or Enter caused the single-line
                    // TextEdit to surrender focus naturally — honour that
                    // instead of fighting to keep the box focused.
                    self.search_focus = false;
                }
                if self.search_focus {
                    response.request_focus();
                } else {
                    response.surrender_focus();
                }

                let match_count = self.cache.search_ranges().len();
                ui.label(match self.cache.active_match() {
                    Some(i) if match_count > 0 => format!("{}/{match_count}", i + 1),
                    _ => format!("0/{match_count}"),
                });

                if ui.button("Previous").clicked()
                    || (enter_pressed && ui.input(|i| i.modifiers.shift))
                {
                    self.cache.go_to_match(-1);
                }
                if ui.button("Next").clicked()
                    || (enter_pressed && !ui.input(|i| i.modifiers.shift))
                {
                    self.cache.go_to_match(1);
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::thin();

            ui.style_mut().url_in_tooltip = true;

            // Handle any keyboard scrolling requests
            let user_scrolled = self.cache.handle_keyboard_scrolling(ui);

            ui.separator();

            // Demonstrating optional custom search match highlight colors.
            // They need to be visible on light and dark backgrounds and not
            // clash with existing backgrounds or highlighted text. You may
            // need to use different shades for light vs dark, and you may
            // need to adjust the opacity for visibility of text and existing
            // background.
            let active_bg = Color32::ORANGE;
            let match_bg = Color32::GOLD;

            // To anchor searches to the scroll position, optionally replace `show` by `show_with_id`
            // and then call `self.cache.sync_active_match`.
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Scroll by accumulated scroll amount before rendering
                self.cache.apply_pending_scroll_delta(ui);
                CommonMarkViewer::new()
                    // Optionally override default search match colors
                    .search_active_match_color(active_bg)
                    .search_match_color(match_bg)
                    .enable_scroll_to_heading(true)
                    .show_with_id(&self.egui_source_id, ui, &mut self.cache, &self.content);
            });

            // Optionally anchor any current or new search to the current viewport so that
            // Next/Previous will continue from there instead of from its previous location.
            // New searches are affected only when using regular `CommonMarkViewer::show`:
            // without this call they will start from the top of the document.
            // When using `CommonMarkViewer::show_with_id`, new searches will always be
            // anchored to the current viewport anyway, thanks to the `egui_source_id` argument.
            self.cache.sync_active_match(user_scrolled);
        });
    }
}

impl App {
    // Lays out the search option buttons and checks if they've changed from frame to frame.
    fn search_options_changed(&mut self, ui: &mut egui::Ui) -> bool {
        let mut search_options_changed = false;

        let mut search_toggle =
            |ui: &mut egui::Ui, flag: SearchOptions, label: egui::WidgetText, tooltip: String| {
                let selected = self.cache.search_options.contains(flag);

                if ui
                    .selectable_label(selected, label)
                    .on_hover_text(tooltip)
                    .clicked()
                {
                    self.cache.search_options.toggle(flag);
                    search_options_changed = true;
                }
            };

        search_toggle(
            ui,
            SearchOptions::CASE_SENSITIVE,
            "Aa".into(),
            "Case sensitive search".to_string(),
        );

        search_toggle(
            ui,
            SearchOptions::WHOLE_WORD,
            egui::RichText::new("wd").underline().into(),
            "Whole word search".to_string(),
        );

        search_toggle(
            ui,
            SearchOptions::REGEX,
            ".*".into(),
            "Regex search".to_string(),
        );
        search_options_changed
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args();
    args.next();

    let readme = include_str!("../README.md").lines().map(|l|
        {
            if l.starts_with(r#"<img src="https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v4.png""#) {
                "[![Showcase](https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v4.png)](https://raw.githubusercontent.com/lampsitter/egui_commonmark/master/assets/example-v4.png)
"
            } else {l}
        }).collect::<Vec<_>>().join("\n");

    let lists = include_str!("markdown/lists.md");
    let definition_list = include_str!("markdown/definition_list.md");
    let blockquotes = include_str!("markdown/blockquotes.md");
    let tables = include_str!("markdown/tables.md");
    let wide_table = include_str!("markdown/wide_table.md");

    let content = format!(
        r"{INTRO}

---

## README

{readme}

---

## Scroll to heading

{SCROLL_TO_HEADING}

---

## Lists

{lists}

---

{definition_list}

---

{blockquotes}

---

{tables}

---

{wide_table}

        "
    );

    eframe::run_native(
        "Markdown search example",
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
                egui_source_id: String::from("search_example"),
                content,
                search_focus: true,
            }))
        }),
    )
}
