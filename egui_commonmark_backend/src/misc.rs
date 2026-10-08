use crate::alerts::AlertBundle;
#[cfg(feature = "regex")]
use crate::{alerts::try_get_alert, search};
#[cfg(feature = "regex")]
use bitflags::bitflags;
use egui::{Id, RichText, TextBuffer, TextStyle, Ui, text::LayoutJob};
#[cfg(feature = "regex")]
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;

use crate::pulldown::ViewerCache;

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
    /// Background colour for passive search matches. When `None`, a
    /// theme-derived default is used (see [`crate::search::default_match_bg`]).
    #[cfg(feature = "regex")]
    pub search_match_bg: Option<egui::Color32>,
    /// Background colour for the active (focused) search match. When
    /// `None`, a theme-derived default is used (see
    /// [`crate::search::default_active_match_bg`]).
    #[cfg(feature = "regex")]
    pub search_active_match_bg: Option<egui::Color32>,
    /// When set via [`show_with_id`](crate::CommonMarkViewer::show_with_id),
    /// `full_render` records block-boundary positions (split points) under
    /// this id so that
    /// [`viewport_start_byte_offset`](CommonMarkCache::viewport_start_byte_offset)
    /// works and
    /// [`update_search_matches`](CommonMarkCache::update_search_matches) can
    /// anchor new searches to the current viewport position.
    /// Not used by `show_scrollable`, which carries its own source id.
    pub source_id: Option<Id>,
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
            .field("source_id", &self.source_id);

        #[cfg(feature = "regex")]
        s.field("search_match_bg", &self.search_match_bg)
            .field("search_active_match_bg", &self.search_active_match_bg);

        s.finish()
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
            #[cfg(feature = "regex")]
            search_match_bg: None,
            #[cfg(feature = "regex")]
            search_active_match_bg: None,
            source_id: Some(Id::NULL),
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
    #[cfg(feature = "regex")]
    pub fn search_match_bg(&self, ui: &Ui) -> egui::Color32 {
        self.search_match_bg
            .unwrap_or_else(|| search::default_match_bg(ui.visuals()))
    }

    /// The background colour to use for the active search match: the
    /// explicit override if one was set, otherwise a theme-derived default.
    #[cfg(feature = "regex")]
    pub fn search_active_match_bg(&self, ui: &Ui) -> egui::Color32 {
        self.search_active_match_bg
            .unwrap_or_else(|| search::default_active_match_bg(ui.visuals()))
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

        #[cfg(feature = "regex")]
        let (intervals, has_active_match, search_ranges_snapshot) = {
            let id = options.source_id.unwrap_or(Id::NULL);
            let sc = cache.search_cache_mut(&id);
            let ranges = sc.search_ranges();
            let (intervals, has_active_match) = if ranges.is_empty() {
                (vec![], false)
            } else {
                let iv =
                    search::chunked_search_intervals(&chunks, ranges, sc.active_search_range());
                let active = iv.iter().any(|(_, is_active)| *is_active);
                (iv, active)
            };
            let snapshot: Vec<Range<usize>> = sc.search_ranges().to_vec();
            (intervals, has_active_match, snapshot)
        };
        // Without the regex feature there is no search state; intervals is only
        // used inside regex-gated blocks so we only bind the two variables that
        // the non-search render path actually reads.
        #[cfg(not(feature = "regex"))]
        let (has_active_match, search_ranges_snapshot): (bool, Vec<Range<usize>>) = (false, vec![]);

        let mut layout_job = LayoutJob::default();
        for t in text {
            t.append_to(
                &mut layout_job,
                ui.style(),
                egui::FontSelection::Default,
                egui::Align::LEFT,
            );
        }
        #[cfg(feature = "regex")]
        if !intervals.is_empty() {
            search::apply_search_highlights(
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
            }
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
        let match_ys: Vec<(usize, f32)> = search_ranges_snapshot
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
    #[cfg_attr(
        not(feature = "regex"),
        expect(unused_variables), // cache, want_scroll_to_active_match, content_origin_y,
                                  // alt_src_spans only used in the regex-gated search section
    )]
    pub fn end(
        self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
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

        // --- Search match highlighting (regex feature only) ---
        #[cfg(feature = "regex")]
        {
            let source_id = options.source_id.unwrap_or(Id::NULL);
            let vc = viewer_cache(cache, &source_id);
            let search_cache = &mut vc.search_cache;

            let ranges = search_cache.search_ranges();
            if !ranges.is_empty() && !alt_src_spans.is_empty() {
                let has_active_match = search_cache.active_search_range().is_some_and(|a| {
                    alt_src_spans
                        .iter()
                        .any(|span| a.start < span.end && a.end > span.start)
                });

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

                if !match_ys.is_empty() && ui.is_rect_visible(rect) {
                    let make_opaque =
                        |c: egui::Color32| egui::Color32::from_rgb(c.r(), c.g(), c.b());
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

                let scrolled = if has_active_match && want_scroll_to_active_match {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                    true
                } else {
                    false
                };

                return (height, scrolled, match_ys);
            }
        }

        (height, false, vec![])
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
    #[cfg_attr(
        not(feature = "regex"),
        allow(unused_variables), // content_origin_y, galley_pos, galley only used by
                                  // the regex-gated match-Y tracking block
    )]
    pub fn end(
        &self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
        want_scroll_to_active_match: bool,
        content_origin_y: f32,
    ) -> (bool, Vec<(usize, f32)>) {
        #[cfg(feature = "regex")]
        let intervals = {
            let vc = viewer_cache(cache, &options.source_id.unwrap_or(egui::Id::NULL));
            let search_cache = &vc.search_cache;
            search::chunked_search_intervals(
                &self.chunks,
                search_cache.search_ranges(),
                search_cache.active_search_range(),
            )
        };
        #[cfg(not(feature = "regex"))]
        let intervals: Vec<(std::ops::Range<usize>, bool)> = vec![];

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

                    #[cfg(feature = "regex")]
                    if !intervals.is_empty() {
                        search::apply_search_highlights(
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
        // galley that was just rendered, so that we can scroll to the exact
        // line of any active match within the block.
        #[cfg(feature = "regex")]
        let match_ys: Vec<(usize, f32)> = {
            let search_cache =
                &viewer_cache(cache, &options.source_id.unwrap_or(egui::Id::NULL)).search_cache;
            self.chunks
                .iter()
                .flat_map(|(local_chunk, src_chunk)| {
                    let chunk_text_len = local_chunk.end.saturating_sub(local_chunk.start);
                    search_cache
                        .search_ranges()
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| r.start < src_chunk.end && r.end > src_chunk.start)
                        .filter_map(|(global_idx, r)| {
                            let local_start =
                                r.start.saturating_sub(src_chunk.start).min(chunk_text_len)
                                    + local_chunk.start;
                            let local_end =
                                r.end.saturating_sub(src_chunk.start).min(chunk_text_len)
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
                .collect()
        };
        #[cfg(not(feature = "regex"))]
        let match_ys: Vec<(usize, f32)> = vec![];

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
            .unwrap_or_else(|| style.visuals.extreme_bg_color);

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
        let syntax = cache
            .ps
            .find_syntax_by_token(extension)
            .unwrap_or_else(|| cache.ps.find_syntax_plain_text());

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
    }
}

#[cfg(not(feature = "better_syntax_highlighting"))]
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

#[cfg(feature = "regex")]
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

    link_hooks: HashMap<String, bool>,
    viewers: HashMap<Id, ViewerCache>,
    pub(self) has_installed_loaders: bool,
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
            viewers: HashMap::default(),
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

    /// Clear the cache for all viewers.
    pub fn clear_viewers(&mut self) {
        self.viewers.clear();
    }

    /// Clear the cache for a specific viewer. Returns false if the
    /// id was not in the cache.
    pub fn clear_viewer(&mut self, id: &Id) -> bool {
        self.viewers.remove(id).is_some()
    }

    /// Programmatically scroll the viewer identified by `id` to the heading
    /// with the given anchor slug on the next rendered frame. The slug must
    /// match a heading defined with the `{#my-anchor}` attribute syntax.
    ///
    /// Pass `None` to cancel a pending scroll.
    ///
    /// This is useful when driving navigation from outside the viewer — for
    /// example, clicking an entry in a separate table-of-contents widget.
    pub fn scroll_to_heading(&mut self, id: &Id, slug: Option<String>) {
        *viewer_cache(self, id).scroll_to_id_target_mut() = slug;
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
    pub fn handle_keyboard_scrolling(&mut self, id: &Id, ui: &egui::Ui) -> bool {
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

        let vc = viewer_cache(self, id);

        // Apply scroll delta if a key was pressed
        if let Some(delta_y) = key_scroll_delta {
            vc.set_scroll_delta(egui::vec2(0.0, delta_y));
        }

        let user_scroll_input =
            ui.input(egui::InputState::is_scrolling) || key_scroll_delta.is_some();

        #[cfg(feature = "regex")]
        {
            if user_scroll_input {
                vc.search_cache.search_scroll_protection = 0;
            }
        }

        // Return combined user scroll status
        user_scroll_input
    }

    /// To apply scrolling without `show_scrollable`, call this function immediately before
    /// or after `show`.
    pub fn apply_pending_scroll_delta(&mut self, id: &Id, ui: &Ui) {
        let vc = self.viewers.get_mut(id).unwrap();
        let delta = std::mem::replace(&mut vc.pending_scroll_delta, egui::Vec2::ZERO);
        if delta != egui::Vec2::ZERO {
            ui.scroll_with_delta(delta);
        }
    }

    #[cfg(feature = "regex")]
    pub fn search_regex_error(&mut self, id: &Id) -> Option<String> {
        viewer_cache(self, id)
            .search_cache
            .search_regex_error
            .clone()
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
    #[cfg(feature = "regex")]
    pub fn update_search_matches(&mut self, id: &Id, content: &str) {
        let vc = viewer_cache(self, id);
        let search_cache = &mut vc.search_cache;

        // Anchor to the byte position of the currently active match so that
        // adding/removing characters from the query stays on the same spot.
        // Fall back to the viewport position for a fresh (no active match)
        // search. Using viewport_start here on a query change would jump
        // backwards whenever the viewport centre is a couple of sections
        // before the active match (i.e. the match is centred on screen).
        let anchor = search_cache
            .active_match
            .and_then(|i| search_cache.search_ranges.get(i))
            .map(|r| r.start)
            .or_else(|| search_cache.viewport_start_byte_offset(&vc.split_points))
            .unwrap_or(0);

        search_cache.search_ranges.clear();

        let Some(regex) = build_search_regex(search_cache) else {
            return;
        };

        // Mirror the options CommonMarkViewer itself parses with,
        // including heading attributes since `enable_scroll_to_heading`
        // is set below (otherwise `{#section-500}` would remain in the
        // heading's Text event and get matched too).
        let options =
            crate::pulldown::parser_options() | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;

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

        // ── Main loop ────────────────────────────────────────────────────────
        for (event, range) in parser {
            // Blockquote alert state machine (borrows event, does not consume).
            match &event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::BlockQuote(_)) => {
                    flush_run(search_cache, &regex, &mut run_text, &mut run_segs);
                    flush_pending(search_cache, &regex, &mut pending_buf);
                    bq_stack.push(false);
                    in_bq_first_run = false;
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::BlockQuote(_)) => {
                    flush_run(search_cache, &regex, &mut run_text, &mut run_segs);
                    flush_pending(search_cache, &regex, &mut pending_buf);
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
                    decide_alert_matches(search_cache, &regex, &mut pending_buf);
                    in_bq_first_run = false;
                    if let Some(seen) = bq_stack.last_mut() {
                        *seen = true;
                    }
                }
                pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak
                    if in_bq_first_run =>
                {
                    decide_alert_matches(search_cache, &regex, &mut pending_buf);
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
            decide_search_match_continuity(
                search_cache,
                &regex,
                &mut run_text,
                &mut run_segs,
                in_bq_first_run,
                &mut pending_buf,
                event,
                range,
            );
        }

        // End of document: flush whatever is still open.

        flush_run(search_cache, &regex, &mut run_text, &mut run_segs);
        decide_alert_matches(search_cache, &regex, &mut pending_buf);
        if search_cache.search_ranges.is_empty() {
            search_cache.active_match = None;
            search_cache.sync_active_search_range();
            return;
        }

        let nearest = search_cache
            .search_ranges
            .iter()
            .position(|r| r.start >= anchor)
            .unwrap_or(0);
        search_cache.active_match = Some(nearest);
        search_cache.sync_active_search_range();
        search_cache.scroll_to_active_search_match();
        // Suppress viewport-sync for ~30 frames so the animation toward the
        // new match is not immediately overridden by the centering drift
        // (viewport start lands before the active match when centred on screen).
        search_cache.search_scroll_protection = 30;
    }

    #[cfg(feature = "regex")]
    pub fn search_cache_mut(&mut self, id: &Id) -> &mut crate::pulldown::SearchCache {
        &mut viewer_cache(self, id).search_cache
    }

    /// The current set of search-match byte ranges for this viewer, or an
    /// empty slice if no search has been run yet.
    #[cfg(feature = "regex")]
    pub fn search_ranges(&self, id: &Id) -> &[Range<usize>] {
        self.viewers
            .get(id)
            .map(|vc| vc.search_cache.search_ranges())
            .unwrap_or_default()
    }

    /// The zero-based ordinal of the currently active (focused) search match,
    /// or `None` if there is no active match.
    #[cfg(feature = "regex")]
    pub fn active_match(&self, id: &Id) -> Option<usize> {
        self.viewers.get(id)?.search_cache.active_match()
    }

    /// Advance the active match by `delta` steps (negative = backwards),
    /// wrapping around. Does nothing if there are no matches.
    #[cfg(feature = "regex")]
    pub fn go_to_match(&mut self, id: &Id, delta: isize) {
        viewer_cache(self, id).search_cache.go_to_match(delta);
    }

    /// Mutable access to the search query string for this viewer, suitable
    /// for binding directly to a [`egui::TextEdit`].
    #[cfg(feature = "regex")]
    pub fn search_query_mut(&mut self, id: &Id) -> &mut String {
        &mut viewer_cache(self, id).search_cache.search_query
    }

    /// Mutable access to the search options bitflags for this viewer.
    #[cfg(feature = "regex")]
    pub fn search_options_mut(&mut self, id: &Id) -> &mut SearchOptions {
        &mut viewer_cache(self, id).search_cache.search_options
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
    #[cfg(feature = "regex")]
    pub fn sync_active_match(&mut self, id: &Id, user_scrolled: bool) {
        let search_cache = &mut viewer_cache(self, id).search_cache;

        if user_scrolled {
            search_cache.search_scroll_protection = 0;
            // The user scrolled manually, so it's safe to re-anchor
            // active_match to the viewport again.
            search_cache.go_to_match_locked = false;
        }
        if search_cache.search_scroll_protection > 0 {
            search_cache.search_scroll_protection -= 1;
            return;
        }
        // go_to_match explicitly chose a match; don't override it until the
        // user scrolls. The 30-frame countdown above protects against drift
        // during the scroll animation, but it can expire while the viewport
        // is still centred on the same row as the chosen match — causing an
        // unwanted snap back to the first match on that row. This flag keeps
        // the lock alive past the countdown.
        if search_cache.go_to_match_locked {
            return;
        }
        if search_cache.search_ranges.is_empty() || search_cache.search_match_virtual_ys.is_empty()
        {
            return;
        }

        let vt = search_cache.last_viewport_top_y;
        let len = search_cache.search_match_virtual_ys.len();
        let nearest = search_cache
            .search_match_virtual_ys
            .iter()
            .position(|&y| y >= vt)
            .unwrap_or(len - 1);
        if search_cache.active_match != Some(nearest) {
            // Only move away from the active match if it has actually scrolled
            // out of the viewport. While it is still on screen the user is
            // just panning around within the same view, and we should honour
            // their explicit Next/Prev choice rather than snapping to the
            // first match visible at the viewport top.
            let active_y = search_cache
                .active_match
                .and_then(|i| search_cache.search_match_virtual_ys.get(i).copied());
            let in_viewport =
                active_y.is_some_and(|y| y >= vt && y < vt + search_cache.last_viewport_height);
            if !in_viewport {
                search_cache.active_match = Some(nearest);
                search_cache.sync_active_search_range();
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
    #[cfg(feature = "regex")]
    pub fn sync_scrollable_active_match(
        &mut self,
        id: &Id,
        viewport_cache: bool,
        user_scrolled: bool,
    ) {
        if !viewport_cache {
            self.sync_active_match(id, user_scrolled);
            return;
        }

        // Call `viewer_cache` once and split the struct fields to avoid a
        // double mutable borrow of `self` (`search_cache` lives inside the
        // same `ViewerCache` as split_points`).
        let vc = viewer_cache(self, id);
        let search_cache = &mut vc.search_cache;

        if user_scrolled {
            search_cache.search_scroll_protection = 0;
            search_cache.go_to_match_locked = false;
        }

        let current_offset = search_cache
            .viewport_start_byte_offset(&vc.split_points)
            .unwrap_or(0);

        if !search_cache.search_ranges.is_empty()
            && search_cache.search_scroll_protection == 0
            && current_offset != search_cache.last_viewport_offset
        {
            let idx = search_cache
                .search_ranges
                .partition_point(|r| r.start < current_offset);
            // `idx` is the number of matches whose start byte is strictly before
            // the viewport. The last such match (idx-1) is the one the user
            // has most recently scrolled past. At the document top idx == 0
            // (nothing yet passed), so the nearest match is the first one (0),
            // not the last (which would be a spurious wrap-around).
            let nearest = idx.saturating_sub(1);

            if search_cache.active_match != Some(nearest) {
                // Only move away from the active match if it has scrolled out
                // of the viewport. Matches rendered in the current slice have
                // their exact pixel Y in `search_match_virtual_ys`; those outside
                // the slice carry the `NEG_INFINITY` sentinel, which is always
                // < any real viewport top (>= 0) and is therefore not-in-viewport.
                let active_y = search_cache
                    .active_match
                    .and_then(|i| search_cache.search_match_virtual_ys.get(i).copied());
                let vt = search_cache.last_viewport_top_y;
                let in_viewport =
                    active_y.is_some_and(|y| y >= vt && y < vt + search_cache.last_viewport_height);

                if !in_viewport {
                    search_cache.active_match = Some(nearest);
                    search_cache.sync_active_search_range();
                    // Do NOT call `scroll_to_active_search_match` here: the
                    // viewport is already where the user put it.
                }
            }
        }

        search_cache.last_viewport_offset = current_offset;
        if search_cache.search_scroll_protection > 0 {
            search_cache.search_scroll_protection -= 1;
        }
    }
}

// Flush the current inline run: search the combined text and emit one
// source range per match, potentially spanning multiple events.
#[cfg(feature = "regex")]
fn flush_run(
    search_cache: &mut crate::pulldown::SearchCache,
    regex: &fancy_regex::Regex,
    run_text: &mut String,
    run_segs: &mut Vec<(usize, Range<usize>)>,
) {
    if !run_segs.is_empty() {
        for m in regex.find_iter(&*run_text).flatten() {
            let cstart = m.start();
            let cend = m.end();
            let si = run_segs
                .partition_point(|(off, _)| *off <= cstart)
                .saturating_sub(1);
            let src_start = run_segs[si].1.start + (cstart - run_segs[si].0);
            let ei = run_segs
                .partition_point(|(off, _)| *off < cend)
                .saturating_sub(1);
            let src_end = run_segs[ei].1.start + (cend - run_segs[ei].0);
            search_cache.search_ranges.push(src_start..src_end);
        }
        run_text.clear();
        run_segs.clear();
    }
}

// Flush pending_buf as plain single-event matches (not an alert keyword).
#[cfg(feature = "regex")]
fn flush_pending(
    search_cache: &mut crate::pulldown::SearchCache,
    regex: &fancy_regex::Regex,
    pending_buf: &mut Vec<(String, Range<usize>)>,
) {
    for (text, r) in pending_buf.drain(..) {
        for m in regex.find_iter(&text).flatten() {
            search_cache
                .search_ranges
                .push(r.start + m.start()..r.start + m.end());
        }
    }
}

// Decide what to emit for the buffered blockquote first-paragraph text.
// If it is a known alert keyword the viewer renders `identifier_rendered`
// (e.g. "Note") rather than the raw source text, so we match the regex
// against the rendered form and store the keyword's source span as the
// range anchor (the blockquote renderer checks for overlap and highlights
// the title label). Otherwise fall back to normal single-event matching.
#[cfg(feature = "regex")]
fn decide_alert_matches(
    search_cache: &mut crate::pulldown::SearchCache,
    regex: &fancy_regex::Regex,
    pending_buf: &mut Vec<(String, Range<usize>)>,
) {
    {
        let ident: String = pending_buf.iter().map(|(t, _)| t.as_str()).collect();
        let alert_title: Option<String> =
            try_get_alert(&search_cache.alerts, &ident).map(|a| a.identifier_rendered.clone());
        if let Some(rendered) = alert_title {
            let src_start = pending_buf.iter().map(|(_, r)| r.start).min().unwrap_or(0);
            let src_end = pending_buf.iter().map(|(_, r)| r.end).max().unwrap_or(0);
            let alert_src_range = src_start..src_end;
            for _ in regex.find_iter(&rendered) {
                search_cache.search_ranges.push(alert_src_range.clone());
            }
            pending_buf.clear();
        } else {
            flush_pending(search_cache, regex, pending_buf);
        }
    };
}

// Incorporate the current event text into the current search match or not, as appropriate.
#[cfg(feature = "regex")]
#[allow(clippy::too_many_arguments)]
fn decide_search_match_continuity(
    search_cache: &mut crate::pulldown::SearchCache,
    regex: &fancy_regex::Regex,
    run_text: &mut String,
    run_segs: &mut Vec<(usize, Range<usize>)>,
    in_bq_first_run: bool,
    pending_buf: &mut Vec<(String, Range<usize>)>,
    event: pulldown_cmark::Event<'_>,
    range: Range<usize>,
) {
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
            flush_run(search_cache, regex, run_text, run_segs);
        }
    }
}

#[cfg(feature = "regex")]
/// Builds the compiled search regex from the current query and options in
/// `search_cache`, updating `search_regex_error` as a side-effect.
///
/// Returns `None` — and the caller should return early — when:
/// - the query is empty (no-op), or
/// - the regex fails to compile (invalid pattern; the error is stored in
///   `search_regex_error` for display to the user).
///
/// On success clears `search_regex_error` and returns `Some(regex)`.
fn build_search_regex(
    search_cache: &mut crate::pulldown::SearchCache,
) -> Option<fancy_regex::Regex> {
    if search_cache.search_query.is_empty() {
        return None;
    }

    let options = search_cache.search_options;
    let mut pattern = if options.contains(SearchOptions::REGEX) {
        search_cache.search_query.clone()
    } else {
        fancy_regex::escape(&search_cache.search_query).to_string()
    };

    if options.contains(SearchOptions::WHOLE_WORD) {
        pattern = format!(r"\b(?:{pattern})\b");
    }

    match fancy_regex::RegexBuilder::new(&(Cow::from(pattern)))
        .case_insensitive(!options.contains(SearchOptions::CASE_SENSITIVE))
        .build()
    {
        Ok(regex) => {
            search_cache.search_regex_error = None;
            Some(regex)
        }
        Err(err) => {
            search_cache.search_regex_error = Some(err.to_string());
            None
        }
    }
}

pub fn viewer_cache<'a>(cache: &'a mut CommonMarkCache, id: &Id) -> &'a mut ViewerCache {
    if !cache.viewers.contains_key(id) {
        cache.viewers.insert(*id, ViewerCache::default());
    }
    cache.viewers.get_mut(id).unwrap()
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
