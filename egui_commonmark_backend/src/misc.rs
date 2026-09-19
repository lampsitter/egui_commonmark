use crate::alerts::{AlertBundle, try_get_alert};
use bitflags::bitflags;
use egui::{RichText, TextBuffer, TextStyle, Ui, text::LayoutJob};
use std::collections::HashMap;
use std::ops::Range;

use crate::pulldown::ScrollableCache;

#[cfg(feature = "better_syntax_highlighting")]
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::LinesWithEndings,
};

#[cfg(feature = "better_syntax_highlighting")]
const DEFAULT_THEME_LIGHT: &str = "base16-ocean.light";
#[cfg(feature = "better_syntax_highlighting")]
const DEFAULT_THEME_DARK: &str = "base16-ocean.dark";

pub struct CommonMarkOptions<'f> {
    pub indentation_spaces: usize,
    pub max_image_width: Option<usize>,
    pub show_alt_text_on_hover: bool,
    pub default_width: Option<usize>,
    #[cfg(feature = "better_syntax_highlighting")]
    pub theme_light: String,
    #[cfg(feature = "better_syntax_highlighting")]
    pub theme_dark: String,
    pub use_explicit_uri_scheme: bool,
    pub default_implicit_uri_scheme: String,
    pub alerts: AlertBundle,
    /// Whether to present a mutable ui for things like checkboxes
    pub mutable: bool,
    pub math_fn: Option<&'f crate::RenderMathFn>,
    pub html_fn: Option<&'f crate::RenderHtmlFn>,
    /// Whether to enable scrolling to headings by their ID.
    /// To give a heading an ID, use the syntax `# Heading {#myheadingid}`. Then links to `#myheadingid` e.g. `[click me!](#myheadingid)` will scroll to that heading.
    pub enable_scroll_to_heading: bool,
    /// When `true`, `show_scrollable` only renders the visible slice of the
    /// document each frame. When `false` (the default) the full document is
    /// rendered every frame and egui clips what is off-screen.
    pub use_viewport_cache: bool,
    /// Background colour for passive search matches. When `None`, a
    /// theme-derived default is used (see [`crate::search::default_match_bg`]).
    pub search_match_bg: Option<egui::Color32>,
    /// Background colour for the active (focused) search match. When
    /// `None`, a theme-derived default is used (see
    /// [`crate::search::default_active_match_bg`]).
    pub search_active_match_bg: Option<egui::Color32>,
    /// When set via [`show_with_id`](crate::CommonMarkViewer::show_with_id),
    /// `full_render` records block-boundary positions (split points) under
    /// this id so that
    /// [`viewport_start_byte_offset`](CommonMarkCache::viewport_start_byte_offset)
    /// works and
    /// [`update_search_matches`](CommonMarkCache::update_search_matches) can
    /// anchor new searches to the current viewport position.
    /// Not used by `show_scrollable`, which carries its own source id.
    pub source_id: Option<egui::Id>,
}

impl std::fmt::Debug for CommonMarkOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("CommonMarkOptions");

        s.field("indentation_spaces", &self.indentation_spaces)
            .field("max_image_width", &self.max_image_width)
            .field("show_alt_text_on_hover", &self.show_alt_text_on_hover)
            .field("default_width", &self.default_width);

        #[cfg(feature = "better_syntax_highlighting")]
        s.field("theme_light", &self.theme_light)
            .field("theme_dark", &self.theme_dark);

        s.field("use_explicit_uri_scheme", &self.use_explicit_uri_scheme)
            .field(
                "default_implicit_uri_scheme",
                &self.default_implicit_uri_scheme,
            )
            .field("alerts", &self.alerts)
            .field("mutable", &self.mutable)
            .field("search_match_bg", &self.search_match_bg)
            .field("search_active_match_bg", &self.search_active_match_bg)
            .field("source_id", &self.source_id)
            .finish()
    }
}

impl Default for CommonMarkOptions<'_> {
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
            math_fn: None,
            html_fn: None,
            enable_scroll_to_heading: false,
            use_viewport_cache: false,
            search_match_bg: None,
            search_active_match_bg: None,
            source_id: None,
        }
    }
}

impl CommonMarkOptions<'_> {
    #[cfg(feature = "better_syntax_highlighting")]
    pub fn curr_theme(&self, ui: &Ui) -> &str {
        if ui.style().visuals.dark_mode {
            &self.theme_dark
        } else {
            &self.theme_light
        }
    }

    pub fn max_width(&self, ui: &Ui) -> f32 {
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

    /// The background colour to use for passive search matches: the
    /// explicit override if one was set, otherwise a theme-derived default.
    pub fn search_match_bg(&self, ui: &Ui) -> egui::Color32 {
        self.search_match_bg
            .unwrap_or_else(|| crate::search::default_match_bg(ui.visuals()))
    }

    /// The background colour to use for the active search match: the
    /// explicit override if one was set, otherwise a theme-derived default.
    pub fn search_active_match_bg(&self, ui: &Ui) -> egui::Color32 {
        self.search_active_match_bg
            .unwrap_or_else(|| crate::search::default_active_match_bg(ui.visuals()))
    }
}

#[derive(Default, Clone)]
pub struct Style {
    pub heading: Option<u8>,
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub quote: bool,
    pub code: bool,
}

