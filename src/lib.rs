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
//! Remember to opt into the image formats you want to use!
//!
//! ```toml
//! image = { version = "0.24", default-features = false, features = ["png"] }
//! ```
//!
#![cfg_attr(feature = "document-features", doc = "# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use std::collections::HashMap;

use egui::{self, text::LayoutJob, Id, RichText, TextStyle, Ui};

mod alerts;
mod elements;
mod parsers;

pub use alerts::*;

#[cfg(all(feature = "comrak", feature = "pulldown_cmark"))]
compile_error!("Cannot have multiple different parsing backends enabled at the same time");

#[cfg(not(any(feature = "comrak", feature = "pulldown_cmark")))]
compile_error!("Either the pulldown_cmark or comrak backend must be enabled");

#[cfg(feature = "better_syntax_highlighting")]
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::LinesWithEndings,
};

#[cfg(feature = "pulldown_cmark")]
use parsers::pulldown::ScrollableCache;

/// A cache used for storing content such as images.
#[derive(Debug)]
pub struct CommonMarkCache {
    // Everything stored in `CommonMarkCache` must take into account that
    // the cache is for multiple `CommonMarkviewer`s with different source_ids.
    #[cfg(feature = "better_syntax_highlighting")]
    ps: SyntaxSet,

    #[cfg(feature = "better_syntax_highlighting")]
    ts: ThemeSet,

    link_hooks: HashMap<String, bool>,

    #[cfg(feature = "pulldown_cmark")]
    scroll: HashMap<Id, ScrollableCache>,
    has_installed_loaders: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for CommonMarkCache {
    fn default() -> Self {
        Self {
            #[cfg(feature = "better_syntax_highlighting")]
            ps: SyntaxSet::load_defaults_newlines(),
            #[cfg(feature = "better_syntax_highlighting")]
            ts: ThemeSet::load_defaults(),
            link_hooks: HashMap::new(),
            #[cfg(feature = "pulldown_cmark")]
            scroll: Default::default(),
            has_installed_loaders: false,
        }
    }
}

impl CommonMarkCache {
    #[cfg(feature = "better_syntax_highlighting")]
    pub fn add_syntax_from_folder(&mut self, path: &str) {
        let mut builder = self.ps.clone().into_builder();
        let _ = builder.add_from_folder(path, true);
        self.ps = builder.build();
    }

    #[cfg(feature = "better_syntax_highlighting")]
    pub fn add_syntax_from_str(&mut self, s: &str, fallback_name: Option<&str>) {
        let mut builder = self.ps.clone().into_builder();
        let _ = SyntaxDefinition::load_from_str(s, true, fallback_name).map(|d| builder.add(d));
        self.ps = builder.build();
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Add more color themes for code blocks(.tmTheme files). Set the color theme with
    /// [`syntax_theme_dark`](CommonMarkViewer::syntax_theme_dark) and
    /// [`syntax_theme_light`](CommonMarkViewer::syntax_theme_light)
    pub fn add_syntax_themes_from_folder(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), syntect::LoadingError> {
        self.ts.add_from_folder(path)
    }

    #[cfg(feature = "better_syntax_highlighting")]
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
    #[cfg(feature = "pulldown_cmark")]
    pub fn clear_scrollable(&mut self) {
        self.scroll.clear();
    }

    /// Clear the cache for a specific scrollable viewer. Returns false if the
    /// id was not in the cache.
    #[cfg(feature = "pulldown_cmark")]
    pub fn clear_scrollable_with_id(&mut self, source_id: impl std::hash::Hash) -> bool {
        self.scroll.remove(&Id::new(source_id)).is_some()
    }

    /// If the user clicks on a link in the markdown render that has `name` as a link. The hook
    /// specified with this method will be set to true. It's status can be acquired
    /// with [`get_link_hook`](Self::get_link_hook). Be aware that all hook state is reset once
    /// [`CommonMarkViewer::show`] gets called
    ///
    /// # Why use link hooks
    ///
    /// egui provides a method for checking links afterwards so why use this instead?
    ///
    /// ```rust
    /// # use egui::__run_test_ctx;
    /// # __run_test_ctx(|ctx| {
    /// ctx.output_mut(|o| o.open_url.is_some());
    /// # });
    /// ```
    ///
    /// The main difference is that link hooks allows egui_commonmark to check for link hooks
    /// while rendering. Normally when hovering over a link, egui_commonmark will display the full
    /// url. With link hooks this feature is disabled, but to do that all hooks must be known.
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

    #[cfg(feature = "better_syntax_highlighting")]
    fn curr_theme(&self, ui: &Ui, options: &CommonMarkOptions) -> &Theme {
        self.ts
            .themes
            .get(options.curr_theme(ui))
            // Since we have called load_defaults, the default theme *should* always be available..
            .unwrap_or_else(|| &self.ts.themes[default_theme(ui)])
    }

    #[cfg(feature = "pulldown_cmark")]
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
            egui_extras::install_image_loaders(ctx);
            self.has_installed_loaders = true;
        }

