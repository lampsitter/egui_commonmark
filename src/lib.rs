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
//! // Stores image handles between each frame
//! let mut cache = CommonMarkCache::default();
//! # __run_test_ui(|ui| {
//! CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
//! # });
//!
//! ```
//!

use egui::{self, Id, RichText, Sense, TextStyle, Ui};
use egui::{ColorImage, TextureHandle};
use pulldown_cmark::HeadingLevel;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

#[cfg(feature = "syntax_highlighting")]
use syntect::easy::HighlightLines;
#[cfg(feature = "syntax_highlighting")]
use syntect::highlighting::ThemeSet;
#[cfg(feature = "syntax_highlighting")]
use syntect::parsing::SyntaxSet;

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

type ImageHashMap = Arc<Mutex<HashMap<String, Option<TextureHandle>>>>;
pub struct CommonMarkCache {
    images: ImageHashMap,
    #[cfg(feature = "syntax_highlighting")]
    ps: SyntaxSet,
    #[cfg(feature = "syntax_highlighting")]
    ts: ThemeSet,
}

#[allow(clippy::derivable_impls)]
impl Default for CommonMarkCache {
    fn default() -> Self {
        Self {
            images: Default::default(),
            #[cfg(feature = "syntax_highlighting")]
            ps: SyntaxSet::load_defaults_newlines(),
            #[cfg(feature = "syntax_highlighting")]
            ts: ThemeSet::load_defaults(),
        }
    }
}

impl CommonMarkCache {
    #[cfg(feature = "syntax_highlighting")]
    pub fn add_syntax_from_folder(&mut self, path: &str) {
        let mut builder = self.ps.clone().into_builder();
        let _ = builder.add_from_folder(path, true);
        self.ps = builder.build();
    }

    /// Refetch all images
    pub fn reload_images(&mut self) {
        self.images.lock().unwrap().clear();
    }

    #[cfg(feature = "syntax_highlighting")]
    fn background_colour(&mut self, options: &CommonMarkOptions) -> egui::Color32 {
        if let Some(bg) = self.ts.themes[&options.theme].settings.background {
            egui::Color32::from_rgb(bg.r, bg.g, bg.b)
        } else {
            egui::Color32::BLACK
        }
    }

    #[cfg(not(feature = "syntax_highlighting"))]
    fn background_colour(&mut self, _options: &CommonMarkOptions) -> egui::Color32 {
        egui::Color32::BLACK
    }

    fn max_image_width(&self, options: &CommonMarkOptions) -> f32 {
        let mut max = 0.0;
        for i in self.images.lock().unwrap().values().flatten() {
            let width = options.image_scaled(i)[0];
            if width >= max {
                max = width;
            }
        }
        max
    }
}

struct CommonMarkOptions {
    indentation_spaces: usize,
    max_image_width: Option<usize>,
    show_alt_text_on_hover: bool,
    default_width: Option<usize>,
    #[cfg(feature = "syntax_highlighting")]
    theme: String,
}