impl Style {
    pub fn to_richtext(&self, ui: &Ui, text: &str) -> RichText {
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
pub struct Link {
    pub destination: String,
    pub text: Vec<RichText>,
    /// For each accumulated `text` piece, its local byte range within the
    /// final rendered job's text paired with its byte range in the original
    /// source. Used to translate global search match ranges into positions
    /// local to this link's rendered text.
    pub chunks: Vec<(Range<usize>, Range<usize>)>,
}

impl Link {
    /// Append a piece of link text, recording its source span so search
    /// matches can later be mapped back onto it.
    pub fn push_text(&mut self, text: RichText, src_span: Range<usize>) {
        let local_start: usize = self.text.iter().map(|t| t.text().len()).sum();
        let local_end = local_start + text.text().len();
        self.chunks.push((local_start..local_end, src_span));
        self.text.push(text);
    }

    /// Renders the link. If `want_scroll_to_active_match` is true and the
    /// currently active search match falls inside this link's text, the view
    /// is scrolled (centering the link) and `true` is returned so the caller
    /// knows the request has been fulfilled.
    /// `content_origin_y` is the screen-space Y of the document top for the
    /// current render pass (as recorded by `CommonMarkViewerInternal`). It is
    /// subtracted from the link widget's screen Y to produce the virtual
    /// (scroll-independent) Y stored in `search_match_virtual_ys`.
    ///
    /// Returns `(scrolled, match_ys)` where `match_ys` is a list of
    /// `(global_match_index, virtual_y)` pairs for every search match that
    /// falls inside this link's text. The caller should extend
    /// `search_match_ys_scratch` with these so that `sync_active_match` can
    /// correctly identify which visual row link matches are on.
    pub fn end(
        self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        scroll_to_heading: &mut Option<String>,
        want_scroll_to_active_match: bool,
        content_origin_y: f32,
    ) -> (bool, Vec<(usize, f32)>) {
        let Self {
            destination,
            text,
            chunks,
        } = self;

        // When a link wraps an image (`[![alt](img)](url)`), all text events are captured
        // by the image widget and link.text is never populated. Rendering an empty Label in
        // a wrapping layout resets cursor.min.x to 0, superimposing subsequent elements on
        // the image that was just drawn. Nothing to render, so return early.
        if text.is_empty() {
            return (false, vec![]);
        }

        let ranges = cache.search_ranges();
        let (intervals, has_active_match) = if ranges.is_empty() {
            (vec![], false)
        } else {
            let intervals = crate::search::chunked_search_intervals(
                &chunks,
                ranges,
                cache.active_search_range(),
            );
            let has_active_match = intervals.iter().any(|(_, is_active)| *is_active);
            (intervals, has_active_match)
        };

        let mut layout_job = LayoutJob::default();
        for t in text {
            t.append_to(
                &mut layout_job,
                ui.style(),
                egui::FontSelection::Default,
                egui::Align::LEFT,
            );
        }
        if !intervals.is_empty() {
            crate::search::apply_search_highlights(
                &mut layout_job,
                &intervals,
                options.search_match_bg(ui),
                options.search_active_match_bg(ui),
            );
        }

        let response = if cache.link_hooks().contains_key(&destination) {
            let ui_link = ui.link(layout_job);
            if ui_link.clicked() || ui_link.middle_clicked() {
                cache.link_hooks_mut().insert(destination, true);
            }
            ui_link
        } else if options.enable_scroll_to_heading
            && let Some(stripped) = destination.strip_prefix("#")
        {
            let response = ui.link(layout_job);
            if response.clicked() {
                scroll_to_heading.replace(stripped.to_string());
            };
            response
        } else {
            ui.hyperlink_to(layout_job, destination)
        };

        let scrolled = if has_active_match && want_scroll_to_active_match {
            ui.scroll_to_rect(response.rect, Some(egui::Align::Center));
            true
        } else {
            false
        };

        // Record the virtual Y for every search match inside this link.
        // Link text is accumulated and rendered as a single widget, so it
        // never goes through the `event_text` else-branch that normally
        // fills `search_match_ys_scratch`. Use the widget's top-left Y
        // (same row as the surrounding inline text) for all of them.
        let link_virtual_y = response.rect.min.y - content_origin_y;
        let link_src_start = chunks
            .iter()
            .map(|(_, s)| s.start)
            .min()
            .unwrap_or(usize::MAX);
        let link_src_end = chunks.iter().map(|(_, s)| s.end).max().unwrap_or(0);
        let match_ys: Vec<(usize, f32)> = cache
            .search_ranges()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.start < link_src_end && r.end > link_src_start)
            .map(|(i, _)| (i, link_virtual_y))
            .collect();

        (scrolled, match_ys)
    }
}

pub struct Image {
    pub uri: String,
    pub alt_text: Vec<RichText>,
    /// Source byte spans (in the original markdown) of each alt-text [`Text`]
    /// event accumulated while this image was being parsed. Used to match
    /// global search ranges against the image at render time.
    ///
    /// [`Text`]: pulldown_cmark::Event::Text
    pub alt_src_spans: Vec<Range<usize>>,
}

impl Image {
    // FIXME: string conversion
    pub fn new(uri: &str, options: &CommonMarkOptions) -> Self {
        let has_scheme = uri.contains("://") || uri.starts_with("data:");
        let uri = if options.use_explicit_uri_scheme || has_scheme {
            uri.to_string()
        } else {
            // Assume file scheme
            format!("{}{uri}", options.default_implicit_uri_scheme)
        };

        Self {
            uri,
            alt_text: Vec::new(),
            alt_src_spans: Vec::new(),
        }
    }

    /// Append a piece of alt text, recording its source span so that search
    /// matches can later be mapped back onto it (mirroring [`Link::push_text`]).
    pub fn push_alt_text(&mut self, text: RichText, src_span: Range<usize>) {
        self.alt_text.push(text);
        self.alt_src_spans.push(src_span);
    }

    /// Renders the image.
    ///
    /// Returns `(height, scrolled, match_ys)` where:
    /// - `height` is `0.0` while the texture is still loading (same semantics
    ///   as before; the caller uses it to detect unreliable split-point heights),
    /// - `scrolled` is `true` if the active search match fell on this image and
    ///   the view was scrolled to it (the caller should then clear
    ///   `want_scroll_to_active_match`),
    /// - `match_ys` is a list of `(global_match_index, virtual_y)` pairs for
    ///   every global search range that overlaps any of the image's alt-text
    ///   source spans. The caller should extend `search_match_ys_scratch` with
    ///   these so that `sync_active_match` can locate the image on screen.
    pub fn end(
        self,
        ui: &mut Ui,
        cache: &CommonMarkCache,
        options: &CommonMarkOptions,
        want_scroll_to_active_match: bool,
        content_origin_y: f32,
    ) -> (f32, bool, Vec<(usize, f32)>) {
        let Self {
            uri,
            alt_text,
            alt_src_spans,
        } = self;

        let response = ui.add(
            egui::Image::from_uri(&uri)
                .fit_to_original_size(1.0)
                .max_width(options.max_width(ui)),
        );
        // Save the rect now; `on_hover_ui_at_pointer` consumes `response`.
        let rect = response.rect;

        if !alt_text.is_empty() && options.show_alt_text_on_hover {
            response.on_hover_ui_at_pointer(|ui| {
                for alt in alt_text {
                    ui.label(alt);
                }
            });
        }

        // egui's 24×24 placeholder means height ≥ 1.0 even while Pending, so
        // query the load state directly rather than relying on height alone.
        let is_pending = matches!(
            ui.ctx().try_load_texture(
                &uri,
                egui::TextureOptions::default(),
                egui::load::SizeHint::default(),
            ),
            Ok(egui::load::TexturePoll::Pending { .. })
        );
        let height = if is_pending { 0.0 } else { rect.height() };

        // --- Search match handling ---
        let ranges = cache.search_ranges();
        if ranges.is_empty() || alt_src_spans.is_empty() {
            return (height, false, vec![]);
        }

        let has_active_match = cache.active_search_range().is_some_and(|a| {
            alt_src_spans
                .iter()
                .any(|span| a.start < span.end && a.end > span.start)
        });

        // One `(global_index, virtual_y)` entry per matching search range.
        let virtual_y = rect.min.y - content_origin_y;
        let match_ys: Vec<(usize, f32)> = ranges
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.start < r.end
                    && alt_src_spans
                        .iter()
                        .any(|span| r.start < span.end && r.end > span.start)
            })
            .map(|(i, _)| (i, virtual_y))
            .collect();