        self.deactivate_link_hooks();
    }
}

#[cfg(feature = "better_syntax_highlighting")]
const DEFAULT_THEME_LIGHT: &str = "base16-ocean.light";
#[cfg(feature = "better_syntax_highlighting")]
const DEFAULT_THEME_DARK: &str = "base16-ocean.dark";

#[derive(Debug)]
struct CommonMarkOptions {
    indentation_spaces: usize,
    max_image_width: Option<usize>,
    show_alt_text_on_hover: bool,
    default_width: Option<usize>,
    #[cfg(feature = "better_syntax_highlighting")]
    theme_light: String,
    #[cfg(feature = "better_syntax_highlighting")]
    theme_dark: String,
    use_explicit_uri_scheme: bool,
    default_implicit_uri_scheme: String,
    alerts: AlertBundle,
    /// Whether to present a mutable ui for things like checkboxes
    mutable: bool,
}

impl Default for CommonMarkOptions {
    fn default() -> Self {
        Self {
            indentation_spaces: 4,
            max_image_width: None,
            show_alt_text_on_hover: true,
            default_width: None,
            #[cfg(feature = "better_syntax_highlighting")]
            theme_light: DEFAULT_THEME_LIGHT.to_owned(),
            #[cfg(feature = "better_syntax_highlighting")]
            theme_dark: DEFAULT_THEME_DARK.to_owned(),
            use_explicit_uri_scheme: false,
            default_implicit_uri_scheme: "file://".to_owned(),
            alerts: AlertBundle::gfm(),
            mutable: false,
        }
    }
}

impl CommonMarkOptions {
    #[cfg(feature = "better_syntax_highlighting")]
    fn curr_theme(&self, ui: &Ui) -> &str {
        if ui.style().visuals.dark_mode {
            &self.theme_dark
        } else {
            &self.theme_light
        }
    }

    fn max_width(&self, ui: &Ui) -> f32 {
        let max_image_width = self.max_image_width.unwrap_or(0) as f32;
        let available_width = ui.available_width();

        let max_width = max_image_width.max(available_width);
        if let Some(default_width) = self.default_width {
            if default_width as f32 > max_width {
                default_width as f32
            } else {
                max_width
            }
        } else {
            max_width
        }
    }
}

/// Takes [`egui::Ui`], the math text to be rendered and whether it is inline
type RenderMathFn = dyn Fn(&mut Ui, &str, bool);

pub struct CommonMarkViewer<'a> {
    source_id: Id,
    options: CommonMarkOptions,
    math_fn: Option<&'a RenderMathFn>,
}

impl<'a> CommonMarkViewer<'a> {
    pub fn new(source_id: impl std::hash::Hash) -> Self {
        Self {
            source_id: Id::new(source_id),
            options: CommonMarkOptions::default(),
            math_fn: None,
        }
    }