impl Default for CommonMarkOptions {
    fn default() -> Self {
        Self {
            indentation_spaces: 4,
            max_image_width: None,
            show_alt_text_on_hover: true,
            default_width: None,
            #[cfg(feature = "syntax_highlighting")]
            theme: "base16-mocha.dark".to_owned(),
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

pub struct CommonMarkViewer {
    source_id: Id,
    options: CommonMarkOptions,
}

impl CommonMarkViewer {
    pub fn new(source_id: impl std::hash::Hash) -> Self {
        Self {
            source_id: Id::new(source_id),
            options: CommonMarkOptions::default(),
        }
    }

    /// The amount of spaces a bullet point is indented. By default this is 4
    /// spaces.
    pub fn indentation_spaces(mut self, spaces: usize) -> Self {
        self.options.indentation_spaces = spaces;
        self
    }

    /// The maximum size images are allowed to be. They will be scaled down if
    /// they are larger
    pub fn max_image_width(mut self, width: Option<usize>) -> Self {
        self.options.max_image_width = width;
        self
    }

    /// The default width of the ui. This is only respected if this is larger than
    /// the [`max_image_width`](Self::max_image_width)
    pub fn default_width(mut self, width: Option<usize>) -> Self {
        self.options.default_width = width;
        self
    }

    /// Show alt text when hovering over images. By default this is enabled.
    pub fn show_alt_text_on_hover(mut self, show: bool) -> Self {
        self.options.show_alt_text_on_hover = show;
        self
    }

    #[cfg(feature = "syntax_highlighting")]
    pub fn syntax_theme(mut self, theme: String) -> Self {
        self.options.theme = theme;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        CommonMarkViewerInternal::new(self.source_id).show(ui, cache, &self.options, text);
    }
}

#[derive(Default)]
struct Style {
    heading: Option<pulldown_cmark::HeadingLevel>,
    strong: bool,
    emphasis: bool,
    strikethrough: bool,
    quote: bool,
    code: bool,
}

#[derive(Default)]
struct Link {
    destination: String,
    text: String,
}

struct Image {
    handle: Option<TextureHandle>,
    url: String,
    alt_text: Vec<RichText>,
}

struct CommonMarkViewerInternal {
    source_id: Id,
    curr_table: usize,
    /// The current text style
    text_style: Style,
    list_point: Option<u64>,
    link: Option<Link>,
    indentation: i64,
    image: Option<Image>,
    should_insert_newline: bool,
    is_first_heading: bool,
    fenced_code_block: Option<String>,
    is_table: bool,
}

impl CommonMarkViewerInternal {
    fn new(source_id: Id) -> Self {
        Self {
            source_id,
            curr_table: 0,
            text_style: Style::default(),
            list_point: None,
            link: None,
            indentation: -1,
            image: None,
            should_insert_newline: true,
            is_first_heading: true,
            fenced_code_block: None,
            is_table: false,
        }
    }
}

impl CommonMarkViewerInternal {
    /// Be aware that this aquires egui::Context internally.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let max_image_width = cache.max_image_width(options);
        let available_width = ui.available_width();

        let max_width = max_image_width.max(available_width);
        let max_width = if let Some(default_width) = options.default_width {
            if default_width as f32 > max_width {
                default_width as f32
            } else {
                max_width
            }
        } else {
            max_width
        };

        let layout = egui::Layout::left_to_right()
            .with_main_wrap(true)
            .with_cross_align(egui::Align::BOTTOM);

        ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let mut events = pulldown_cmark::Parser::new_ext(text, pulldown_cmark::Options::all());

            while let Some(e) = events.next() {
                self.event(ui, e, cache, options);

                self.fenced_code_block(&mut events, max_width, cache, options, ui);
                self.table(&mut events, cache, options, ui);
            }
        });
    }