        // Draw a colored border around the image for every match that hits it.
        // Active match gets a thicker, more prominent stroke.
        if !match_ys.is_empty() && ui.is_rect_visible(rect) {
            let make_opaque = |c: egui::Color32| egui::Color32::from_rgb(c.r(), c.g(), c.b());
            let (stroke_color, stroke_width): (egui::Color32, f32) = if has_active_match {
                (make_opaque(options.search_active_match_bg(ui)), 2.0)
            } else {
                (make_opaque(options.search_match_bg(ui)), 1.5)
            };
            ui.painter().add(egui::epaint::RectShape::new(
                rect,
                egui::CornerRadius::default(),
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(stroke_width, stroke_color),
                egui::StrokeKind::Outside,
            ));
        }

        // Scroll the active match into view if requested.
        let scrolled = if has_active_match && want_scroll_to_active_match {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            true
        } else {
            false
        };

        (height, scrolled, match_ys)
    }
}

#[derive(Default)]
pub struct CodeBlock {
    pub lang: Option<String>,
    pub content: String,
    /// For each chunk of text appended to `content` (one per markdown text
    /// event), the local byte range within `content` paired with its byte
    /// range in the original source text. Used to translate global search
    /// match ranges into positions local to this code block.
    pub chunks: Vec<(Range<usize>, Range<usize>)>,
}

impl CodeBlock {
    /// Append a chunk of text to the code block's content, recording its
    /// source span so search matches can later be mapped back onto it.
    pub fn push_text(&mut self, text: &str, src_span: Range<usize>) {
        let start = self.content.len();
        self.content.push_str(text);
        self.chunks.push((start..self.content.len(), src_span));
    }

    /// Renders the code block. If `want_scroll_to_active_match` is true and
    /// the currently active search match falls inside this block, the view
    /// is scrolled (centering the match) and `true` is returned so the
    /// caller knows the request has been fulfilled.
    ///
    /// `content_origin_y` is the screen-space Y of the document top (see
    /// [`Link::end`] for details). Returns `(scrolled, match_ys)` where
    /// `match_ys` lists `(global_match_index, virtual_y)` for every search
    /// match in this block so the caller can extend `search_match_ys_scratch`.
    pub fn end(
        &self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
        want_scroll_to_active_match: bool,
        content_origin_y: f32,
    ) -> (bool, Vec<(usize, f32)>) {
        let intervals = crate::search::chunked_search_intervals(
            &self.chunks,
            cache.search_ranges(),
            cache.active_search_range(),
        );
        let scroll_to_active_match = want_scroll_to_active_match
            .then(|| intervals.iter().find(|(_, is_active)| *is_active))
            .flatten()
            .map(|(range, _)| range.clone());
        let did_scroll = scroll_to_active_match.is_some();

        let (galley_pos, galley) = ui
            .scope(|ui| {
                Self::pre_syntax_highlighting(cache, options, ui);

                let mut layout = |ui: &Ui, string: &dyn TextBuffer, wrap_width: f32| {
                    let mut job = if let Some(lang) = &self.lang {
                        self.syntax_highlighting(cache, options, lang, ui, string.as_str())
                    } else {
                        plain_highlighting(ui, string.as_str())
                    };

                    if !intervals.is_empty() {
                        crate::search::apply_search_highlights(
                            &mut job,
                            &intervals,
                            options.search_match_bg(ui),
                            options.search_active_match_bg(ui),
                        );
                    }

                    job.wrap.max_width = wrap_width;
                    ui.fonts_mut(|f| f.layout_job(job))
                };

                crate::elements::code_block(
                    ui,
                    max_width,
                    &self.content,
                    &mut layout,
                    scroll_to_active_match,
                )
            })
            .inner;

        // Record the exact virtual Y of each search match by querying the
        // galley that was just rendered. Unlike the old single block-top Y
        // approach, this handles code blocks taller than one viewport: a
        // match partway down a large block gets the Y of its actual line,
        // not the block top, so the in-viewport check stays correct while
        // the user scrolls through the block.
        let match_ys: Vec<(usize, f32)> = self
            .chunks
            .iter()
            .flat_map(|(local_chunk, src_chunk)| {
                let chunk_text_len = local_chunk.end.saturating_sub(local_chunk.start);
                cache
                    .search_ranges()
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.start < src_chunk.end && r.end > src_chunk.start)
                    .filter_map(|(global_idx, r)| {
                        // Translate source byte range to local range within `content`.
                        let local_start =
                            r.start.saturating_sub(src_chunk.start).min(chunk_text_len)
                                + local_chunk.start;
                        let local_end = r.end.saturating_sub(src_chunk.start).min(chunk_text_len)
                            + local_chunk.start;
                        if local_start >= local_end {
                            return None;
                        }
                        let rect = crate::elements::highlight_rect_for_byte_range(
                            &galley,
                            galley_pos,
                            local_start..local_end,
                        )?;
                        Some((global_idx, rect.min.y - content_origin_y))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        (did_scroll, match_ys)
    }
}

#[cfg(not(feature = "better_syntax_highlighting"))]
impl CodeBlock {
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
        simple_highlighting(ui, text, extension)
    }
}

#[cfg(feature = "better_syntax_highlighting")]
impl CodeBlock {
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

        if let Some(color) = curr_theme.settings.selection_foreground {
            style.visuals.selection.bg_fill = syntect_color_to_egui(color);
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
                            syntect_color_to_egui(front),
                        ),
                    );
                }
            }

            job
        } else {
            simple_highlighting(ui, text, extension)
        }
    }
}

fn simple_highlighting(ui: &Ui, text: &str, extension: &str) -> egui::text::LayoutJob {
    egui_extras::syntax_highlighting::highlight(
        ui.ctx(),
        ui.style(),
        &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()),
        text,
        extension,
    )
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

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct SearchOptions: u8 {
        const CASE_SENSITIVE = 1 << 0;
        const WHOLE_WORD     = 1 << 1;
        const REGEX          = 1 << 2;
    }
}

/// A cache used for storing content such as images.
#[derive(Debug)]
pub struct CommonMarkCache {
    // Everything stored in `CommonMarkCache` must take into account that
    // the cache is for multiple `CommonMarkviewer`s with different source_ids.
    #[cfg(feature = "better_syntax_highlighting")]
    ps: SyntaxSet,

