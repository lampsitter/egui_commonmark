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
//! # __run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
//! # });
//!
//! ```
//!
#![cfg_attr(feature = "document-features", doc = "# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use std::collections::HashMap;

use egui::{self, epaint, Id, NumExt, Pos2, RichText, Sense, TextStyle, Ui, Vec2};
use pulldown_cmark::{CowStr, HeadingLevel, Options};

#[cfg(feature = "syntax_highlighting")]
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::LinesWithEndings,
};

#[derive(Default, Debug)]
struct ScrollableCache {
    available_size: Vec2,
    page_size: Option<Vec2>,
    split_points: Vec<(usize, Pos2, Pos2)>,
}

/// A cache used for storing content such as images.
pub struct CommonMarkCache {
    // Everything stored in `CommonMarkCache` must take into account that
    // the cache is for multiple `CommonMarkviewer`s with different source_ids.
    #[cfg(feature = "syntax_highlighting")]
    ps: SyntaxSet,

    #[cfg(feature = "syntax_highlighting")]
    ts: ThemeSet,

    link_hooks: HashMap<String, bool>,

    scroll: HashMap<Id, ScrollableCache>,
    has_installed_loaders: bool,
}

#[cfg(not(feature = "syntax_highlighting"))]
impl std::fmt::Debug for CommonMarkCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommonMarkCache")
            .field("images", &format_args!(" {{ .. }} "))
            .field("link_hooks", &self.link_hooks)
            .field("scroll", &self.scroll)
            .finish()
    }
}

#[cfg(feature = "syntax_highlighting")]
impl std::fmt::Debug for CommonMarkCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommonMarkCache")
            .field("images", &format_args!(" {{ .. }}"))
            .field("ps", &self.ps)
            .field("ts", &self.ts)
            .field("link_hooks", &self.link_hooks)
            .field("scroll", &self.scroll)
            .finish()
    }
}

#[allow(clippy::derivable_impls)]
impl Default for CommonMarkCache {
    fn default() -> Self {
        Self {
            #[cfg(feature = "syntax_highlighting")]
            ps: SyntaxSet::load_defaults_newlines(),
            #[cfg(feature = "syntax_highlighting")]
            ts: ThemeSet::load_defaults(),
            link_hooks: HashMap::new(),
            scroll: Default::default(),
            has_installed_loaders: false,
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

    #[cfg(feature = "syntax_highlighting")]
    /// Add more color themes for code blocks(.tmTheme files). Set the color theme with
    /// [`syntax_theme_dark`](CommonMarkViewer::syntax_theme_dark) and
    /// [`syntax_theme_light`](CommonMarkViewer::syntax_theme_light)
    pub fn add_syntax_themes_from_folder(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), syntect::LoadingError> {
        self.ts.add_from_folder(path)
    }

    #[cfg(feature = "syntax_highlighting")]
    /// Add color theme for code blocks(.tmTheme files). Set the color theme with
    /// [`syntax_theme_dark`](CommonMarkViewer::syntax_theme_dark) and
    /// [`syntax_theme_light`](CommonMarkViewer::syntax_theme_light)
    pub fn add_syntax_theme_from_bytes(
        &mut self,
        name: impl Into<String>,
        bytes: &[u8],
    ) -> Result<(), syntect::LoadingError> {
        let mut cursor = std::io::Cursor::new(bytes);
        self.ts
            .themes
            .insert(name.into(), ThemeSet::load_from_reader(&mut cursor)?);
        Ok(())
    }

    /// Clear the cache for all scrollable elements
    pub fn clear_scrollable(&mut self) {
        self.scroll.clear();
    }

    /// Clear the cache for a specific scrollable viewer. Returns false if the
    /// id was not in the cache.
    pub fn clear_scrollable_with_id(&mut self, source_id: impl std::hash::Hash) -> bool {
        self.scroll.remove(&Id::new(source_id)).is_some()
    }

    /// If the user clicks on a link in the markdown render that has `name` as a link. The hook
    /// specified with this method will be set to true. It's status can be acquired
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
    fn curr_theme(&self, ui: &Ui, options: &CommonMarkOptions) -> &Theme {
        self.ts
            .themes
            .get(options.curr_theme(ui))
            // Since we have called load_defaults, the default theme *should* always be available..
            .unwrap_or_else(|| &self.ts.themes[default_theme(ui)])
    }

    fn scroll(&mut self, id: &Id) -> &mut ScrollableCache {
        if !self.scroll.contains_key(id) {
            self.scroll.insert(*id, Default::default());
        }
        self.scroll.get_mut(id).unwrap()
    }

    /// Should be called before any rendering
    fn prepare_show(&mut self, ctx: &egui::Context) {
        if !self.has_installed_loaders {
            // Even though the install function can be called multiple times, its not the cheapest
            // so we ensure that we only call it once.
            // This could be done at the creation of the cache, however it is better to keep the
            // cache free from egui's Ui and Context types as this allows it to be created before
            // any egui instances. It also keeps the API similar to before the introduction of the
            // image loaders.
            egui_extras::loaders::install_image_loaders(ctx);
            self.has_installed_loaders = true;
        }

        self.deactivate_link_hooks();
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
    use_explicit_uri_scheme: bool,
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
            use_explicit_uri_scheme: false,
        }
    }
}

impl CommonMarkOptions {
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

