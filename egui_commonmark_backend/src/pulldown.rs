#[cfg(feature = "regex")]
use crate::SearchOptions;
use crate::alerts::*;
use egui::{Pos2, Vec2};
use pulldown_cmark::Options;
use std::collections::HashMap;
use std::ops::Range;

/// One recorded block boundary used by the viewport-culling and search
/// machinery. Every top-level block (paragraph, heading, code block) that
/// sits at a safe renderer restart point gets one entry.
#[derive(Debug, Clone)]
pub struct SplitPoint {
    /// Index of this block's end-event in the flat event stream. Used to
    /// skip past already-rendered blocks when starting a viewport slice.
    pub event_index: usize,
    /// Virtual start position of this block (content-relative; Y = 0 is the
    /// document top). Captured at the `Start` event so it reflects the block
    /// top rather than the cursor position just before the `End` event.
    pub vstart: Pos2,
    /// Virtual end position of this block.
    pub vend: Pos2,
    /// Source byte range of this block in the original document text. Lets
    /// [`ViewerCache::virtual_y_for_byte_offset`] approximate on-screen
    /// positions of arbitrary offsets (e.g. search matches) without a fresh
    /// full render.
    pub src_span: Range<usize>,
}

#[cfg(feature = "regex")]
#[derive(Debug, Default)]
pub struct SearchCache {
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
    pub search_ranges: Vec<Range<usize>>,
    /// The currently active (focused) search match, highlighted more
    /// prominently than the others.
    pub active_search_range: Option<Range<usize>>,
    /// The ordinal number of the active search match
    pub active_match: Option<usize>,
    /// The y positions of the search matches, relative to the document
    pub search_match_virtual_ys: Vec<f32>,
    /// Use with `show_scrollable`. Used to detect viewport movement without
    /// relying on input events.
    pub last_viewport_offset: usize,
    /// Counts down after a search-initiated scroll (`go_to_match` /
    /// `update_search_matches`) to prevent viewport-driven active-match
    /// updates from overriding the scroll animation. Cleared immediately
    /// when the user scrolls manually.
    pub search_scroll_protection: u32,
    /// Set by [`go_to_match`](Self::go_to_match) to hold `sync_active_match`
    /// from overriding the explicitly chosen match until the user next
    /// scrolls. Unlike `search_scroll_protection` (which counts down and
    /// expires), this stays latched indefinitely so that multiple matches
    /// on the same visual line don't snap back to the first one once the
    /// scroll-protection countdown expires.
    pub(crate) go_to_match_locked: bool,
    /// Set by [`CommonMarkCache::scroll_to_active_search_match`] and cleared
    /// once the render pass that finds and scrolls to the active match runs.
    pending_scroll_to_active_match: bool,
    /// Number of consecutive internal retries (viewport blind-scroll not
    /// yet having brought the match into a rendered slice). Bounds an
    /// otherwise-unlikely non-convergent loop; reset whenever the user
    /// requests a fresh scroll via [`CommonMarkCache::scroll_to_active_search_match`].
    pending_scroll_to_active_match_retries: u8,
    /// The most recent viewport top Y (virtual, content-relative), recorded
    /// every frame the cheap viewport-only path renders. Lets callers
    /// approximate "what's currently visible" (e.g. to implement search
    /// that starts from the current scroll position) via
    /// [`Self::byte_offset_for_virtual_y`].
    pub last_viewport_top_y: f32,
    /// Height of the viewport on the most recent frame, in the same virtual
    /// coordinate space as `last_viewport_top_y`. Together they define the visible
    /// interval `[last_viewport_top_y, last_viewport_top_y + last_viewport_height)`.
    pub last_viewport_height: f32,
}

#[cfg(feature = "regex")]
impl SearchCache {
    /// Approximate the virtual Y (content-relative; 0 = document top) of a
    /// byte offset in the source text, using the split points collected
    /// during the last full render. This never requires a fresh render: at
    /// worst (e.g. a byte offset inside a large, untracked container like a
    /// list or table) it falls back to the nearest preceding tracked block,
    /// which is the same granularity the viewport-slice calculation itself
    /// already uses.
    ///
    /// Returns `None` only if there are no split points at all yet (i.e. no
    /// full render has happened), in which case the caller has no choice
    /// but to wait for one.
    pub fn virtual_y_for_byte_offset(
        &self,
        split_points: &[SplitPoint],
        offset: usize,
    ) -> Option<f32> {
        if let Some(sp) = split_points.iter().find(|sp| sp.src_span.contains(&offset)) {
            return Some(sp.vstart.y);
        }

        // Not inside any tracked block, e.g. it's inside a list/table/
        // blockquote, which aren't tracked individually. Use the nearest
        // preceding tracked block as a reasonable approximation, same as
        // `show_scrollable`'s own slice calculation does.
        split_points
            .iter()
            .rev()
            .find(|sp| sp.src_span.start <= offset)
            .map(|sp| sp.vstart.y)
            .or_else(|| split_points.first().map(|sp| sp.vstart.y))
    }