    #[cfg(feature = "better_syntax_highlighting")]
    ts: ThemeSet,

    /// The ID of the heading to scroll to. This is set when a link whose destination is a fragment (e.g. `#my-heading`) has been clicked.
    scroll_to_id_target: Option<String>,
    link_hooks: HashMap<String, bool>,

    scroll: HashMap<egui::Id, ScrollableCache>,
    pub(self) has_installed_loaders: bool,
    /// Keyboard / programmatic scroll delta applied inside the next
    /// `show_scrollable` call and then cleared.
    pub pending_scroll_delta: egui::Vec2,

    /// The alert bundle used to detect alert markers in blockquotes so that
    /// [`update_search_matches`](Self::update_search_matches) can skip them.
    /// Should match the bundle passed to
    /// [`CommonMarkViewer::alerts`](crate::misc::CommonMarkOptions). Defaults
    /// to [`AlertBundle::gfm`], the same default as the viewer.
    pub alerts: AlertBundle,
    /// The search query text
    pub search_query: String,
    /// The search options
    pub search_options: SearchOptions,
    /// Any regex message, e.g. when an escape character is being typed
    pub search_regex_error: Option<String>,
    /// Byte ranges (into the source text passed to the viewer) that should
    /// be highlighted as search matches.
    search_ranges: Vec<Range<usize>>,
    /// The currently active (focused) search match, highlighted more
    /// prominently than the others.
    active_search_range: Option<Range<usize>>,
    /// The ordinal number of the active search match
    active_match: Option<usize>,
    /// The y positions of the search matches, relative to the document
    search_match_virtual_ys: Vec<f32>,
    /// The y position of the top of the last viewport, relative to the document
    last_viewport_virtual_top_y: f32,
    /// The height of the last viewport (clip rect), in the same virtual
    /// coordinate space as `last_viewport_virtual_top_y`. Together they form
    /// the interval `[top, top + height)` that defines what is on screen.
    last_viewport_height: f32,
    /// The byte offset of the last viewport, relative to the document, for
    /// use with `show_scrollable`. Used to detect viewport movement without
    /// relying on input events.
    last_viewport_offset: usize,
    /// Counts down after a search-initiated scroll (`go_to_match` /
    /// `update_search_matches`) to prevent viewport-driven active-match
    /// updates from overriding the scroll animation. Cleared immediately
    /// when the user scrolls manually.
    search_scroll_protection: u32,
    /// Set by [`go_to_match`](Self::go_to_match) to hold `sync_active_match`
    /// from overriding the explicitly chosen match until the user next
    /// scrolls. Unlike `search_scroll_protection` (which counts down and
    /// expires), this stays latched indefinitely so that multiple matches
    /// on the same visual line don't snap back to the first one once the
    /// scroll-protection countdown expires.
    go_to_match_locked: bool,
    /// Set by [`CommonMarkCache::scroll_to_active_search_match`] and cleared
    /// once the render pass that finds and scrolls to the active match runs.
    pending_scroll_to_active_match: bool,
    /// Number of consecutive internal retries (viewport blind-scroll not
    /// yet having brought the match into a rendered slice). Bounds an
    /// otherwise-unlikely non-convergent loop; reset whenever the user
    /// requests a fresh scroll via [`CommonMarkCache::scroll_to_active_search_match`].
    pending_scroll_to_active_match_retries: u8,
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
            scroll: Default::default(),
            scroll_to_id_target: None,
            has_installed_loaders: false,
            pending_scroll_delta: egui::Vec2::ZERO,
            alerts: AlertBundle::gfm(),
            search_query: String::new(),
            search_options: SearchOptions::empty(),
            search_regex_error: None,
            search_ranges: Vec::new(),
            active_search_range: None,
            active_match: None,
            search_match_virtual_ys: Vec::new(),
            last_viewport_virtual_top_y: 0.0,
            last_viewport_height: 0.0,
            last_viewport_offset: 0,
            search_scroll_protection: 0,
            go_to_match_locked: false,
            pending_scroll_to_active_match: false,
            pending_scroll_to_active_match_retries: 0,
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
    pub fn add_syntax_from_str(
        &mut self,
        s: &str,
        fallback_name: Option<&str>,
    ) -> Result<(), syntect::parsing::ParseSyntaxError> {
        let mut builder = self.ps.clone().into_builder();
        SyntaxDefinition::load_from_str(s, true, fallback_name).map(|d| builder.add(d))?;
        self.ps = builder.build();
        Ok(())
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

    /// Get the desired fragment. This is the id which will be scrolled to if it is found in the markdown.
    pub fn scroll_to_id_target(&self) -> Option<&str> {
        self.scroll_to_id_target.as_deref()
    }

    /// Get mutable access to the desired fragment. Setting this will cause the viewer to scroll to the heading with this id if it exists. Setting it to None will prevent scrolling.
    pub fn scroll_to_id_target_mut(&mut self) -> &mut Option<String> {
        &mut self.scroll_to_id_target
    }

    /// Accumulate a scroll delta to be applied inside the next [`show_scrollable`] or
    /// [`apply_pending_scroll_delta`] call and then cleared.
    /// Positive y scrolls toward the top; negative toward the bottom.
    /// Multiple calls before the next frame are summed.
    ///
    /// This is the preferred way to drive keyboard or programmatic scrolling when using
    /// [`show_scrollable`], because the scroll area is owned internally and cannot be
    /// reached directly by the caller.
    ///
    /// [`show_scrollable`]: crate::CommonMarkViewer::show_scrollable
    pub fn set_scroll_delta(&mut self, delta: egui::Vec2) {
        self.pending_scroll_delta += delta;
    }

    /// To apply scrolling without `show_scrollable`, call this function immediately before
    /// or after `show`.
    pub fn apply_pending_scroll_delta(&mut self, ui: &Ui) {
        let delta = std::mem::replace(&mut self.pending_scroll_delta, egui::Vec2::ZERO);
        if delta != egui::Vec2::ZERO {
            ui.scroll_with_delta(delta);
        }
    }

    /// Set the byte ranges (into the source text passed to the viewer) that
    /// should be highlighted as search matches. Ranges must fall on valid
    /// UTF-8 character boundaries.
    ///
    /// This is cheap to call every frame (e.g. while the user is typing into
    /// a search box): highlighting never changes the number of widgets that
    /// get rendered, so it cannot desync egui's widget IDs.
    pub fn set_search_ranges(&mut self, ranges: Vec<Range<usize>>) {
        self.search_ranges = ranges;
    }

    /// The search match ranges currently set for highlighting.
    pub fn search_ranges(&self) -> &[Range<usize>] {
        &self.search_ranges
    }

    /// Approximate the source byte offset of whatever is currently at the
    /// top of the viewport for the given [`show_scrollable`](crate::CommonMarkViewer::show_scrollable)
    /// instance. Useful for implementing "search from the current position"
    /// (like a typical find-in-page: jump to the nearest match at or after
    /// what's currently on screen, instead of always restarting from the top
    /// of the document).
    ///
    /// Returns `None` if nothing has been rendered for `source_id` yet, or
    /// if it was rendered with [`viewport_cache`](crate::CommonMarkViewer::viewport_cache)
    /// disabled (in which case the whole document is visible-ish anyway).
    pub fn viewport_start_byte_offset(&self, source_id: impl egui::AsId) -> Option<usize> {
        let sc = self.scroll.get(&egui::Id::new(source_id))?;
        sc.byte_offset_for_virtual_y(sc.last_viewport_top_y)
    }

    /// Set the active (focused) search match, which is highlighted more
    /// prominently than the other matches. Pass `None` to clear it.
    ///
    /// This does not by itself scroll the view; call
    /// [`scroll_to_active_search_match`](Self::scroll_to_active_search_match)
    /// as well if you want that (typically when the user moves to the next/
    /// previous match).
    pub fn set_active_search_range(&mut self, range: Option<Range<usize>>) {
        self.active_search_range = range;
    }

    /// The currently active (focused) search match, if any.
    pub fn active_search_range(&self) -> Option<&Range<usize>> {
        self.active_search_range.as_ref()
    }

    /// The virtual-y (content-relative, scroll-independent) position of the
    /// top of each search match's rendered rect, updated every frame by
    /// [`show`](crate::CommonMarkViewer::show). Index-parallel to
    /// [`search_ranges`](Self::search_ranges).
    ///
    /// Combined with [`last_viewport_virtual_top_y`](Self::last_viewport_virtual_top_y),
    /// this lets you determine which matches were above, inside, and below
    /// the viewport after the user scrolls — use it to update the active
    /// match in the same way [`scroll.rs`] does via `viewport_start_byte_offset`.
    ///
    /// Values are 0.0 for any match whose containing text run has not yet
    /// been rendered, and are only meaningful for the `show()` path (not
    /// `show_scrollable`).
    pub fn search_match_virtual_ys(&self) -> &[f32] {
        &self.search_match_virtual_ys
    }

    /// The virtual-y of the top of the viewport as recorded by the most
    /// recent [`show`](crate::CommonMarkViewer::show) call (0.0 before the
    /// first call). "Virtual" means content-relative and scroll-independent:
    /// it equals the current scroll offset from the top of the document.
    ///
    /// Compare against [`search_match_virtual_ys`](Self::search_match_virtual_ys)
    /// to find the last match above (or first match at-or-after) the
    /// current scroll position.
    pub fn last_viewport_virtual_top_y(&self) -> f32 {
        self.last_viewport_virtual_top_y
    }

    /// Updates the per-match virtual-y positions and the viewport geometry
    /// recorded during a [`show`](crate::CommonMarkViewer::show) call.
    /// `match_ys` is an iterator of `(match_index, virtual_y)` pairs.
    /// `viewport_height` is the height of the clip rect (same coordinate
    /// space as `viewport_top_y`).
    ///
    /// Used internally by the renderer; read the results via
    /// [`search_match_virtual_ys`](Self::search_match_virtual_ys) and
    /// [`last_viewport_virtual_top_y`](Self::last_viewport_virtual_top_y).
    pub fn update_show_viewport(
        &mut self,
        match_ys: impl IntoIterator<Item = (usize, f32)>,
        viewport_top_y: f32,
        viewport_height: f32,
    ) {
        let n = self.search_ranges.len();
        self.search_match_virtual_ys.clear();
        self.search_match_virtual_ys.resize(n, 0.0);
        for (idx, y) in match_ys {
            if idx < n {
                self.search_match_virtual_ys[idx] = y;
            }
        }
        self.last_viewport_virtual_top_y = viewport_top_y;
        self.last_viewport_height = viewport_height;
    }

    /// The ordinal number of the currently active (focused) search match, if any.
    pub fn active_match(&self) -> Option<usize> {
        self.active_match
    }

    /// Request that the view scroll so that the active search match (set via
    /// [`set_active_search_range`](Self::set_active_search_range)) becomes
    /// visible, centered in the viewport where possible. The request is
    /// consumed by the next render.
    ///
    /// This never forces a full re-render of the document, even in
    /// [`show_scrollable`](crate::CommonMarkViewer::show_scrollable)'s
    /// viewport-cached mode: if the match isn't already in the currently
    /// rendered slice, the view is scrolled toward its approximate position
    /// (using data already collected by the last full render) and refined
    /// precisely over the following frame or two as the real slice comes
    /// into view.
    pub fn scroll_to_active_search_match(&mut self) {
        self.pending_scroll_to_active_match = true;
        self.pending_scroll_to_active_match_retries = 0;
    }

    /// Takes (and clears) the pending scroll-to-active-match request. Used
    /// internally by the renderer.
    pub fn take_pending_scroll_to_active_match(&mut self) -> bool {
        std::mem::take(&mut self.pending_scroll_to_active_match)
    }

    /// Re-arms the pending scroll-to-active-match request for another
    /// attempt (the match wasn't in the slice rendered this frame), unless
    /// the retry budget has been exhausted, in which case the request is
    /// dropped and `false` is returned. Used internally by the renderer to
    /// bound an otherwise-unlikely non-convergent blind-scroll loop.
    pub fn retry_scroll_to_active_match(&mut self) -> bool {
        const MAX_RETRIES: u8 = 8;
        if self.pending_scroll_to_active_match_retries >= MAX_RETRIES {
            self.pending_scroll_to_active_match = false;
            return false;
        }
        self.pending_scroll_to_active_match_retries += 1;
        self.pending_scroll_to_active_match = true;
        true
    }

    /// Recomputes `search_ranges` from the *rendered* text only (via
    /// pulldown-cmark's `Text`/`Code` events), so link destinations,
    /// heading `{#id}` attribute syntax, and other non-visible markdown
    /// syntax are never matched (a naive substring search over the raw
    /// source would, for example, double-count "500" in
    /// `[Section 500](#section-500)`: once in the visible text, once in the
    /// URL).
    ///
    /// Recomputes `search_ranges` on every keystroke and immediately
    /// advances to the nearest match at or after wherever the user is
    /// currently scrolled to (wrapping to the first match if there is none
    /// after that point), mirroring how a normal "find in page" behaves.
    /// Recomputation and the resulting scroll are both cheap (see
    /// `CommonMarkCache::scroll_to_active_search_match`'s docs: this never
    /// forces a full document re-render), so this should stay responsive.
    ///
    /// Anchoring to the current viewport position requires the viewer to be
    /// shown via [`show_with_id`](crate::CommonMarkViewer::show_with_id) or
    /// [`show_scrollable`](crate::CommonMarkViewer::show_scrollable) with the
    /// same `egui_source_id`. When using plain
    /// [`show`](crate::CommonMarkViewer::show), the search always starts from
    /// the document top.
    pub fn update_search_matches(&mut self, egui_source_id: &str, content: &str) {
        // Anchor to the byte position of the currently active match so that
        // adding/removing characters from the query stays on the same spot.
        // Fall back to the viewport position for a fresh (no active match)
        // search. Using viewport_start here on a query change would jump
        // backwards whenever the viewport centre is a couple of sections
        // before the active match (i.e. the match is centred on screen).
        let anchor = self
            .active_match
            .and_then(|i| self.search_ranges.get(i))
            .map(|r| r.start)
            .or_else(|| self.viewport_start_byte_offset(egui_source_id))
            .unwrap_or(0);

        self.search_ranges.clear();

        let query = &self.search_query;

        if query.is_empty() {
            return;
        }

        let options = self.search_options;

        let mut pattern = if options.contains(SearchOptions::REGEX) {
            query.clone()
        } else {
            regex::escape(query)
        };

        if options.contains(SearchOptions::WHOLE_WORD) {
            pattern = format!(r"\b(?:{pattern})\b");
        }

        let regex = match regex::RegexBuilder::new(&pattern)
            .case_insensitive(!options.contains(SearchOptions::CASE_SENSITIVE))
            .build()
        {
            Ok(regex) => {
                self.search_regex_error = None;
                regex
            }
            Err(err) => {
                self.search_regex_error = Some(err.to_string());
                return;
            }
        };

        // Mirror the options CommonMarkViewer itself parses with,
        // including heading attributes since `enable_scroll_to_heading`
        // is set below (otherwise `{#section-500}` would remain in the
        // heading's Text event and get matched too).
        let options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS
            | pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_FOOTNOTES
            | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;

        let parser = pulldown_cmark::Parser::new_ext(content, options).into_offset_iter();

        // ── Cross-style inline run ────────────────────────────────────────────
        //
        // Consecutive Text and Code events within the same inline context are
        // concatenated into a single "run" string and searched as a unit. This
        // lets a query like "book example" match across a style boundary such as
        // "`book` example" (inline code followed by normal text).
        //
        // SoftBreaks are folded in as a space so that "foo bar" also matches
        // across a soft line-wrap. Any other event (paragraph break, heading,
        // hard break, …) ends the run.
        //
        // For each regex match in the combined string we reconstruct a source
        // byte range using the segment list. Each segment records the start of
        // its text in `run_text` (combined_offset) and the pulldown event's
        // source span. The range formula is the same as for single events:
        //
        //   src_start = seg_at_cstart.src_span.start + (cstart - seg_at_cstart.combined_offset)
        //   src_end   = seg_at_cend  .src_span.start + (cend   - seg_at_cend  .combined_offset)
        //
        // Because the renderer's search_intervals() clips any range to the
        // current event's src_span, a single cross-event range is all that is
        // needed: each label highlights only the portion of the match that falls
        // within it, with no renderer changes required.
        let mut run_text = String::new();
        // (combined_offset, src_span) for each Text/Code event in the run.
        let mut run_segs: Vec<(usize, Range<usize>)> = Vec::new();

        // ── Alert-keyword state ───────────────────────────────────────────────
        //
        // parse_alerts() strips known leading alert markers (e.g. "[!NOTE]")
        // from blockquotes so those bytes are never rendered. We buffer the
        // leading text event of each blockquote first paragraph and, after the
        // first break, either discard it (confirmed alert keyword — see
        // decide_alert_matches!) or retroactively add its matches.
        //
        // bq_stack: one bool per nested blockquote level; `true` once the
        // first-paragraph header decision has been made at that level.
        let mut bq_stack: Vec<bool> = Vec::new();
        let mut in_bq_first_run = false;
        let mut pending_buf: Vec<(String, Range<usize>)> = Vec::new();

        // ── Macros ───────────────────────────────────────────────────────────

        // Flush the current inline run: search the combined text and emit one
        // source range per match, potentially spanning multiple events.
        macro_rules! flush_run {
            () => {
                if !run_segs.is_empty() {
                    for m in regex.find_iter(&run_text) {
                        let cstart = m.start();
                        let cend = m.end();
                        // Segment containing the first byte of the match.
                        let si = run_segs
                            .partition_point(|(off, _)| *off <= cstart)
                            .saturating_sub(1);
                        let src_start = run_segs[si].1.start + (cstart - run_segs[si].0);
                        // Segment containing the last byte of the match (cend-1).
                        let ei = run_segs
                            .partition_point(|(off, _)| *off < cend)
                            .saturating_sub(1);
                        let src_end = run_segs[ei].1.start + (cend - run_segs[ei].0);
                        self.search_ranges.push(src_start..src_end);
                    }
                    run_text.clear();
                    run_segs.clear();
                }
            };
        }

        // Flush pending_buf as plain single-event matches (not an alert keyword).
        macro_rules! flush_pending {
            () => {
                for (text, r) in pending_buf.drain(..) {
                    for m in regex.find_iter(&text) {
                        self.search_ranges
                            .push(r.start + m.start()..r.start + m.end());
                    }
                }
            };
        }

        // Decide what to emit for the buffered blockquote first-paragraph text.
        // If it is a known alert keyword the viewer renders `identifier_rendered`
        // (e.g. "Note") rather than the raw source text, so we match the regex
        // against the rendered form and store the keyword's source span as the
        // range anchor (the blockquote renderer checks for overlap and highlights
        // the title label). Otherwise fall back to normal single-event matching.
        macro_rules! decide_alert_matches {
            () => {{
                let ident: String = pending_buf.iter().map(|(t, _)| t.as_str()).collect();
                let alert_title: Option<String> =
                    try_get_alert(&self.alerts, &ident).map(|a| a.identifier_rendered.clone());
                if let Some(rendered) = alert_title {
                    let src_start = pending_buf.iter().map(|(_, r)| r.start).min().unwrap_or(0);
                    let src_end = pending_buf.iter().map(|(_, r)| r.end).max().unwrap_or(0);
                    let alert_src_range = src_start..src_end;
                    for _ in regex.find_iter(&rendered) {
                        self.search_ranges.push(alert_src_range.clone());
                    }
                    pending_buf.clear();
                } else {
                    flush_pending!();
                }
            }};
        }

        // ── Main loop ────────────────────────────────────────────────────────

        for (event, range) in parser {
            // Blockquote alert state machine (borrows event, does not consume).
            match &event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::BlockQuote(_)) => {
                    flush_run!();
                    flush_pending!();
                    bq_stack.push(false);
                    in_bq_first_run = false;
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::BlockQuote(_)) => {
                    flush_run!();
                    flush_pending!();
                    in_bq_first_run = false;
                    bq_stack.pop();
                }
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph)
                    if bq_stack.last() == Some(&false) =>
                {
                    in_bq_first_run = true;
                    pending_buf.clear();
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph)
                    if in_bq_first_run =>
                {
                    decide_alert_matches!();
                    in_bq_first_run = false;
                    if let Some(seen) = bq_stack.last_mut() {
                        *seen = true;
                    }
                }
                pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak
                    if in_bq_first_run =>
                {
                    decide_alert_matches!();
                    in_bq_first_run = false;
                    if let Some(seen) = bq_stack.last_mut() {
                        *seen = true;
                    }
                    // The break itself is not rendered content; skip event processing.
                    continue;
                }
                _ => {}
            }

