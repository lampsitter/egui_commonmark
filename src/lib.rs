//! A commonmark viewer for egui
//!
//! # Example
//!
//! ```
//! # use egui_commonmark::*;
//! # use egui::__run_test_ui;
//! let markdown =
//! r"# Hello world
//!
//! * A list
//! * [ ] Checkbox
//! ";
//! let options = CommonMarkOptions::default();
//! // Stores image handles between each frame
//! let mut cache = CommonMarkCache::default();
//! # __run_test_ui(|ui| {
//! CommonMarkViewer::show(ui, &mut cache, &options, markdown);
//! # });
//!
//! ```
//!

use egui::{self, RichText, Sense, TextStyle};
use egui::{ColorImage, TextureHandle};
use pulldown_cmark::HeadingLevel;
use std::borrow::Borrow;
use std::collections::HashMap;

fn load_image(data: &[u8]) -> image::ImageResult<ColorImage> {
    let image = image::load_from_memory(data)?;
    let image_buffer = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image_buffer.as_flat_samples();

    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

#[cfg(not(feature = "svg"))]
fn try_render_svg(_data: &[u8]) -> Option<ColorImage> {
    None
}

#[cfg(feature = "svg")]
fn try_render_svg(data: &[u8]) -> Option<ColorImage> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &options.to_ref()).ok()?;
    let size = tree.svg_node().size.to_screen_size();

    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(
        &tree,
        usvg::FitTo::Original,
        tiny_skia::Transform::identity(),
        pixmap.as_mut(),
    );

    Some(
        if let Some((_, _, pixmap)) = resvg::trim_transparency(pixmap.clone()) {
            ColorImage::from_rgba_unmultiplied(
                [pixmap.width() as usize, pixmap.height() as usize],
                &pixmap.take(),
            )
        } else {
            ColorImage::from_rgba_unmultiplied(
                [pixmap.width() as usize, pixmap.height() as usize],
                &pixmap.take(),
            )
        },
    )
}

#[derive(Default)]
struct Style {
    heading: Option<pulldown_cmark::HeadingLevel>,
    strong: bool,
    emphasis: bool,
    strikethrough: bool,
    quote: bool,
}

struct Table {
    rows: Vec<Vec<Vec<RichText>>>,
    curr_row: i64,
    curr_cell: i64,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            curr_row: -1,
            curr_cell: -1,
        }
    }
}

#[derive(Default)]
struct Link {
    destination: String,
    text: String,
}

#[derive(Default)]
pub struct CommonMarkCache {
    images: HashMap<String, TextureHandle>,
}

impl CommonMarkCache {
    fn max_image_width(&self, options: &CommonMarkOptions) -> f32 {
        let mut max = 0.0;
        for i in self.images.values() {
            let width = options.image_scaled(i)[0];
            if width >= max {
                max = width;
            }
        }
        max
    }
}

pub struct CommonMarkOptions {
    pub indentation_spaces: usize,
    pub max_image_width: Option<usize>,
    pub show_alt_text_on_hover: bool,
}

impl Default for CommonMarkOptions {
    fn default() -> Self {
        Self {
            indentation_spaces: 4,
            max_image_width: None,
            show_alt_text_on_hover: true,
        }
    }
}

impl CommonMarkOptions {
    fn image_scaled(&self, texture: &TextureHandle) -> egui::Vec2 {
        let size = texture.size();
        if let Some(max_width) = self.max_image_width {
            let width = size[0];

            if width > max_width {
                let height = size[1] as f32;
                let ratio = height / width as f32;

                let scaled_height = ratio * max_width as f32;
                egui::vec2(max_width as f32, scaled_height)
            } else {
                egui::vec2(width as f32, size[1] as f32)
            }
        } else {
            egui::vec2(size[0] as f32, size[1] as f32)
        }
    }
}

pub struct CommonMarkViewer<'ui> {
    ui: &'ui mut egui::Ui,
    /// The current text style
    text_style: Style,
    list_point: Option<u64>,
    link: Option<Link>,
    table: Option<Table>,
    indentation: i64,
    image_alt_text: Option<(egui::Response, Vec<RichText>)>,
    should_insert_newline: bool,
}

impl<'ui> CommonMarkViewer<'ui> {
    fn new(ui: &'ui mut egui::Ui) -> Self {
        Self {
            ui,
            text_style: Style::default(),
            list_point: None,
            link: None,
            table: None,
            indentation: -1,
            image_alt_text: None,
            should_insert_newline: true,
        }
    }
}