    pub fn render_math_fn(mut self, math_fn: Option<&'a RenderMathFn>) -> Self {
        self.math_fn = math_fn;
        self
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

    /// Allows changing the default implicit `file://` uri scheme.
    /// This does nothing if [`explicit_image_uri_scheme`](`Self::explicit_image_uri_scheme`) is enabled
    ///
    /// # Example
    /// ```
    /// # use egui_commonmark::CommonMarkViewer;
    /// CommonMarkViewer::new("viewer").default_implicit_uri_scheme("https://example.org/");
    /// ```
    pub fn default_implicit_uri_scheme<S: Into<String>>(mut self, scheme: S) -> Self {
        self.options.default_implicit_uri_scheme = scheme.into();
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

    #[cfg(feature = "better_syntax_highlighting")]
    #[deprecated(note = "use `syntax_theme_light` or `syntax_theme_dark` instead")]
    pub fn syntax_theme(mut self, theme: String) -> Self {
        self.options.theme_light = theme.clone();
        self.options.theme_dark = theme;
        self
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Set the syntax theme to be used inside code blocks in light mode
    pub fn syntax_theme_light<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_light = theme.into();
        self
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Set the syntax theme to be used inside code blocks in dark mode
    pub fn syntax_theme_dark<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_dark = theme.into();
        self
    }

    #[cfg(not(feature = "comrak"))] // not supported by the backend atm.
    /// Specify what kind of alerts are supported. This can also be used to localize alerts.
    ///
    /// By default [github flavoured markdown style alerts](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts)
    /// are used
    pub fn alerts(mut self, alerts: AlertBundle) -> Self {
        self.options.alerts = alerts;
        self
    }

    /// Shows rendered markdown
    pub fn show(
        self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &str,
    ) -> egui::InnerResponse<()> {
        cache.prepare_show(ui.ctx());

        #[cfg(feature = "pulldown_cmark")]
        let (response, _) = parsers::pulldown::CommonMarkViewerInternal::new(self.source_id).show(
            ui,
            cache,
            &self.options,
            text,
            false,
            self.math_fn,
        );

        #[cfg(feature = "comrak")]
        let response = parsers::comrak::CommonMarkViewerInternal::new(self.source_id).show(
            ui,
            cache,
            &self.options,
            text,
            self.math_fn,
        );

        response
    }

    /// Shows rendered markdown, and allows the rendered ui to mutate the source text.
    ///
    /// The only currently implemented mutation is allowing checkboxes to be toggled through the ui.
    #[cfg(feature = "pulldown_cmark")]
    pub fn show_mut(
        mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &mut String,
    ) -> egui::InnerResponse<()> {
        self.options.mutable = true;
        cache.prepare_show(ui.ctx());

        let (response, checkmark_events) = parsers::pulldown::CommonMarkViewerInternal::new(
            self.source_id,
        )
        .show(ui, cache, &self.options, text, false, self.math_fn);

        // Update source text for checkmarks that were clicked
        for ev in checkmark_events {
            if ev.checked {
                text.replace_range(ev.span, "[x]")
            } else {
                text.replace_range(ev.span, "[ ]")
            }
        }

        response
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
    #[cfg(feature = "pulldown_cmark")]
    pub fn show_scrollable(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        cache.prepare_show(ui.ctx());
        parsers::pulldown::CommonMarkViewerInternal::new(self.source_id).show_scrollable(
            ui,
            cache,
            &self.options,
            text,
            self.math_fn,
        );
    }
}

#[derive(Default)]
struct Style {
    heading: Option<u8>,
    strong: bool,
    emphasis: bool,
    strikethrough: bool,
    quote: bool,
    code: bool,
}

impl Style {
    fn to_richtext(&self, ui: &Ui, text: &str) -> RichText {
        let mut text = RichText::new(text);

        if let Some(level) = self.heading {
            let max_height = ui
                .style()
                .text_styles
                .get(&TextStyle::Heading)
                .map_or(32.0, |d| d.size);
            let min_height = ui
                .style()
                .text_styles
                .get(&TextStyle::Body)
                .map_or(14.0, |d| d.size);
            let diff = max_height - min_height;

            match level {
                0 => {
                    text = text.strong().heading();
                }
                1 => {
                    let size = min_height + diff * 0.835;
                    text = text.strong().size(size);
                }
                2 => {
                    let size = min_height + diff * 0.668;
                    text = text.strong().size(size);
                }
                3 => {
                    let size = min_height + diff * 0.501;
                    text = text.strong().size(size);
                }
                4 => {
                    let size = min_height + diff * 0.334;
                    text = text.size(size);
                }
                // We only support 6 levels
                5.. => {
                    let size = min_height + diff * 0.167;
                    text = text.size(size);
                }
            }
        }

        if self.quote {
            text = text.weak();
        }

        if self.strong {
            text = text.strong();
        }

        if self.emphasis {
            // FIXME: Might want to add some space between the next text
            text = text.italics();
        }

        if self.strikethrough {
            text = text.strikethrough();
        }

        if self.code {
            text = text.code();
        }

        text
    }
}

#[derive(Default)]
struct Link {
    destination: String,
    text: Vec<RichText>,
}

impl Link {
    fn end(self, ui: &mut Ui, cache: &mut CommonMarkCache) {
        let Self { destination, text } = self;

        let mut layout_job = LayoutJob::default();
        for t in text {
            t.append_to(
                &mut layout_job,
                ui.style(),
                egui::FontSelection::Default,
                egui::Align::LEFT,
            );
        }
        if cache.link_hooks().contains_key(&destination) {
            let ui_link = ui.link(layout_job);
            if ui_link.clicked() || ui_link.middle_clicked() {
                cache.link_hooks_mut().insert(destination, true);
            }
        } else {
            ui.hyperlink_to(layout_job, destination);
        }
    }
}

struct Image {
    uri: String,
    alt_text: Vec<RichText>,
}

impl Image {
    // FIXME: string conversion
    pub fn new(uri: &str, options: &CommonMarkOptions) -> Self {
        let has_scheme = uri.contains("://");
        let uri = if options.use_explicit_uri_scheme || has_scheme {
            uri.to_string()
        } else {
            // Assume file scheme
            format!("{}{uri}", options.default_implicit_uri_scheme)
        };

        Self {
            uri,
            alt_text: Vec::new(),
        }
    }

    fn end(self, ui: &mut Ui, options: &CommonMarkOptions) {
        let response = ui.add(
            egui::Image::from_uri(&self.uri)
                .fit_to_original_size(1.0)
                .max_width(options.max_width(ui)),
        );

        if !self.alt_text.is_empty() && options.show_alt_text_on_hover {
            response.on_hover_ui_at_pointer(|ui| {
                for alt in self.alt_text {
                    ui.label(alt);
                }
            });
        }
    }
}

struct FencedCodeBlock {
    lang: String,
    content: String,
}

impl FencedCodeBlock {
    fn end(
        &self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        ui.scope(|ui| {
            Self::pre_syntax_highlighting(cache, options, ui);

            let mut layout = |ui: &Ui, string: &str, wrap_width: f32| {
                let mut job = self.syntax_highlighting(cache, options, &self.lang, ui, string);
                job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(job))
            };

            elements::code_block(ui, max_width, &self.content, &mut layout);
        });
    }
}

#[cfg(not(feature = "better_syntax_highlighting"))]
impl FencedCodeBlock {
    fn pre_syntax_highlighting(
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        ui.style_mut().visuals.extreme_bg_color = ui.visuals().extreme_bg_color;
    }

    fn syntax_highlighting(
        &self,
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        extension: &str,
        ui: &Ui,
        text: &str,
    ) -> egui::text::LayoutJob {
        crate::plain_highlighting(ui, text, extension)
    }
}

#[cfg(feature = "better_syntax_highlighting")]
impl FencedCodeBlock {
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
            .map(crate::syntect_color_to_egui)
            .unwrap_or(style.visuals.extreme_bg_color);

        if let Some(color) = curr_theme.settings.selection_foreground {
            style.visuals.selection.bg_fill = crate::syntect_color_to_egui(color);
        }
    }

    fn syntax_highlighting(
        &self,
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
                            crate::syntect_color_to_egui(front),
                        ),
                    );
                }
            }

            job
        } else {
            crate::plain_highlighting(ui, text, extension)
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

#[cfg(feature = "better_syntax_highlighting")]
fn syntect_color_to_egui(color: syntect::highlighting::Color) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
}

#[cfg(feature = "better_syntax_highlighting")]
fn default_theme(ui: &Ui) -> &str {
    if ui.style().visuals.dark_mode {
        DEFAULT_THEME_DARK
    } else {
        DEFAULT_THEME_LIGHT
    }
}