    /// By default any image without a uri scheme such as `foo://` is assumed to
    /// be of the type `file://`. This assumption can sometimes be wrong or be done
    /// incorrectly, so if you want to always be explicit with the scheme then set
    /// this to `true`
    pub fn explicit_image_uri_scheme(mut self, use_explicit: bool) -> Self {
        self.options.use_explicit_uri_scheme = use_explicit;
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
    /// Set the syntax theme to be used inside code blocks in light mode
    pub fn syntax_theme_light<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_light = theme.into();
        self
    }

    #[cfg(feature = "syntax_highlighting")]
    /// Set the syntax theme to be used inside code blocks in dark mode
    pub fn syntax_theme_dark<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_dark = theme.into();
        self
    }

    /// Shows rendered markdown
    pub fn show(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        cache.prepare_show(ui.ctx());
        CommonMarkViewerInternal::new(self.source_id).show(ui, cache, &self.options, text, false);
    }

    /// Shows markdown inside a [`ScrollArea`].
    /// This function is much more performant than just calling [`show`] inside a [`ScrollArea`],
    /// because it only renders elements that are visible.
    ///
    /// # Caveat
    ///
    /// This assumes that the markdown is static. If it does change, you have to clear the cache
    /// by using [`clear_scrollable_with_id`](CommonMarkCache::clear_scrollable_with_id) or
    /// [`clear_scrollable`](CommonMarkCache::clear_scrollable). If the content changes every frame,
    /// it's faster to call [`show`] directly.
    ///
    /// [`ScrollArea`]: egui::ScrollArea
    /// [`show`]: crate::CommonMarkViewer::show
    #[doc(hidden)] // Buggy in scenarios more complex than the example application
    pub fn show_scrollable(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        cache.prepare_show(ui.ctx());
        CommonMarkViewerInternal::new(self.source_id).show_scrollable(
            ui,
            cache,
            &self.options,
            text,
        );
    }
}