    /// The inverse of [`Self::virtual_y_for_byte_offset`]: approximate the
    /// source byte offset of whatever is at (or just before) the given
    /// virtual Y, using the same split points. Returns `None` only if there
    /// are no split points at all yet.
    pub fn byte_offset_for_virtual_y(&self, split_points: &[SplitPoint], y: f32) -> Option<usize> {
        split_points
            .iter()
            .rev()
            .find(|sp| sp.vstart.y <= y)
            .map(|sp| sp.src_span.start)
            .or_else(|| split_points.first().map(|sp| sp.src_span.start))
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

    // Update the active search range to the desired ordinal value
    pub fn sync_active_search_range(&mut self) {
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
    pub fn viewport_start_byte_offset(&self, split_points: &[SplitPoint]) -> Option<usize> {
        self.byte_offset_for_virtual_y(split_points, self.last_viewport_top_y)
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
    /// Combined with [`last_viewport_top_y`](Self::last_viewport_top_y),
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
    pub fn last_viewport_top_y(&self) -> f32 {
        self.last_viewport_top_y
    }

    /// Updates the per-match virtual-y positions and the viewport geometry
    /// recorded during a [`show`](crate::CommonMarkViewer::show) call.
    /// `match_ys` is an iterator of `(match_index, virtual_y)` pairs.
    /// `viewport_height` is the height of the clip rect (same coordinate
    /// space as `viewport_top_y`).
    ///
    /// Used internally by the renderer; read the results via
    /// [`search_match_virtual_ys`](Self::search_match_virtual_ys) and
    /// [`last_viewport_top_y`](Self::last_viewport_top_y).
    pub fn update_show_viewport(
        &mut self,
        match_ys: impl IntoIterator<Item = (usize, f32)>,
        viewport_top_y: f32,
        viewport_height: f32,
    ) {
        let n = self.search_ranges.len();
        self.search_match_virtual_ys.clear();
        // Use NEG_INFINITY as the sentinel for "not rendered in this slice".
        // A real match Y is always >= 0 (content-relative from the document
        // top), so NEG_INFINITY is unambiguously "not in viewport" for any
        // viewport top >= 0, avoiding the false-positive that 0.0 caused at
        // the document top.
        self.search_match_virtual_ys.resize(n, f32::NEG_INFINITY);
        for (idx, y) in match_ys {
            if idx < n {
                self.search_match_virtual_ys[idx] = y;
            }
        }
        self.last_viewport_top_y = viewport_top_y;
        self.last_viewport_height = viewport_height;
    }
}

/// A content cache for document-viewer-specific content, requiring an `egui::Id`
/// as the viewer identifier.
#[derive(Default, Debug)]
pub struct ViewerCache {
    pub available_size: Vec2,
    pub page_size: Option<Vec2>,
    /// One [`SplitPoint`] per top-level block at a safe renderer restart
    /// boundary, in document order.
    pub split_points: Vec<SplitPoint>,
    /// Heading slug → virtual Y (content-relative; 0 = document top).
    /// Populated during the full render; used by the viewport path to
    /// scroll to headings outside the currently rendered slice.
    pub heading_y_positions: HashMap<String, f32>,
    /// Keyboard / programmatic scroll delta applied inside the next
    /// `show_scrollable` call and then cleared.
    pub pending_scroll_delta: egui::Vec2,
    /// The ID of the heading to scroll to. This is set when a link whose destination is
    /// a fragment (e.g. `#my-heading`) has been clicked.
    scroll_to_id_target: Option<String>,
    #[cfg(feature = "regex")]
    pub search_cache: SearchCache,
}

impl ViewerCache {
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

    /// Get the desired fragment. This is the id which will be scrolled to if it is found
    /// in the markdown.
    pub fn scroll_to_id_target(&self) -> Option<&str> {
        self.scroll_to_id_target.as_deref()
    }

    /// Get mutable access to the desired fragment. Setting this will cause the viewer to scroll
    /// to the heading with this id if it exists. Setting it to None will prevent scrolling.
    pub fn scroll_to_id_target_mut(&mut self) -> &mut Option<String> {
        &mut self.scroll_to_id_target
    }
}

pub type EventIteratorItem<'e> = (usize, (pulldown_cmark::Event<'e>, Range<usize>));

/// Parse events until a desired end tag is reached or no more events are found.
/// This is needed for multiple events that must be rendered inside a single widget
pub fn delayed_events<'e>(
    events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
    end_at: impl Fn(pulldown_cmark::TagEnd) -> bool,
) -> Vec<(pulldown_cmark::Event<'e>, Range<usize>)> {
    let mut curr_event = events.next();
    let mut total_events = Vec::new();
    loop {
        if let Some(event) = curr_event.take() {
            total_events.push(event.1.clone());
            if let (_, (pulldown_cmark::Event::End(tag), _range)) = event
                && end_at(tag)
            {
                return total_events;
            }
        } else {
            return total_events;
        }

        curr_event = events.next();
    }
}