            // Inline-run management (consumes event).
            match event {
                pulldown_cmark::Event::Text(text) | pulldown_cmark::Event::Code(text) => {
                    if in_bq_first_run {
                        // Buffer until we know whether this is an alert keyword.
                        pending_buf.push((text.to_string(), range));
                    } else {
                        let off = run_text.len();
                        run_text.push_str(&text);
                        run_segs.push((off, range));
                    }
                }
                pulldown_cmark::Event::SoftBreak => {
                    // A soft break renders as a space; fold it into the run so
                    // that "foo bar" matches across a soft line-wrap.
                    if !in_bq_first_run {
                        run_text.push(' ');
                    }
                }
                // Inline formatting markers carry no text of their own but do
                // not break the visual line. Treat them as transparent so that
                // a query like "with syntect" matches across a link boundary
                // (e.g. "with [`syntect`](url)") or emphasis markers.
                pulldown_cmark::Event::Start(
                    pulldown_cmark::Tag::Emphasis
                    | pulldown_cmark::Tag::Strong
                    | pulldown_cmark::Tag::Strikethrough
                    | pulldown_cmark::Tag::Link { .. },
                )
                | pulldown_cmark::Event::End(
                    pulldown_cmark::TagEnd::Emphasis
                    | pulldown_cmark::TagEnd::Strong
                    | pulldown_cmark::TagEnd::Strikethrough
                    | pulldown_cmark::TagEnd::Link,
                ) => {}
                _ => {
                    // Any other event ends the current inline run.
                    flush_run!();
                }
            }
        }

        // End of document: flush whatever is still open.
        flush_run!();
        decide_alert_matches!();

        if self.search_ranges.is_empty() {
            self.active_match = None;
            self.sync_active_search_range();
            return;
        }

        let nearest = self
            .search_ranges
            .iter()
            .position(|r| r.start >= anchor)
            .unwrap_or(0);
        self.active_match = Some(nearest);
        self.sync_active_search_range();
        self.scroll_to_active_search_match();
        // Suppress viewport-sync for ~30 frames so the animation toward the
        // new match is not immediately overridden by the centering drift
        // (viewport start lands before the active match when centred on screen).
        self.search_scroll_protection = 30;
    }

    /// Synchronises the active search match to the current scroll position
    /// for documents displayed with [`show`](crate::CommonMarkViewer::show).
    ///
    /// Call this once per frame immediately after `show()` returns, passing
    /// `user_scrolled = true` whenever the frame received explicit user
    /// scroll input (mouse wheel, keyboard arrow/page keys, etc.).
    ///
    /// When `user_scrolled` is `true`, any ongoing post-search scroll
    /// protection is cancelled and the active match re-anchors immediately
    /// to the top of the viewport. When `false`, the protection counter
    /// winds down automatically over the following ~30 frames (enough for
    /// a typical scroll-to-match animation to settle), after which normal
    /// syncing resumes.
    ///
    /// The active match is set to the first match at or after the viewport
    /// top, so that pressing Next from the current scroll position advances
    /// to the next unseen match. `scroll_to_active_search_match` is
    /// deliberately *not* called: the viewport is already where the user
    /// put it.
    ///
    /// For documents displayed with
    /// [`show_scrollable`](crate::CommonMarkViewer::show_scrollable),
    /// use [`sync_scrollable_active_match`](Self::sync_scrollable_active_match)
    /// instead (see the `scroll` example).
    pub fn sync_active_match(&mut self, user_scrolled: bool) {
        if user_scrolled {
            self.search_scroll_protection = 0;
            // The user scrolled manually, so it's safe to re-anchor
            // active_match to the viewport again.
            self.go_to_match_locked = false;
        }
        if self.search_scroll_protection > 0 {
            self.search_scroll_protection -= 1;
            return;
        }
        // go_to_match explicitly chose a match; don't override it until the
        // user scrolls. The 30-frame countdown above protects against drift
        // during the scroll animation, but it can expire while the viewport
        // is still centred on the same row as the chosen match — causing an
        // unwanted snap back to the first match on that row. This flag keeps
        // the lock alive past the countdown.
        if self.go_to_match_locked {
            return;
        }
        if self.search_ranges.is_empty() || self.search_match_virtual_ys.is_empty() {
            return;
        }

        let vt = self.last_viewport_virtual_top_y;
        let len = self.search_match_virtual_ys.len();
        let nearest = self
            .search_match_virtual_ys
            .iter()
            .position(|&y| y >= vt)
            .unwrap_or(len - 1);
        if self.active_match != Some(nearest) {
            // Only move away from the active match if it has actually scrolled
            // out of the viewport. While it is still on screen the user is
            // just panning around within the same view, and we should honour
            // their explicit Next/Prev choice rather than snapping to the
            // first match visible at the viewport top.
            let active_y = self
                .active_match
                .and_then(|i| self.search_match_virtual_ys.get(i).copied());
            let in_viewport =
                active_y.is_some_and(|y| y >= vt && y < vt + self.last_viewport_height);
            if !in_viewport {
                self.active_match = Some(nearest);
                self.sync_active_search_range();
                // Do NOT call scroll_to_active_search_match: the viewport is
                // already where the user put it.
            }
        }
    }

    /// After `show_scrollable` the viewer has applied any pending scroll
    /// delta and updated the byte-offset tracker.  Sync `active_match`
    /// whenever the viewport byte offset actually changed AND no search
    /// scroll is still animating.  This fires on every animation frame
    /// (not just the key-press frame), so even large `PageDown` jumps
    /// settle to the correct match once the animation completes.
    pub fn sync_scrollable_active_match(
        &mut self,
        egui_source_id: &str,
        viewport_cache: bool,
        user_scrolled: bool,
    ) {
        if !viewport_cache {
            self.sync_active_match(user_scrolled);
            return;
        }

        if user_scrolled {
            self.search_scroll_protection = 0;
            self.go_to_match_locked = false;
        }

        let current_offset = self.viewport_start_byte_offset(egui_source_id).unwrap_or(0);

        if !self.search_ranges.is_empty()
            && self.search_scroll_protection == 0
            && current_offset != self.last_viewport_offset
        {
            let len = self.search_ranges.len();
            let idx = self
                .search_ranges
                .partition_point(|r| r.start < current_offset);
            let nearest = if idx > 0 {
                idx - 1
            } else {
                len.saturating_sub(1)
            };

            if self.active_match != Some(nearest) {
                // Only move away from the active match if it has scrolled out
                // of the viewport. search_match_virtual_ys is now populated
                // by the viewport-cache slice render (via update_show_viewport)
                // so we can use the same check as sync_active_match: matches
                // in the rendered slice have their exact pixel Y; those outside
                // default to 0.0 and are treated as not-in-viewport.
                let active_y = self
                    .active_match
                    .and_then(|i| self.search_match_virtual_ys.get(i).copied());
                let vt = self.last_viewport_virtual_top_y;
                let in_viewport =
                    active_y.is_some_and(|y| y >= vt && y < vt + self.last_viewport_height);

                if !in_viewport {
                    self.active_match = Some(nearest);
                    self.sync_active_search_range();
                    // Do NOT call scroll_to_active_search_match here: the
                    // viewport is already where the user put it.
                }
            }
        }

        self.last_viewport_offset = current_offset;
        if self.search_scroll_protection > 0 {
            self.search_scroll_protection -= 1;
        }
    }

    // Update the active search range to the desired ordinal value
    fn sync_active_search_range(&mut self) {
        self.set_active_search_range(
            self.active_match
                .and_then(|i| self.search_ranges.get(i))
                .cloned(),
        );
    }

    /// Scroll back or forward `delta` matches (according to the sign of `delta`) if applicable
    pub fn go_to_match(&mut self, delta: isize) {
        if self.search_ranges.is_empty() {
            return;
        }
        let len = self.search_ranges.len().cast_signed();
        let next = match self.active_match {
            Some(i) => (i.cast_signed() + delta).rem_euclid(len),
            // First navigation after a fresh search: start at the first
            // match for Next, the last one for Previous.
            None if delta >= 0 => 0,
            None => len.saturating_sub(1),
        };
        self.active_match = Some(next.cast_unsigned());
        self.sync_active_search_range();
        self.scroll_to_active_search_match();
        self.search_scroll_protection = 30;
        // Hold the lock until the user scrolls, so that sync_active_match
        // cannot revert to the first match on the same visual row once the
        // 30-frame countdown expires.
        self.go_to_match_locked = true;
    }

    /// Clear the cache for all scrollable elements
    pub fn clear_scrollable(&mut self) {
        self.scroll.clear();
    }

    /// Clear the cache for a specific scrollable viewer. Returns false if the
    /// id was not in the cache.
    pub fn clear_scrollable_with_id(&mut self, source_id: impl egui::AsId) -> bool {
        self.scroll.remove(&egui::Id::new(source_id)).is_some()
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
    /// ctx.output_mut(|o| for command in &o.commands {
    ///     matches!(command, egui::OutputCommand::OpenUrl(_));
    /// });
    /// # });
    /// ```
    ///
    /// The main difference is that link hooks allows egui_commonmark to check for link hooks
    /// while rendering. Normally when hovering over a link, egui_commonmark will display the full
    /// url. With link hooks this feature is disabled, but to do that all hooks must be known.
    // Works when displayed through egui_commonmark
    #[allow(rustdoc::broken_intra_doc_links)]
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

    /// Handles keyboard scrolling input and updates cache delta.
    /// Returns `true` if any explicit user scrolling (wheel or keyboard) occurred.
    pub fn handle_keyboard_scrolling(&mut self, ui: &egui::Ui) -> bool {
        let no_text_focus = !ui.ctx().egui_wants_keyboard_input();

        // Calculate line and page heights up front
        let line_h = ui.text_style_height(&egui::TextStyle::Body);
        let page_h = ui.available_height();

        // Map key inputs directly to vertical scroll offsets (f32)
        let key_scroll_delta = no_text_focus
            .then(|| {
                ui.ctx().input(|i| {
                    use egui::Key;
                    if i.key_pressed(Key::Home)
                        || (i.modifiers.command && i.key_pressed(Key::ArrowUp))
                    {
                        Some(f32::MAX / 2.0)
                    } else if i.key_pressed(Key::End)
                        || (i.modifiers.command && i.key_pressed(Key::ArrowDown))
                    {
                        Some(-f32::MAX / 2.0)
                    } else if i.key_pressed(Key::PageUp) {
                        Some(page_h)
                    } else if i.key_pressed(Key::PageDown) {
                        Some(-page_h)
                    } else if !i.modifiers.command && i.key_pressed(Key::ArrowUp) {
                        Some(line_h)
                    } else if !i.modifiers.command && i.key_pressed(Key::ArrowDown) {
                        Some(-line_h)
                    } else {
                        None
                    }
                })
            })
            .flatten();

        // Apply scroll delta if a key was pressed
        if let Some(delta_y) = key_scroll_delta {
            self.set_scroll_delta(egui::vec2(0.0, delta_y));
        }

        let user_scroll_input =
            ui.input(egui::InputState::is_scrolling) || key_scroll_delta.is_some();

        if user_scroll_input {
            self.search_scroll_protection = 0;
        }

        // Return combined user scroll status
        user_scroll_input
    }
}

pub fn scroll_cache<'a>(cache: &'a mut CommonMarkCache, id: &egui::Id) -> &'a mut ScrollableCache {
    if !cache.scroll.contains_key(id) {
        cache.scroll.insert(*id, Default::default());
    }
    cache.scroll.get_mut(id).unwrap()
}

/// Should be called before any rendering
pub fn prepare_show(cache: &mut CommonMarkCache, ctx: &egui::Context) {
    if !cache.has_installed_loaders {
        // Even though the install function can be called multiple times, its not the cheapest
        // so we ensure that we only call it once.
        // This could be done at the creation of the cache, however it is better to keep the
        // cache free from egui's Ui and Context types as this allows it to be created before
        // any egui instances. It also keeps the API similar to before the introduction of the
        // image loaders.
        #[cfg(feature = "embedded_image")]
        crate::data_url_loader::install_loader(ctx);

        egui_extras::install_image_loaders(ctx);
        cache.has_installed_loaders = true;
    }

    cache.deactivate_link_hooks();
}