/// Supported pulldown_cmark options
fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
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
    uri: String,
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
    /// Be aware that this acquires egui::Context internally.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
        populate_split_points: bool,
    ) {
        let max_width = self.max_width(options, ui);
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

                let max_width = self.max_width(options, ui);
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

    fn max_width(&self, options: &CommonMarkOptions, ui: &Ui) -> f32 {
        let max_image_width = options.max_image_width.unwrap_or(0) as f32;
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

    fn style_text(&mut self, ui: &Ui, text: &str) -> RichText {
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
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, options),
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
            pulldown_cmark::Event::TaskListMarker(mut checkbox) => {
                ui.add(Checkbox::without_text(&mut checkbox));
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
            pulldown_cmark::Tag::Image(_, uri, _) => {
                let has_scheme = uri.contains("://");
                let uri = if options.use_explicit_uri_scheme || has_scheme {
                    uri.to_string()
                } else {
                    // Assume file scheme
                    format!("file://{uri}")
                };

                self.start_image(uri);
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

    fn start_image(&mut self, uri: String) {
        self.image = Some(Image {
            uri,
            alt_text: Vec::new(),
        });
    }

    fn end_image(&mut self, ui: &mut Ui, options: &CommonMarkOptions) {
        if let Some(image) = self.image.take() {
            let response = ui.add(
                egui::Image::from_uri(&image.uri)
                    .fit_to_original_size(1.0)
                    .max_width(self.max_width(options, ui)),
            );
            if !image.alt_text.is_empty() && options.show_alt_text_on_hover {
                response.on_hover_ui_at_pointer(|ui| {
                    for alt in image.alt_text {
                        ui.label(alt);
                    }
                });
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
                Self::pre_syntax_highlighting(cache, options, ui);

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
}

#[cfg(not(feature = "syntax_highlighting"))]
impl CommonMarkViewerInternal {
    fn pre_syntax_highlighting(
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        ui.style_mut().visuals.extreme_bg_color = ui.visuals().extreme_bg_color;
    }

    fn syntax_highlighting(
        &mut self,
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        extension: &str,
        ui: &Ui,
        text: &str,
    ) -> egui::text::LayoutJob {
        plain_highlighting(ui, text, extension)
    }
}

#[cfg(feature = "syntax_highlighting")]
impl CommonMarkViewerInternal {
    fn pre_syntax_highlighting(
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        let curr_theme = cache.curr_theme(ui, options);
        let style = ui.style_mut();

        style.visuals.extreme_bg_color = curr_theme
            .settings
            .background
            .map(syntect_color_to_egui)
            .unwrap_or(style.visuals.extreme_bg_color);

        if let Some(color) = curr_theme.settings.caret {
            style.visuals.text_cursor.color = syntect_color_to_egui(color);
        }

        if let Some(color) = curr_theme.settings.selection_foreground {
            style.visuals.selection.bg_fill = syntect_color_to_egui(color);
        }
    }

    fn syntax_highlighting(
        &mut self,
        cache: &CommonMarkCache,
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
                            syntect_color_to_egui(front),
                        ),
                    );
                }
            }

            job
        } else {
            plain_highlighting(ui, text, extension)
        }
    }
}

fn plain_highlighting(ui: &Ui, text: &str, extension: &str) -> egui::text::LayoutJob {
    egui_extras::syntax_highlighting::highlight(
        ui.ctx(),
        &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()),
        text,
        extension,
    )
}

#[cfg(feature = "syntax_highlighting")]
fn syntect_color_to_egui(color: syntect::highlighting::Color) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
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

#[cfg(feature = "syntax_highlighting")]
fn default_theme(ui: &Ui) -> &str {
    if ui.style().visuals.dark_mode {
        DEFAULT_THEME_DARK
    } else {
        DEFAULT_THEME_LIGHT
    }
}

// Stripped down version of egui's Checkbox. The only difference is that this
// creates a noninteractive checkbox. ui.add_enabled could have been used instead,
// but it makes the checkbox too grey.
struct Checkbox<'a> {
    checked: &'a mut bool,
}

impl<'a> Checkbox<'a> {
    pub fn without_text(checked: &'a mut bool) -> Self {
        Checkbox { checked }
    }
}

impl<'a> egui::Widget for Checkbox<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let Checkbox { checked } = self;

        let spacing = &ui.spacing();
        let icon_width = spacing.icon_width;

        let mut desired_size = egui::vec2(icon_width, 0.0);
        desired_size = desired_size.at_least(Vec2::splat(spacing.interact_size.y));
        desired_size.y = desired_size.y.max(icon_width);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().visuals.noninteractive();
            let (small_icon_rect, big_icon_rect) = ui.spacing().icon_rectangles(rect);
            ui.painter().add(epaint::RectShape::new(
                big_icon_rect.expand(visuals.expansion),
                visuals.rounding,
                visuals.bg_fill,
                visuals.bg_stroke,
            ));

            if *checked {
                // Check mark:
                ui.painter().add(egui::Shape::line(
                    vec![
                        egui::pos2(small_icon_rect.left(), small_icon_rect.center().y),
                        egui::pos2(small_icon_rect.center().x, small_icon_rect.bottom()),
                        egui::pos2(small_icon_rect.right(), small_icon_rect.top()),
                    ],
                    visuals.fg_stroke,
                ));
            }
        }

        response
    }
}