    fn fenced_code_block<'e>(
        &mut self,
        events: &mut impl Iterator<Item = pulldown_cmark::Event<'e>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.fenced_code_block.is_some() {
            let bg_colour = cache.background_colour(options);
            egui::Frame::default()
                .fill(bg_colour)
                .margin(egui::vec2(0.0, 0.0))
                .show(ui, |ui| {
                    ui.set_min_width(max_width);

                    while self.fenced_code_block.is_some() {
                        if let Some(e) = events.next() {
                            self.event(ui, e, cache, options);
                        } else {
                            break;
                        }
                    }
                });
            newline(ui);
        }
    }

    fn table<'e>(
        &mut self,
        events: &mut impl Iterator<Item = pulldown_cmark::Event<'e>>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_table {
            newline(ui);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                let id = self.source_id.with(self.curr_table);
                self.curr_table += 1;
                egui::Grid::new(id).striped(true).show(ui, |ui| {
                    while self.is_table {
                        if let Some(e) = events.next() {
                            self.should_insert_newline = false;
                            self.event(ui, e, cache, options);
                        } else {
                            break;
                        }
                    }
                });
            });

            newline(ui);
        }
    }

    fn style_text(&mut self, ui: &mut Ui, text: &str) -> RichText {
        let mut text = RichText::new(text);

        if let Some(level) = self.text_style.heading {
            let max_height = ui.text_style_height(&TextStyle::Heading);
            let min_height = ui.text_style_height(&TextStyle::Body);
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

        if self.text_style.code {
            text = text.font(TextStyle::Monospace.resolve(ui.style()))
        }

        text
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        event: pulldown_cmark::Event,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, cache, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, options),
            pulldown_cmark::Event::Text(text) => {
                if let Some(link) = &mut self.link {
                    link.text += &text;
                } else {
                    let rich_text = self.style_text(ui, &text);
                    if let Some(image) = &mut self.image {
                        image.alt_text.push(rich_text);
                    } else if let Some(lang) = &self.fenced_code_block.clone() {
                        self.syntax_highlighting(cache, options, lang, ui, &text);
                    } else {
                        ui.label(rich_text);
                    }

                    if self.text_style.heading.is_some() {
                        newline_heading(ui);
                    }
                }
            }
            pulldown_cmark::Event::Code(text) => {
                ui.code(text.as_ref());
            }
            pulldown_cmark::Event::Html(_) => {}
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                footnote_start(ui, &footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                ui.label(" ");
            }
            pulldown_cmark::Event::HardBreak => newline(ui),
            pulldown_cmark::Event::Rule => {
                newline(ui);
                ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Event::TaskListMarker(checkbox) => {
                if checkbox {
                    ui.label("☑ ");
                } else {
                    ui.label("☐ ");
                }
            }
        }
    }

    fn start_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::Tag,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                if self.should_insert_newline {
                    newline(ui);
                }
                self.should_insert_newline = true;
            }
            pulldown_cmark::Tag::Heading(l, _, _) => {
                if matches!(l, HeadingLevel::H1) {
                    if !self.is_first_heading {
                        newline_heading(ui);
                    }
                    self.is_first_heading = false;
                } else {
                    newline_heading(ui);
                }

                self.text_style.heading = Some(l);
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = true;
                ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                if let pulldown_cmark::CodeBlockKind::Fenced(lang) = c {
                    self.fenced_code_block = Some(lang.to_string());
                    newline(ui);
                }

                self.text_style.code = true;
            }
            pulldown_cmark::Tag::List(number) => {
                self.indentation += 1;
                self.list_point = number;
            }
            pulldown_cmark::Tag::Item => {
                newline(ui);
                ui.label(" ".repeat(self.indentation as usize * options.indentation_spaces));

                self.should_insert_newline = false;
                if let Some(mut number) = self.list_point.take() {
                    numbered_point(ui, &number.to_string());
                    number += 1;
                    self.list_point = Some(number);
                } else if self.indentation >= 1 {
                    bullet_point_hollow(ui);
                } else {
                    bullet_point(ui);
                }
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
                self.link = Some(Link {
                    destination: destination.to_string(),
                    text: String::new(),
                });
            }
            pulldown_cmark::Tag::Image(_, url, _) => {
                let handle = match cache.images.lock().unwrap().entry(url.to_string()) {
                    Entry::Occupied(o) => o.get().clone(),
                    Entry::Vacant(v) => {
                        let ctx = ui.ctx();
                        let handle =
                            get_image_data(url.to_string(), ctx, Arc::clone(&cache.images))
                                .and_then(|data| parse_image(ctx, url.to_string(), &data));

                        v.insert(handle.clone());
                        handle
                    }
                };

                self.image = Some(Image {
                    handle,
                    url: url.to_string(),
                    alt_text: Vec::new(),
                });

                // TODO: Support urls
            }
        }
    }

    fn end_tag(&mut self, ui: &mut Ui, tag: pulldown_cmark::Tag, options: &CommonMarkOptions) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                newline(ui);
            }
            pulldown_cmark::Tag::Heading(_, _, _) => {
                self.text_style.heading = None;
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = false;
                ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Tag::CodeBlock(_) => {
                self.fenced_code_block = None;
                self.text_style.code = false;
                newline(ui);
            }
            pulldown_cmark::Tag::List(_) => {
                self.indentation -= 1;
                newline(ui);
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
                    ui.hyperlink_to(link.text, link.destination);
                }
            }
            pulldown_cmark::Tag::Image(_, _, _) => {
                if let Some(image) = self.image.take() {
                    if let Some(texture) = image.handle {
                        let size = options.image_scaled(&texture);
                        let response = ui.image(&texture, size);

                        if !image.alt_text.is_empty() && options.show_alt_text_on_hover {
                            response.on_hover_ui_at_pointer(|ui| {
                                for alt in image.alt_text {
                                    ui.label(alt);
                                }
                            });
                        }
                    } else {
                        ui.label("![");
                        for alt in image.alt_text {
                            ui.label(alt);
                        }
                        ui.label(format!("]({})", image.url));
                    }

                    if self.should_insert_newline {
                        newline(ui);
                        self.should_insert_newline = true;
                    }
                }
            }
        }
    }

    #[cfg(feature = "syntax_highlighting")]
    fn syntax_highlighting(
        &mut self,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        extension: &str,
        ui: &mut Ui,
        text: &str,
    ) {
        if let Some(syntax) = cache.ps.find_syntax_by_extension(extension) {
            let mut h = HighlightLines::new(syntax, &cache.ts.themes[&options.theme]);
            let ranges = h.highlight(text, &cache.ps);
            for v in ranges {
                let front = v.0.foreground;
                ui.label(
                    RichText::new(v.1)
                        .color(egui::Color32::from_rgb(front.r, front.g, front.b))
                        .font(TextStyle::Monospace.resolve(ui.style())),
                );
            }
        } else {
            let rich_text = self.style_text(ui, text);
            ui.label(rich_text);
        }
    }

    #[cfg(not(feature = "syntax_highlighting"))]
    fn syntax_highlighting(
        &mut self,
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        _extension: &str,
        ui: &mut Ui,
        text: &str,
    ) {
        let rich_text = self.style_text(ui, text);
        ui.label(rich_text);
    }
}

