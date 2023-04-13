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

use egui::{self, Id, Pos2, RichText, Sense, TextStyle, Ui, Vec2};
use egui::{ColorImage, TextureHandle};
use pulldown_cmark::{CowStr, HeadingLevel};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(feature = "syntax_highlighting")]
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::LinesWithEndings,
};

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
    let mut options = usvg::Options::default();
    options.fontdb.load_system_fonts();
    let tree = usvg::Tree::from_data(data, &options.to_ref()).ok()?;
    let size = tree.svg_node().size.to_screen_size();

    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(
        &tree,
        usvg::FitTo::Original,
        tiny_skia::Transform::default(),
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
struct ScrollableCache {
    available_size: Vec2,
    page_size: Option<Vec2>,
    split_points: Vec<(usize, Pos2, Pos2)>,
}

type ImageHashMap = Arc<Mutex<HashMap<String, Option<TextureHandle>>>>;

// Everything stored here must take into account that the cache is for multiple
// CommonMarkviewers with different source_ids.
pub struct CommonMarkCache {
    images: ImageHashMap,
    #[cfg(feature = "syntax_highlighting")]
    ps: SyntaxSet,
    #[cfg(feature = "syntax_highlighting")]
    ts: ThemeSet,
    link_hooks: HashMap<String, bool>,
    scroll: HashMap<Id, ScrollableCache>,
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
            link_hooks: HashMap::new(),
            scroll: Default::default(),
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

    #[cfg(feature = "syntax_highlighting")]
    pub fn add_syntax_from_str(&mut self, s: &str, fallback_name: Option<&str>) {
        let mut builder = self.ps.clone().into_builder();
        let _ = SyntaxDefinition::load_from_str(s, true, fallback_name).map(|d| builder.add(d));
        self.ps = builder.build();
    }

    /// Refetch all images
    pub fn reload_images(&mut self) {
        self.images.lock().unwrap().clear();
    }

    /// Clear the cache for scrollable elements
    pub fn clear_scrollable(&mut self) {
        self.scroll.clear();
    }

    /// If the user clicks on a link in the markdown render that has `name` as a link. The hook
    /// specified with this method will be set to true. It's status can be aquired
    /// with [`get_link_hook`](Self::get_link_hook). Be aware that all hooks are reset once
    /// [`CommonMarkViewer::show`] gets called
    pub fn add_link_hook<S: Into<String>>(&mut self, name: S) {
        self.link_hooks.insert(name.into(), false);
    }

    /// Returns None if the link hook could not be found. Returns the last known status of the
    /// hook otherwise.
    pub fn remove_link_hook(&mut self, name: &str) -> Option<bool> {
        self.link_hooks.remove(name)
    }

    /// Get status of link. Returns true if it was clicked
    pub fn get_link_hook(&self, name: &str) -> Option<bool> {
        self.link_hooks.get(name).copied()
    }

    /// Remove all link hooks
    pub fn link_hooks_clear(&mut self) {
        self.link_hooks.clear();
    }

    /// All link hooks
    pub fn link_hooks(&self) -> &HashMap<String, bool> {
        &self.link_hooks
    }

    /// Raw access to link hooks
    pub fn link_hooks_mut(&mut self) -> &mut HashMap<String, bool> {
        &mut self.link_hooks
    }

    /// Set all link hooks to false
    fn deactivate_link_hooks(&mut self) {
        for v in self.link_hooks.values_mut() {
            *v = false;
        }
    }

    #[cfg(feature = "syntax_highlighting")]
    fn background_colour(&mut self, ui: &Ui, options: &CommonMarkOptions) -> egui::Color32 {
        if let Some(bg) = self.curr_theme(ui, options).settings.background {
            egui::Color32::from_rgb(bg.r, bg.g, bg.b)
        } else {
            ui.visuals().extreme_bg_color
        }
    }

    #[cfg(not(feature = "syntax_highlighting"))]
    fn background_colour(&mut self, ui: &Ui, _options: &CommonMarkOptions) -> egui::Color32 {
        ui.visuals().extreme_bg_color
    }

    #[cfg(feature = "syntax_highlighting")]
    fn curr_theme(&self, ui: &Ui, options: &CommonMarkOptions) -> &Theme {
        self.ts
            .themes
            .get(options.curr_theme(ui))
            // Since we have called load_defaults, the default theme *should* always be available..
            .unwrap_or_else(|| &self.ts.themes[default_theme(ui)])
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

    fn scroll(&mut self, id: &Id) -> &mut ScrollableCache {
        if !self.scroll.contains_key(id) {
            self.scroll.insert(*id, Default::default());
        }
        self.scroll.get_mut(id).unwrap()
    }
}

#[cfg(feature = "syntax_highlighting")]
const DEFAULT_THEME_LIGHT: &str = "base16-ocean.light";
#[cfg(feature = "syntax_highlighting")]
const DEFAULT_THEME_DARK: &str = "base16-ocean.dark";

struct CommonMarkOptions {
    indentation_spaces: usize,
    max_image_width: Option<usize>,
    show_alt_text_on_hover: bool,
    default_width: Option<usize>,
    #[cfg(feature = "syntax_highlighting")]
    theme_light: String,
    #[cfg(feature = "syntax_highlighting")]
    theme_dark: String,
}

impl Default for CommonMarkOptions {
    fn default() -> Self {
        Self {
            indentation_spaces: 4,
            max_image_width: None,
            show_alt_text_on_hover: true,
            default_width: None,
            #[cfg(feature = "syntax_highlighting")]
            theme_light: DEFAULT_THEME_LIGHT.to_owned(),
            #[cfg(feature = "syntax_highlighting")]
            theme_dark: DEFAULT_THEME_DARK.to_owned(),
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

    #[cfg(feature = "syntax_highlighting")]
    fn curr_theme(&self, ui: &Ui) -> &str {
        if ui.style().visuals.dark_mode {
            &self.theme_dark
        } else {
            &self.theme_light
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
    #[deprecated(note = "use `syntax_theme_light` or `syntax_theme_dark` instead")]
    pub fn syntax_theme(mut self, theme: String) -> Self {
        self.options.theme_light = theme.clone();
        self.options.theme_dark = theme;
        self
    }

    #[cfg(feature = "syntax_highlighting")]
    pub fn syntax_theme_light<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_light = theme.into();
        self
    }

    #[cfg(feature = "syntax_highlighting")]
    pub fn syntax_theme_dark<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_dark = theme.into();
        self
    }

    pub fn show(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        cache.deactivate_link_hooks();
        CommonMarkViewerInternal::new(self.source_id).show(ui, cache, &self.options, text, false);
    }

    /// Shows markdown file inside a ScrollArea
    /// This function is much more performant than just calling show inside a ScrollArea,
    /// because it only renders elements that are visible.
    pub fn show_scrollable(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        cache.deactivate_link_hooks();
        CommonMarkViewerInternal::new(self.source_id).show_scrollable(
            ui,
            cache,
            &self.options,
            text,
        );
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

struct FencedCodeBlock {
    lang: String,
    content: String,
}

struct CommonMarkViewerInternal {
    source_id: Id,
    curr_table: usize,
    text_style: Style,
    list_point: Option<u64>,
    link: Option<Link>,
    indentation: i64,
    image: Option<Image>,
    should_insert_newline: bool,
    fenced_code_block: Option<FencedCodeBlock>,
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
        populate_split_points: bool,
    ) {
        let max_width = self.max_width(cache, options, ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            use pulldown_cmark::Options;
            let parser_options = Options::ENABLE_TABLES
                | Options::ENABLE_TASKLISTS
                | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_FOOTNOTES;
            let mut events = pulldown_cmark::Parser::new_ext(text, parser_options).enumerate();

            while let Some((index, e)) = events.next() {
                let start_position = ui.next_widget_position();
                let is_element_end = matches!(e, pulldown_cmark::Event::End(_));
                let should_add_split_point = self.indentation == -1 && is_element_end;

                self.event(ui, e, cache, options, max_width);

                self.fenced_code_block(&mut events, max_width, cache, options, ui);
                self.table(&mut events, cache, options, ui, max_width);

                if populate_split_points {
                    let scroll_cache = cache.scroll(&self.source_id);
                    let end_position = ui.next_widget_position();

                    let split_point_exists = scroll_cache
                        .split_points
                        .iter()
                        .any(|(i, _, _)| *i == index);

                    if should_add_split_point && !split_point_exists {
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

        let Some(page_size) = cache.scroll(&self.source_id).page_size else {
            egui::ScrollArea::vertical()
                .id_source(self.source_id.with("_scroll_area"))
                .show(ui, |ui| {
                self.show(ui, cache, options, text, true);
            });
            return;
        };

        use pulldown_cmark::Options;
        let parser_options = Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_FOOTNOTES;
        let events = pulldown_cmark::Parser::new_ext(text, parser_options).collect::<Vec<_>>();

        let num_rows = events.len();

        egui::ScrollArea::vertical()
            .id_source(self.source_id.with("_scroll_area"))
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                ui.set_height(page_size.y);
                let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

                let max_width = self.max_width(cache, options, ui);
                ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
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
                        self.event(ui, e, cache, options, max_width);
                        self.fenced_code_block(&mut events, max_width, cache, options, ui);
                        self.table(&mut events, cache, options, ui, max_width);
                    }
                });
            });

        // Forcing full re-render to repopulate split points for the new size
        let scroll_cache = cache.scroll(&self.source_id);
        if available_size != scroll_cache.available_size {
            scroll_cache.available_size = available_size;
            scroll_cache.page_size = None;
        }
    }

    fn max_width(&self, cache: &CommonMarkCache, options: &CommonMarkOptions, ui: &Ui) -> f32 {
        let max_image_width = cache.max_image_width(options);
        let available_width = ui.available_width();

        let max_width = max_image_width.max(available_width);
        if let Some(default_width) = options.default_width {
            if default_width as f32 > max_width {
                default_width as f32
            } else {
                max_width
            }
        } else {
            max_width
        }
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
        max_width: f32,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, cache, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => {
                self.event_text(text, ui);
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
                    checkbox_point(ui, "☑ ")
                } else {
                    checkbox_point(ui, "☐ ")
                }
            }
        }
    }

    fn event_text(&mut self, text: CowStr, ui: &mut Ui) {
        if let Some(link) = &mut self.link {
            link.text += &text;
        } else {
            let rich_text = self.style_text(ui, &text);
            if let Some(image) = &mut self.image {
                image.alt_text.push(rich_text);
            } else if let Some(block) = &mut self.fenced_code_block {
                block.content.push_str(&text);
            } else {
                ui.label(rich_text);
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
                newline(ui);
                self.text_style.heading = Some(l);
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.text_style.quote = true;
                ui.add(egui::Separator::default().horizontal());
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                if let pulldown_cmark::CodeBlockKind::Fenced(lang) = c {
                    self.fenced_code_block = Some(FencedCodeBlock {
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
                self.link = Some(Link {
                    destination: destination.to_string(),
                    text: String::new(),
                });
            }
            pulldown_cmark::Tag::Image(_, url, _) => self.start_image(url.to_string(), ui, cache),
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
                self.end_link(ui, cache);
            }
            pulldown_cmark::Tag::Image(_, _, _) => {
                self.end_image(ui, options);
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

    fn end_link(&mut self, ui: &mut Ui, cache: &mut CommonMarkCache) {
        if let Some(link) = self.link.take() {
            if cache.link_hooks().contains_key(&link.destination) {
                let ui_link = ui.link(link.text);
                if ui_link.clicked() || ui_link.middle_clicked() {
                    cache.link_hooks_mut().insert(link.destination, true);
                }
            } else {
                ui.hyperlink_to(link.text, link.destination);
            }
        }
    }

    fn start_image(&mut self, url: String, ui: &mut Ui, cache: &mut CommonMarkCache) {
        let handle = match cache.images.lock().unwrap().entry(url.clone()) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let ctx = ui.ctx();
                let handle = get_image_data(&url, ctx, Arc::clone(&cache.images))
                    .and_then(|data| parse_image(ctx, &url, &data));

                v.insert(handle.clone());
                handle
            }
        };

        self.image = Some(Image {
            handle,
            url,
            alt_text: Vec::new(),
        });
    }

    fn end_image(&mut self, ui: &mut Ui, options: &CommonMarkOptions) {
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
                self.should_insert_newline = false;
            }
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
            ui.scope(|ui| {
                ui.style_mut().visuals.extreme_bg_color = cache.background_colour(ui, options);
                let mut layout = |ui: &Ui, string: &str, wrap_width: f32| {
                    let mut job = self.syntax_highlighting(cache, options, &block.lang, ui, string);
                    job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(job))
                };
                ui.add(
                    egui::TextEdit::multiline(
                        &mut block
                            .content
                            .strip_suffix('\n')
                            .unwrap_or(&block.content)
                            .to_string(),
                    )
                    .layouter(&mut layout)
                    .desired_width(max_width)
                    // prevent trailing lines
                    .desired_rows(1),
                );
            });
        }
        self.text_style.code = false;
        newline(ui);
    }

    #[cfg(feature = "syntax_highlighting")]
    fn syntax_highlighting(
        &mut self,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        extension: &str,
        ui: &Ui,
        text: &str,
    ) -> egui::text::LayoutJob {
        if let Some(syntax) = cache.ps.find_syntax_by_extension(extension) {
            let mut job = egui::text::LayoutJob::default();
            let mut h = HighlightLines::new(syntax, cache.curr_theme(ui, options));

            for line in LinesWithEndings::from(text) {
                let ranges = h.highlight_line(line, &cache.ps).unwrap();
                for v in ranges {
                    let front = v.0.foreground;
                    job.append(
                        v.1,
                        0.0,
                        egui::TextFormat::simple(
                            TextStyle::Monospace.resolve(ui.style()),
                            egui::Color32::from_rgb(front.r, front.g, front.b),
                        ),
                    );
                }
            }

            job
        } else {
            plain_highlighting(ui, text)
        }
    }

    #[cfg(not(feature = "syntax_highlighting"))]
    fn syntax_highlighting(
        &mut self,
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        _extension: &str,
        ui: &Ui,
        text: &str,
    ) -> egui::text::LayoutJob {
        plain_highlighting(ui, text)
    }
}

fn plain_highlighting(ui: &Ui, text: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat::simple(
            TextStyle::Monospace.resolve(ui.style()),
            ui.style().visuals.text_color(),
        ),
    );
    job
}

fn newline(ui: &mut Ui) {
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

fn checkbox_point(ui: &mut Ui, ty: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 5.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        ty,
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
    );
}

fn number_point(ui: &mut Ui, number: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        format!("{number}. "),
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
    ui.fonts(|f| f.glyph_width(&id, ' '))
}

fn parse_image(ctx: &egui::Context, url: &str, data: &[u8]) -> Option<TextureHandle> {
    let image = load_image(data).ok().or_else(|| try_render_svg(data));
    image.map(|image| ctx.load_texture(url, image, egui::TextureOptions::LINEAR))
}

#[cfg(feature = "fetch")]
fn get_image_data(path: &str, ctx: &egui::Context, images: ImageHashMap) -> Option<Vec<u8>> {
    let url = url::Url::parse(path);
    if url.is_ok() {
        let ctx2 = ctx.clone();
        let path = path.to_owned();
        ehttp::fetch(ehttp::Request::get(&path), move |r| {
            if let Ok(r) = r {
                let data = r.bytes;
                if let Some(handle) = parse_image(&ctx2, &path, &data) {
                    // we only update if the image was loaded properly
                    *images.lock().unwrap().get_mut(&path).unwrap() = Some(handle);
                    ctx2.request_repaint();
                }
            }
        });

        None
    } else {
        get_image_data_from_file(path)
    }
}

#[cfg(not(feature = "fetch"))]
fn get_image_data(path: &str, _ctx: &egui::Context, _images: ImageHashMap) -> Option<Vec<u8>> {
    get_image_data_from_file(path)
}

fn get_image_data_from_file(url: &str) -> Option<Vec<u8>> {
    std::fs::read(url).ok()
}

#[cfg(feature = "syntax_highlighting")]
fn default_theme(ui: &Ui) -> &str {
    if ui.style().visuals.dark_mode {
        DEFAULT_THEME_DARK
    } else {
        DEFAULT_THEME_LIGHT
    }
}
