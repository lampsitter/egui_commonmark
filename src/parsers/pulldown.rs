//! Duplicates a lot of stuff for now.

use crate::elements::*;
use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, TextStyle, Ui, Vec2};

use pulldown_cmark::{CowStr, HeadingLevel, Options};

#[derive(Default, Debug)]
pub struct ScrollableCache {
    available_size: Vec2,
    page_size: Option<Vec2>,
    split_points: Vec<(usize, Pos2, Pos2)>,
}

/// Supported pulldown_cmark options
fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
}

pub struct CommonMarkViewerInternal {
    source_id: Id,
    curr_table: usize,
    text_style: crate::Style,
    list_point: Option<u64>,
    link: Option<crate::Link>,
    indentation: i64,
    image: Option<crate::Image>,
    should_insert_newline: bool,
    fenced_code_block: Option<crate::FencedCodeBlock>,
    is_table: bool,
}

impl CommonMarkViewerInternal {
    pub fn new(source_id: Id) -> Self {
        Self {
            source_id,
            curr_table: 0,
            text_style: crate::Style::default(),
            list_point: None,
            link: None,
            indentation: -1,
            image: None,
            should_insert_newline: true,
            fenced_code_block: None,
            is_table: false,
        }
    }
}

impl CommonMarkViewerInternal {
    /// Be aware that this acquires egui::Context internally.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
        populate_split_points: bool,
    ) {
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let mut events = pulldown_cmark::Parser::new_ext(text, parser_options()).enumerate();

            while let Some((index, e)) = events.next() {
                let start_position = ui.next_widget_position();
                let is_element_end = matches!(e, pulldown_cmark::Event::End(_));
                let should_add_split_point = self.indentation == -1 && is_element_end;

                self.process_event(ui, &mut events, e, cache, options, max_width);

                if populate_split_points && should_add_split_point {
                    let scroll_cache = cache.scroll(&self.source_id);
                    let end_position = ui.next_widget_position();

                    let split_point_exists = scroll_cache
                        .split_points
                        .iter()
                        .any(|(i, _, _)| *i == index);

                    if !split_point_exists {
                        scroll_cache
                            .split_points
                            .push((index, start_position, end_position));
                    }
                }
            }

            cache.scroll(&self.source_id).page_size = Some(ui.next_widget_position().to_vec2());
        });
    }

    pub fn show_scrollable(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let available_size = ui.available_size();
        let scroll_id = self.source_id.with("_scroll_area");

        let Some(page_size) = cache.scroll(&self.source_id).page_size else {
            egui::ScrollArea::vertical()
                .id_source(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.show(ui, cache, options, text, true);
                });
            // Prevent repopulating points twice at startup
            cache.scroll(&self.source_id).available_size = available_size;
            return;
        };

        let events = pulldown_cmark::Parser::new_ext(text, parser_options()).collect::<Vec<_>>();

        let num_rows = events.len();

        egui::ScrollArea::vertical()
            .id_source(scroll_id)
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                ui.set_height(page_size.y);
                let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

                let max_width = options.max_width(ui);
                ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let scroll_cache = cache.scroll(&self.source_id);

                    // finding the first element that's not in the viewport anymore
                    let (first_event_index, _, first_end_position) = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, _, end_position)| end_position.y < viewport.min.y)
                        .nth_back(1)
                        .copied()
                        .unwrap_or((0, Pos2::ZERO, Pos2::ZERO));

                    // finding the last element that's just outside the viewport
                    let last_event_index = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, start_position, _)| start_position.y > viewport.max.y)
                        .nth(1)
                        .map(|(index, _, _)| *index)
                        .unwrap_or(num_rows);

                    ui.allocate_space(first_end_position.to_vec2());

                    // only rendering the elements that are inside the viewport
                    let mut events = events
                        .into_iter()
                        .enumerate()
                        .skip(first_event_index)
                        .take(last_event_index - first_event_index);

                    while let Some((_, e)) = events.next() {
                        self.process_event(ui, &mut events, e, cache, options, max_width);
                    }
                });
            });

        // Forcing full re-render to repopulate split points for the new size
        let scroll_cache = cache.scroll(&self.source_id);
        if available_size != scroll_cache.available_size {
            scroll_cache.available_size = available_size;
            scroll_cache.page_size = None;
            scroll_cache.split_points.clear();
        }
    }

    fn process_event<'e>(
        &mut self,
        ui: &mut Ui,
        events: &mut impl Iterator<Item = (usize, pulldown_cmark::Event<'e>)>,
        event: pulldown_cmark::Event,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        self.event(ui, event, cache, options, max_width);
        self.fenced_code_block(events, max_width, cache, options, ui);
        self.table(events, cache, options, ui, max_width);
    }

    fn fenced_code_block<'e>(
        &mut self,
        events: &mut impl Iterator<Item = (usize, pulldown_cmark::Event<'e>)>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        while self.fenced_code_block.is_some() {
            if let Some((_, e)) = events.next() {
                self.event(ui, e, cache, options, max_width);
            } else {
                break;
            }
        }
    }

    fn table<'e>(
        &mut self,
        events: &mut impl Iterator<Item = (usize, pulldown_cmark::Event<'e>)>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
        max_width: f32,
    ) {
        if self.is_table {
            newline(ui);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                let id = self.source_id.with(self.curr_table);
                self.curr_table += 1;
                egui::Grid::new(id).striped(true).show(ui, |ui| {
                    while self.is_table {
                        if let Some((_, e)) = events.next() {
                            self.should_insert_newline = false;
                            self.event(ui, e, cache, options, max_width);
                        } else {
                            break;
                        }
                    }
                });
            });

            newline(ui);
        }
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        event: pulldown_cmark::Event,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => {
                self.event_text(text, ui);
            }
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                self.event_text(text, ui);
                self.text_style.code = false;
            }
            pulldown_cmark::Event::Html(_) => {}
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                footnote_start(ui, &footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                soft_break(ui);
            }
            pulldown_cmark::Event::HardBreak => newline(ui),
            pulldown_cmark::Event::Rule => {
                newline(ui);
                ui.add(egui::Separator::default().horizontal());
                // This does not add a new line, but instead ends the separator
                newline(ui);
            }
            pulldown_cmark::Event::TaskListMarker(mut checkbox) => {
                ui.add(Checkbox::without_text(&mut checkbox));
            }
        }
    }

    fn event_text(&mut self, text: CowStr, ui: &mut Ui) {
        let rich_text = self.text_style.to_richtext(ui, &text);
        if let Some(image) = &mut self.image {
            image.alt_text.push(rich_text);
        } else if let Some(block) = &mut self.fenced_code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            link.text.push(rich_text);
        } else {
            ui.label(rich_text);
        }
    }

    fn start_tag(&mut self, ui: &mut Ui, tag: pulldown_cmark::Tag, options: &CommonMarkOptions) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                if self.should_insert_newline {
                    newline(ui);
                }
                self.should_insert_newline = true;
            }
            pulldown_cmark::Tag::Heading(l, _, _) => {
                newline(ui);
                self.text_style.heading = Some(match l {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                });
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = true;
                ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                if let pulldown_cmark::CodeBlockKind::Fenced(lang) = c {
                    self.fenced_code_block = Some(crate::FencedCodeBlock {
                        lang: lang.to_string(),
                        content: "".to_string(),
                    });

                    newline(ui);
                }

                self.text_style.code = true;
            }
            pulldown_cmark::Tag::List(number) => {
                self.indentation += 1;
                self.list_point = number;
            }
            pulldown_cmark::Tag::Item => {
                self.start_item(ui, options);
            }
            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.should_insert_newline = false;
                footnote(ui, &note);
            }
            pulldown_cmark::Tag::Table(_) => {
                self.is_table = true;
            }
            pulldown_cmark::Tag::TableHead => {}
            pulldown_cmark::Tag::TableRow => {}
            pulldown_cmark::Tag::TableCell => {}
            pulldown_cmark::Tag::Emphasis => {
                self.text_style.emphasis = true;
            }
            pulldown_cmark::Tag::Strong => {
                self.text_style.strong = true;
            }
            pulldown_cmark::Tag::Strikethrough => {
                self.text_style.strikethrough = true;
            }
            pulldown_cmark::Tag::Link(_, destination, _) => {
                self.link = Some(crate::Link {
                    destination: destination.to_string(),
                    text: Vec::new(),
                });
            }
            pulldown_cmark::Tag::Image(_, uri, _) => {
                self.image = Some(crate::Image::new(&uri, options));
            }
        }
    }

    fn end_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::Tag,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                newline(ui);
            }
            pulldown_cmark::Tag::Heading(_, _, _) => {
                newline(ui);
                self.text_style.heading = None;
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = false;
                ui.add(egui::Separator::default().horizontal());
                newline(ui);
            }
            pulldown_cmark::Tag::CodeBlock(_) => {
                self.end_code_block(ui, cache, options, max_width);
            }
            pulldown_cmark::Tag::List(_) => {
                self.indentation -= 1;
                if self.indentation == -1 {
                    newline(ui);
                    self.should_insert_newline = true;
                }
            }
            pulldown_cmark::Tag::Item => {}
            pulldown_cmark::Tag::FootnoteDefinition(_) => {}
            pulldown_cmark::Tag::Table(_) => {
                self.is_table = false;
            }
            pulldown_cmark::Tag::TableHead => {
                ui.end_row();
            }
            pulldown_cmark::Tag::TableRow => {
                ui.end_row();
            }
            pulldown_cmark::Tag::TableCell => {
                // Ensure space between cells
                ui.label("  ");
            }
            pulldown_cmark::Tag::Emphasis => {
                self.text_style.emphasis = false;
            }
            pulldown_cmark::Tag::Strong => {
                self.text_style.strong = false;
            }
            pulldown_cmark::Tag::Strikethrough => {
                self.text_style.strikethrough = false;
            }
            pulldown_cmark::Tag::Link(_, _, _) => {
                if let Some(link) = self.link.take() {
                    link.end(ui, cache);
                }
            }
            pulldown_cmark::Tag::Image(_, _, _) => {
                if let Some(image) = self.image.take() {
                    image.end(ui, options);
                }
            }
        }
    }

    fn start_item(&mut self, ui: &mut Ui, options: &CommonMarkOptions) {
        newline(ui);
        ui.label(" ".repeat(self.indentation as usize * options.indentation_spaces));

        self.should_insert_newline = false;
        if let Some(mut number) = self.list_point.take() {
            number_point(ui, &number.to_string());
            number += 1;
            self.list_point = Some(number);
        } else if self.indentation >= 1 {
            bullet_point_hollow(ui);
        } else {
            bullet_point(ui);
        }
    }

    fn end_code_block(
        &mut self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        if let Some(block) = self.fenced_code_block.take() {
            block.end(ui, cache, options, max_width);
            self.text_style.code = false;
        }
    }
}