fn newline(ui: &mut Ui) {
    ui.allocate_exact_size(egui::vec2(0.0, height_body(ui)), Sense::hover());
    ui.end_row();
}

fn newline_heading(ui: &mut Ui) {
    ui.label("\n");
}

fn bullet_point(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().circle_filled(
        rect.center(),
        rect.height() / 6.0,
        ui.visuals().strong_text_color(),
    );
}

fn bullet_point_hollow(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().circle(
        rect.center(),
        rect.height() / 6.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(0.6, ui.visuals().strong_text_color()),
    );
}

fn numbered_point(ui: &mut Ui, number: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        format!("{number}."),
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}

fn footnote_start(ui: &mut Ui, note: &str) {
    ui.label(RichText::new(note).raised().strong().small());
}

fn footnote(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_top(),
        egui::Align2::RIGHT_TOP,
        format!("{text}."),
        TextStyle::Small.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}

fn height_body(ui: &Ui) -> f32 {
    ui.text_style_height(&TextStyle::Body)
}

fn width_body_space(ui: &Ui) -> f32 {
    let id = TextStyle::Body.resolve(ui.style());
    ui.fonts().glyph_width(&id, ' ')
}

fn parse_image(ctx: &egui::Context, url: String, data: &[u8]) -> Option<TextureHandle> {
    let image = load_image(data).ok().or_else(|| try_render_svg(data));
    image.map(|image| ctx.load_texture(url, image))
}

#[cfg(feature = "fetch")]
fn get_image_data(path: String, ctx: &egui::Context, images: ImageHashMap) -> Option<Vec<u8>> {
    let url = url::Url::parse(&path);
    if url.is_ok() {
        let ctx2 = ctx.clone();
        ehttp::fetch(ehttp::Request::get(&path), move |r| {
            if let Ok(r) = r {
                let data = r.bytes;
                if let Some(handle) = parse_image(&ctx2, path.clone(), &data) {
                    // we only update if the image was loaded properly
                    *images.lock().unwrap().get_mut(&path).unwrap() = Some(handle);
                    ctx2.request_repaint();
                }
            }
        });

        None
    } else {
        get_image_data_from_file(&path)
    }
}

#[cfg(not(feature = "fetch"))]
fn get_image_data(path: String, _ctx: &egui::Context, _images: ImageHashMap) -> Option<Vec<u8>> {
    get_image_data_from_file(&path)
}

fn get_image_data_from_file(url: &str) -> Option<Vec<u8>> {
    std::fs::read(url).ok()
}