impl<'ui> CommonMarkViewer<'ui> {
    pub fn show(
        ui: &'ui mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let max_image_width = cache.max_image_width(options);
        let available_width = ui.available_width();
        let max_width = if max_image_width > available_width {
            max_image_width
        } else {
            available_width
        };

        let initial_size = egui::vec2(max_width, 0.0);

        let layout = egui::Layout::left_to_right()
            .with_main_wrap(true)
            .with_cross_align(egui::Align::BOTTOM);

        ui.allocate_ui_with_layout(initial_size, layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let mut writer = CommonMarkViewer::new(ui);
            for e in pulldown_cmark::Parser::new_ext(text, pulldown_cmark::Options::all()) {
                writer.event(e, cache, options);
            }
        });
    }

    fn newline(&mut self) {
        self.ui
            .allocate_exact_size(egui::vec2(0.0, self.height_body()), Sense::hover());
        self.ui.end_row();
    }

    fn newline_heading(&mut self) {
        self.ui.label("\n");
    }

    fn style_text(&mut self, text: &str) -> RichText {
        let mut text = RichText::new(text);

        if let Some(level) = self.text_style.heading {
            let max_height = self.ui.text_style_height(&TextStyle::Heading);
            let min_height = self.ui.text_style_height(&TextStyle::Body);
            let diff = max_height - min_height;
            match level {
                HeadingLevel::H1 => {
                    text = text.strong().heading();
                }
                HeadingLevel::H2 => {
                    let size = min_height + diff * 0.835;
                    text = text.strong().size(size);
                }
                HeadingLevel::H3 => {
                    let size = min_height + diff * 0.668;
                    text = text.strong().size(size);
                }
                HeadingLevel::H4 => {
                    let size = min_height + diff * 0.501;
                    text = text.strong().size(size);
                }
                HeadingLevel::H5 => {
                    let size = min_height + diff * 0.334;
                    text = text.size(size);
                }
                HeadingLevel::H6 => {
                    let size = min_height + diff * 0.167;
                    text = text.size(size);
                }
            }
        }

        if self.text_style.quote {
            text = text.weak();
        }

        if self.text_style.strong {
            text = text.strong();
        }

        if self.text_style.emphasis {
            // FIXME: Might want to add some space between the next text
            text = text.italics();
        }

        if self.text_style.strikethrough {
            text = text.strikethrough();
        }

        text
    }

    fn event(
        &mut self,
        event: pulldown_cmark::Event,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(tag, cache, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(tag, options),
            pulldown_cmark::Event::Text(text) => {
                if let Some(link) = &mut self.link {
                    link.text += &text;
                } else {
                    let text = self.style_text(text.borrow());
                    if let Some(table) = &mut self.table {
                        table.rows[table.curr_row as usize][table.curr_cell as usize].push(text);
                    } else if let Some((_, alt)) = &mut self.image_alt_text {
                        alt.push(text);
                    } else {
                        self.ui.label(text);
                    }

                    if self.text_style.heading.is_some() {
                        self.newline_heading();
                    }
                }
            }
            pulldown_cmark::Event::Code(text) => {
                self.ui.code(text.as_ref());
            }
            pulldown_cmark::Event::Html(_) => todo!(),
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                self.footnote_start(&footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                self.ui.label(" ");
            }
            pulldown_cmark::Event::HardBreak => self.newline(),
            pulldown_cmark::Event::Rule => {
                self.newline();
                self.ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Event::TaskListMarker(checkbox) => {
                if checkbox {
                    self.ui.label("☑ ");
                } else {
                    self.ui.label("☐ ");
                }
            }
        }
    }

    fn start_tag(
        &mut self,
        tag: pulldown_cmark::Tag,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                if self.should_insert_newline {
                    self.newline();
                }
                self.should_insert_newline = true;
            }
            pulldown_cmark::Tag::Heading(l, _, _) => {
                self.text_style.heading = Some(l);
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = true;
                self.ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Tag::CodeBlock(_) => todo!(),
            pulldown_cmark::Tag::List(number) => {
                self.indentation += 1;
                self.list_point = number;
            }
            pulldown_cmark::Tag::Item => {
                self.newline();
                self.ui
                    .label(" ".repeat(self.indentation as usize * options.indentation_spaces));

                // FIXME: If text is longer than available_width, then the entire
                // text is placed below the point
                if let Some(mut number) = self.list_point.take() {
                    self.numbered_point(&number.to_string());
                    number += 1;
                    self.list_point = Some(number);
                } else if self.indentation >= 1 {
                    self.bullet_point_hollow();
                } else {
                    self.bullet_point();
                }
            }
            pulldown_cmark::Tag::FootnoteDefinition(footnote) => {
                self.should_insert_newline = false;
                self.footnote(&footnote);
            }
            pulldown_cmark::Tag::Table(_) => self.table = Some(Table::default()),
            pulldown_cmark::Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.curr_row += 1;
                    table.rows.push(Vec::new());
                }
            }
            pulldown_cmark::Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.curr_row += 1;
                    table.curr_cell = -1;
                    table.rows.push(Vec::new());
                }
            }
            pulldown_cmark::Tag::TableCell => {
                if let Some(table) = &mut self.table {
                    table.curr_cell += 1;
                    table.rows[table.curr_row as usize].push(Vec::new());
                }
            }
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
                self.link = Some(Link {
                    destination: destination.to_string(),
                    text: String::new(),
                });
            }
            pulldown_cmark::Tag::Image(_, destination, _) => {
                use std::collections::hash_map::Entry;
                let texture = match cache.images.entry(destination.to_string()) {
                    Entry::Occupied(o) => Some(o.get().clone()),
                    Entry::Vacant(v) => {
                        if let Ok(data) = std::fs::read(destination.as_ref()) {
                            let image = if let Ok(image) = load_image(&data) {
                                Some(image)
                            } else {
                                try_render_svg(&data)
                            };

                            if let Some(image) = image {
                                let texture =
                                    self.ui.ctx().load_texture(destination.to_string(), image);

                                v.insert(texture.clone());
                                Some(texture)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                };

                if let Some(texture) = texture {
                    let size = options.image_scaled(&texture);
                    let response = self.ui.image(&texture, size);
                    self.newline();

                    self.image_alt_text = Some((response, vec![]));
                }

                // TODO: Support urls
            }
        }
    }

    fn end_tag(&mut self, tag: pulldown_cmark::Tag, options: &CommonMarkOptions) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                self.newline();
            }
            pulldown_cmark::Tag::Heading(_, _, _) => {
                self.text_style.heading = None;
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = false;
                self.ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Tag::CodeBlock(_) => todo!(),
            pulldown_cmark::Tag::List(_) => {
                self.indentation -= 1;
                self.newline();
            }
            pulldown_cmark::Tag::Item => {}
            pulldown_cmark::Tag::FootnoteDefinition(_) => {}
            pulldown_cmark::Tag::Table(_) => {
                egui::Grid::new("todo_unique id").show(self.ui, |ui| {
                    if let Some(table) = self.table.take() {
                        for row in table.rows {
                            ui.add(egui::Separator::default().vertical());
                            for cell in row {
                                for text in cell {
                                    ui.label(text);
                                }
                                ui.add(egui::Separator::default().vertical());
                            }
                            ui.end_row();
                        }
                    }
                });
            }
            pulldown_cmark::Tag::TableHead => {}
            pulldown_cmark::Tag::TableRow => {}
            pulldown_cmark::Tag::TableCell => {}
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
                    self.ui.hyperlink_to(link.text, link.destination);
                }
            }
            pulldown_cmark::Tag::Image(_, _, _) => {
                if let Some((response, alts)) = self.image_alt_text.take() {
                    if !alts.is_empty() && options.show_alt_text_on_hover {
                        response.on_hover_ui_at_pointer(|ui| {
                            for alt in alts {
                                ui.label(alt);
                            }
                        });
                    }
                }
            }
        }
    }

    fn bullet_point(&mut self) {
        let (rect, _) = self.ui.allocate_exact_size(
            egui::vec2(self.width_body_space() * 4.0, self.height_body()),
            Sense::hover(),
        );
        self.ui.painter().circle_filled(
            rect.center(),
            rect.height() / 6.0,
            self.ui.visuals().strong_text_color(),
        );
    }

    fn bullet_point_hollow(&mut self) {
        let (rect, _) = self.ui.allocate_exact_size(
            egui::vec2(self.width_body_space() * 4.0, self.height_body()),
            Sense::hover(),
        );
        self.ui.painter().circle(
            rect.center(),
            rect.height() / 6.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(0.6, self.ui.visuals().strong_text_color()),
        );
    }

    fn numbered_point(&mut self, number: &str) {
        let (rect, _) = self.ui.allocate_exact_size(
            egui::vec2(self.width_body_space() * 4.0, self.height_body()),
            Sense::hover(),
        );
        self.ui.painter().text(
            rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            format!("{number}."),
            TextStyle::Body.resolve(self.ui.style()),
            self.ui.visuals().strong_text_color(),
        );
    }

    fn footnote_start(&mut self, note: &str) {
        self.ui.label(RichText::new(note).raised().strong().small());
    }

    fn footnote(&mut self, text: &str) {
        let (rect, _) = self.ui.allocate_exact_size(
            egui::vec2(self.width_body_space() * 4.0, self.height_body()),
            Sense::hover(),
        );
        self.ui.painter().text(
            rect.right_top(),
            egui::Align2::RIGHT_TOP,
            format!("{text}."),
            TextStyle::Small.resolve(self.ui.style()),
            self.ui.visuals().strong_text_color(),
        );
    }

    fn height_body(&self) -> f32 {
        self.ui.text_style_height(&TextStyle::Body)
    }

    fn width_body_space(&self) -> f32 {
        let id = TextStyle::Body.resolve(self.ui.style());
        self.ui.fonts().glyph_width(&id, ' ')
    }
}