pub fn delayed_events_list_item<'e>(
    events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
) -> Vec<(pulldown_cmark::Event<'e>, Range<usize>)> {
    let mut curr_event = events.next();
    let mut total_events = Vec::new();
    loop {
        if let Some(event) = curr_event.take() {
            total_events.push(event.1.clone());
            if let (_, (pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Item), _range)) = event {
                return total_events;
            }

            if let (_, (pulldown_cmark::Event::Start(pulldown_cmark::Tag::List(_)), _range)) = event
            {
                return total_events;
            }
        } else {
            return total_events;
        }

        curr_event = events.next();
    }
}

type Column<'e> = Vec<(pulldown_cmark::Event<'e>, Range<usize>)>;
type Row<'e> = Vec<Column<'e>>;

pub struct Table<'e> {
    pub header: Row<'e>,
    pub rows: Vec<Row<'e>>,
}

fn parse_row<'e>(
    events: &mut impl Iterator<Item = (pulldown_cmark::Event<'e>, Range<usize>)>,
) -> Vec<Column<'e>> {
    let mut row = Vec::new();
    let mut column = Vec::new();

    for (e, src_span) in events.by_ref() {
        if let pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableCell) = e {
            row.push(column);
            column = Vec::new();
        }

        if let pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableHead) = e {
            break;
        }

        if let pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableRow) = e {
            break;
        }

        column.push((e, src_span));
    }

    row
}

pub fn parse_table<'e>(events: &mut impl Iterator<Item = EventIteratorItem<'e>>) -> Table<'e> {
    let mut all_events = delayed_events(events, |end| matches!(end, pulldown_cmark::TagEnd::Table))
        .into_iter()
        .peekable();

    let header = parse_row(&mut all_events);

    let mut rows = Vec::new();
    while all_events.peek().is_some() {
        let row = parse_row(&mut all_events);
        rows.push(row);
    }

    Table { header, rows }
}

/// try to parse events as an alert quote block. This will modify the events
/// to remove the parsed text that should not be rendered.
/// Assumes that the first element is a Paragraph
pub fn parse_alerts<'a>(
    alerts: &'a AlertBundle,
    events: &mut Vec<(pulldown_cmark::Event<'_>, Range<usize>)>,
) -> Option<&'a Alert> {
    // no point in parsing if there are no alerts to render
    if !alerts.is_empty() {
        let mut alert_ident = "".to_owned();
        let mut alert_ident_ends_at = 0;
        let mut has_extra_line = false;

        for (i, (e, _src_span)) in events.iter().enumerate() {
            if let pulldown_cmark::Event::End(_) = e {
                // > [!TIP]
                // >
                // > Detect the first paragraph
                // In this case the next text will be within a paragraph so it is better to remove
                // the entire paragraph
                alert_ident_ends_at = i;
                has_extra_line = true;
                break;
            }

            if let pulldown_cmark::Event::SoftBreak = e {
                // > [!NOTE]
                // > this is valid and will produce a soft break
                alert_ident_ends_at = i;
                break;
            }

            if let pulldown_cmark::Event::HardBreak = e {
                // > [!NOTE]<whitespace>
                // > this is valid and will produce a hard break
                alert_ident_ends_at = i;
                break;
            }

            if let pulldown_cmark::Event::Text(text) = e {
                alert_ident += text;
            }
        }

        let alert = try_get_alert(alerts, &alert_ident);

        if alert.is_some() {
            // remove the text that identifies it as an alert so that it won't end up in the
            // render
            //
            // FIXME: performance improvement potential
            if has_extra_line {
                for _ in 0..=alert_ident_ends_at {
                    events.remove(0);
                }
            } else {
                for _ in 0..alert_ident_ends_at {
                    // the first element must be kept as it _should_ be Paragraph
                    events.remove(1);
                }
            }
        }

        alert
    } else {
        None
    }
}

/// Supported pulldown_cmark options
#[inline]
pub fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_DEFINITION_LIST
}
