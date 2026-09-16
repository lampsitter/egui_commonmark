use std::borrow::ToOwned;
use std::iter::Peekable;
use std::ops::Range;

use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, RichText, TextStyle, Ui};

use crate::List;
use egui_commonmark_backend::elements::*;
use egui_commonmark_backend::misc::*;
use egui_commonmark_backend::pulldown::*;
use egui_commonmark_backend::search::search_intervals;
use pulldown_cmark::{CowStr, HeadingLevel};

/// Newline logic is constructed by the following:
/// All elements try to insert a newline before them (if they are allowed)
/// and end their own line.
struct Newline {
    /// Whether a newline should not be inserted before a widget. This is only for
    /// the first widget.
    should_not_start_newline_forced: bool,
    /// Whether an element should insert a newline before it
    should_start_newline: bool,
    /// Whether an element should end it's own line using a newline
    /// This will have to be set to false in cases such as when blocks are within
    /// a list.
    should_end_newline: bool,
    /// only false when the widget is the last one.
    should_end_newline_forced: bool,
}

impl Default for Newline {
    fn default() -> Self {
        Self {
            should_not_start_newline_forced: true,
            should_start_newline: true,
            should_end_newline: true,
            should_end_newline_forced: true,
        }
    }
}

impl Newline {
    pub fn can_insert_end(&self) -> bool {
        self.should_end_newline && self.should_end_newline_forced
    }

    pub fn can_insert_start(&self) -> bool {
        self.should_start_newline && !self.should_not_start_newline_forced
    }

    pub fn try_insert_start(&self, ui: &mut Ui) {
        if self.can_insert_start() {
            newline(ui);
        }
    }

    pub fn try_insert_end(&self, ui: &mut Ui) {
        if self.can_insert_end() {
            newline(ui);
        }
    }
}

#[derive(Default)]
struct DefinitionList {
    is_first_item: bool,
    is_def_list_def: bool,
}

pub struct CommonMarkViewerInternal {
    curr_table: usize,
    text_style: Style,
    list: List,
    link: Option<Link>,
    image: Option<Image>,
    line: Newline,
    code_block: Option<CodeBlock>,

    /// Only populated if the html_fn option has been set
    html_block: String,
    is_list_item: bool,
    def_list: DefinitionList,
    is_table: bool,
    is_blockquote: bool,
    checkbox_events: Vec<CheckboxClickEvent>,
    deferred_scroll_to_heading: Option<String>,
    /// Set during a full render if any image rendered at zero height (texture
    /// still loading). When true, split points are discarded and the full render
    /// repeats next frame until all images have stable heights.
    any_image_loading: bool,
    /// Taken from [`CommonMarkCache::take_pending_scroll_to_active_match`] at
    /// the start of a render pass. When true, the renderer scrolls to (and
    /// centers) the active search match the moment it is found, then clears
    /// this flag so it only happens once per pass.
    want_scroll_to_active_match: bool,
    /// Screen-space y of the document content origin for this render pass,
    /// captured once at the top of `show()`. Subtracting this from any
    /// widget's screen y gives a scroll-independent virtual y.
    content_origin_y: f32,
    /// `(global_match_index, virtual_y)` pairs accumulated across all
    /// `event_text` calls during this render pass. Flushed into
    /// [`CommonMarkCache`] at the end of `show()` (non-scrollable path only).
    search_match_ys_scratch: Vec<(usize, f32)>,
}

pub(crate) struct CheckboxClickEvent {
    pub(crate) checked: bool,
    pub(crate) span: Range<usize>,
}

impl CommonMarkViewerInternal {
    pub fn new() -> Self {
        Self {
            curr_table: 0,
            text_style: Style::default(),
            list: List::default(),
            link: None,
            image: None,
            line: Newline::default(),
            is_list_item: false,
            def_list: Default::default(),
            code_block: None,
            html_block: String::new(),
            is_table: false,
            is_blockquote: false,
            checkbox_events: Vec::new(),
            deferred_scroll_to_heading: None,
            any_image_loading: false,
            want_scroll_to_active_match: false,
            content_origin_y: 0.0,
            search_match_ys_scratch: Vec::new(),
        }
    }
}

fn parser_options_extras(
    is_math_enabled: bool,
    is_scroll_to_heading_enabled: bool,
) -> pulldown_cmark::Options {
    let mut result = parser_options();
    if is_math_enabled {
        result |= pulldown_cmark::Options::ENABLE_MATH;
    }
    if is_scroll_to_heading_enabled {
        result |= pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;
    }
    result
}

/// Tracks how many full-document renders have been performed for each
/// `show_scrollable` source, keyed by `split_points_id`. Only compiled in
/// test builds; used by the perf-regression tests below to assert that the
/// cheap viewport-only path is taken once split points have been populated.
/// Because each source uses its own key, tests that run in parallel do not
/// interfere with each other's counts.
#[cfg(test)]
static FULL_RENDER_COUNTS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<Id, usize>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

impl CommonMarkViewerInternal {
    /// Be aware that this acquires egui::Context internally.
    /// If split Id is provided then split points will be populated
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
        split_points_id: Option<Id>,
    ) -> (egui::InnerResponse<()>, Vec<CheckboxClickEvent>) {
        #[cfg(test)]
        if let Some(sid) = split_points_id {
            *FULL_RENDER_COUNTS.lock().unwrap().entry(sid).or_insert(0) += 1;
        }

        self.any_image_loading = false;
        self.want_scroll_to_active_match = cache.take_pending_scroll_to_active_match();
        self.search_match_ys_scratch.clear();
        let max_width = options.max_width(ui);

        // Determine the effective id for split-point recording this frame.
        // show_scrollable's full render passes its source_id as split_points_id and
        // always records. show_with_id stores its source_id in options and rebuilds
        // only when the available width changes; stable frames reuse cached split points.
        let record_id: Option<Id> = split_points_id.or_else(|| {
            options.source_id.and_then(|id| {
                let sc = scroll_cache(cache, &id);
                let needs_rebuild =
                    sc.split_points.is_empty() || (sc.available_size.x - max_width).abs() > 0.5;
                if needs_rebuild {
                    sc.split_points.clear();
                    sc.heading_y_positions.clear();
                    sc.available_size.x = max_width;
                    Some(id)
                } else {
                    None // existing split points are still valid; skip re-recording
                }
            })
        });

        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        let re = ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            // Do a full render
            let content_origin_y = self.full_render(cache, options, text, record_id, max_width, ui);

            // deferral to make it consistent no matter whether the target is before or after the link
            *cache.scroll_to_id_target_mut() = self.deferred_scroll_to_heading.take();

            if let Some(source_id) = split_points_id {
                if self.any_image_loading {
                    // Images are still loading — split points are unreliable.
                    // Discard and leave page_size = None so the full render repeats
                    // next frame (the image loader triggers the repaint automatically).
                    let sc = scroll_cache(cache, &source_id);
                    sc.split_points.clear();
                    sc.heading_y_positions.clear();
                } else {
                    let final_y = ui.next_widget_position().y;
                    scroll_cache(cache, &source_id).page_size =
                        Some(egui::vec2(max_width, final_y - content_origin_y));
                }
            } else {
                // Non-scrollable show() path: flush per-match virtual-y positions
                // and the current viewport top so callers can sync the active
                // match after the user scrolls.
                let viewport_top_y = ui.clip_rect().min.y - content_origin_y;
                let viewport_height = ui.clip_rect().height();
                cache.update_show_viewport(
                    self.search_match_ys_scratch.drain(..),
                    viewport_top_y,
                    viewport_height,
                );
                // For show_with_id: keep the ScrollableCache's viewport position in sync so
                // that viewport_start_byte_offset returns the current scroll location.
                // options.source_id is Some on every frame (including no-rebuild frames).
                if let Some(id) = options.source_id {
                    scroll_cache(cache, &id).last_viewport_top_y = viewport_top_y;
                }
            }
        });

        (re, std::mem::take(&mut self.checkbox_events))
    }

    /// Perform a full render of the document.
    fn full_render(
        &mut self,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions<'_>,
        text: &str,
        record_id: Option<Id>,
        max_width: f32,
        ui: &mut Ui,
    ) -> f32 {
        let mut events = pulldown_cmark::Parser::new_ext(
            text,
            parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading),
        )
        .into_offset_iter()
        .enumerate()
        .peekable();

        // Screen-space y of the content origin. Subtracting this from any
        // cursor position gives virtual (content-relative) y comparable to
        // viewport.min/max.y from show_viewport.
        let content_origin_y = ui.next_widget_position().y;
        self.content_origin_y = content_origin_y;

        // Cursor at the visual top of the current block, captured at
        // Start(Block) so that vstart reflects the block top, not its bottom.
        let mut block_start_position: Option<Pos2> = None;
        // Source byte offset paired with block_start_position, so split
        // points can also record each block's source span (used to
        // locate search matches without a fresh full render).
        let mut block_start_src: Option<usize> = None;

        while let Some((index, (e, src_span))) = events.next() {
            let start_position = ui.next_widget_position();

            let is_safe_block_start = !self.list.is_inside_a_list()
                && matches!(
                    e,
                    pulldown_cmark::Event::Start(
                        pulldown_cmark::Tag::Paragraph
                            | pulldown_cmark::Tag::Heading { .. }
                            | pulldown_cmark::Tag::CodeBlock(_)
                    )
                );
            if is_safe_block_start {
                block_start_position = Some(start_position);
                block_start_src = Some(src_span.start);
            }

            // Record virtual y for each named heading so the viewport path
            // can jump to headings that are outside the rendered slice.
            if let (
                Some(sid),
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { id: Some(id), .. }),
            ) = (record_id, &e)
            {
                scroll_cache(cache, &sid)
                    .heading_y_positions
                    .insert(id.to_string(), ui.cursor().min.y - content_origin_y);
            }

            // Only record split points at clean, top-level block boundaries
            // where the renderer has no pending state and can restart safely.
            let is_safe_block_end = !self.list.is_inside_a_list()
                && matches!(
                    e,
                    pulldown_cmark::Event::End(
                        pulldown_cmark::TagEnd::Paragraph
                            | pulldown_cmark::TagEnd::Heading { .. }
                            | pulldown_cmark::TagEnd::CodeBlock
                    )
                );

            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            let block_end_src = src_span.end;
            self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

            if let Some(source_id) = record_id
                && is_safe_block_end
            {
                let scroll_cache = scroll_cache(cache, &source_id);
                let end_position = ui.next_widget_position();

                let split_point_exists = scroll_cache
                    .split_points
                    .iter()
                    .any(|(i, _, _, _)| *i == index);

                if !split_point_exists {
                    // Use block_start_position (Start event) not start_position
                    // (cursor just before End) so that vstart is the block top.
                    let raw_vstart = block_start_position.take().unwrap_or(start_position);
                    let vstart = egui::pos2(raw_vstart.x, raw_vstart.y - content_origin_y);
                    let vend = egui::pos2(end_position.x, end_position.y - content_origin_y);
                    let block_src_span =
                        block_start_src.take().unwrap_or(block_end_src)..block_end_src;
                    scroll_cache
                        .split_points
                        .push((index, vstart, vend, block_src_span));
                }
            }

            if index == 0 {
                self.line.should_not_start_newline_forced = false;
            }
        }
        content_origin_y
    }

    pub(crate) fn show_scrollable(
        &mut self,
        source_id: Id,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let available_size = ui.available_size();
        let scroll_id = source_id.with("_scroll_area");

        if !options.use_viewport_cache {
            // Full-document render every frame; egui clips what is off-screen.
            // Split points are maintained for viewport_start_byte_offset (search
            // anchoring), rebuilt only on width change — matching egui's own
            // galley-cache invalidation and the show_with_id path.
            let needs_rebuild = {
                let sc = scroll_cache(cache, &source_id);
                sc.page_size = None;
                sc.heading_y_positions.clear(); // not used in this path
                let rebuild = sc.split_points.is_empty()
                    || (sc.available_size.x - available_size.x).abs() > 0.5;
                if rebuild {
                    sc.split_points.clear();
                    sc.available_size.x = available_size.x;
                }
                rebuild
            };
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // Capture viewport top before content is placed; clip_rect
                    // already reflects the current scroll position.
                    let viewport_top_y = ui.clip_rect().min.y - ui.next_widget_position().y;
                    apply_pending_scroll_delta(cache, ui);
                    let sid = if needs_rebuild { Some(source_id) } else { None };
                    self.show(ui, cache, options, text, sid);
                    let sc = scroll_cache(cache, &source_id);
                    if needs_rebuild {
                        // show() sets page_size as a side-effect of receiving
                        // Some(source_id); clear it so the next frame still
                        // takes this full-render path, not the viewport-slice one.
                        sc.page_size = None;
                    }
                    // Keep last_viewport_top_y in sync for viewport_start_byte_offset.
                    sc.last_viewport_top_y = viewport_top_y;
                });
            return;
        }

        // If the scroll cache is invalidated, force a full render.
        let Some(page_size) = scroll_cache(cache, &source_id).page_size else {
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    apply_pending_scroll_delta(cache, ui);
                    self.show(ui, cache, options, text, Some(source_id));
                });
            scroll_cache(cache, &source_id).available_size = available_size;
            return;
        };

        // Try to fulfil a pending scroll-to-active-match directly from the
        // slice we're about to render: if the match is already visible (the
        // common case, e.g. stepping through nearby results), the per-widget
        // rendering code below finds it and scrolls precisely, with no extra
        // cost. If it isn't in the rendered slice, `pending_match_scroll_y`
        // (below) provides a cheap blind scroll toward its approximate
        // position using data already collected by the last full render —
        // this never requires re-rendering the whole document.
        self.want_scroll_to_active_match = cache.take_pending_scroll_to_active_match();

        let events = pulldown_cmark::Parser::new_ext(
            text,
            parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading),
        )
        .into_offset_iter()
        .collect::<Vec<_>>();

        let num_rows = events.len();

        // Resolve any pending TOC scroll via the cached heading positions so that
        // navigation works even when the target is outside the rendered slice.
        let pending_scroll_y: Option<f32> = {
            let slug_owned = cache.scroll_to_id_target().map(ToOwned::to_owned);
            slug_owned.as_ref().and_then(|slug| {
                let sc = scroll_cache(cache, &source_id);
                if let Some(&y) = sc.heading_y_positions.get(slug) {
                    cache.scroll_to_id_target_mut().take();
                    Some(y)
                } else {
                    None
                }
            })
        };
        let pending_delta = std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);

        // Virtual y of the active match used to decide whether a blind
        // scroll is needed before the slice renders.
        //
        // Prefer the precise per-match Y recorded during the *previous*
        // frame's render (search_match_virtual_ys). That value is exact for
        // any match that was on screen last frame — including matches deep
        // inside tall code blocks, where virtual_y_for_byte_offset only
        // returns the block-top Y. Using the block-top Y for such a match
        // triggers a spurious blind scroll to the top of the block followed
        // by a second scroll to the match, producing the "scrolls to the
        // previous page then back" animation the user sees.
        //
        // Fall back to the block-level split-point approximation only when
        // the match was not rendered last frame (Y stored as 0.0).
        let pending_match_scroll_y: Option<f32> = if self.want_scroll_to_active_match {
            let precise_y = cache
                .active_match()
                .and_then(|i| cache.search_match_virtual_ys().get(i).copied())
                .filter(|&y| y > 0.0);
            if precise_y.is_some() {
                precise_y
            } else {
                cache
                    .active_search_range()
                    .map(|r| r.start)
                    .and_then(|start| {
                        scroll_cache(cache, &source_id).virtual_y_for_byte_offset(start)
                    })
            }
        } else {
            None
        };

        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                if let Some(y) = pending_scroll_y {
                    // heading_y_positions stores virtual y (content-relative, 0 = top).
                    // Inside show_viewport, next_widget_position().y = screen_top − scroll.
                    // scroll_to_rect with Align::TOP sets new_scroll = y. ✓
                    let r = egui::Rect::from_min_size(
                        egui::pos2(0.0, ui.next_widget_position().y + y),
                        egui::Vec2::ZERO,
                    );
                    ui.scroll_to_rect(r, Some(egui::Align::TOP));
                }
                if let Some(y) = pending_match_scroll_y
                    && (y < viewport.min.y || y > viewport.max.y)
                {
                    // The match's approximate block isn't in view yet.
                    // Scroll to its approximate position immediately (no
                    // animation) and request a discard so this pass is never
                    // shown to the user. In pass 2 the scroll area begins with
                    // the offset already committed, so the viewport lands on
                    // the target; the per-widget code below then finds the
                    // exact match and refines the scroll precisely.
                    //
                    // Using ScrollAnimation::none() is essential: the
                    // animated default takes 0.1–0.3 s, but consecutive
                    // passes within the same run_dyn loop share nearly the
                    // same timestamp, so the animation would not progress and
                    // pass 2's viewport would still be at the old position.
                    let r = egui::Rect::from_min_size(
                        egui::pos2(0.0, ui.next_widget_position().y + y),
                        egui::Vec2::ZERO,
                    );
                    ui.scroll_to_rect_animation(
                        r,
                        Some(egui::Align::Center),
                        egui::style::ScrollAnimation::none(),
                    );
                    ui.ctx().request_discard("scroll to active search match");
                }
                if pending_delta != egui::Vec2::ZERO {
                    ui.scroll_with_delta(pending_delta);
                }

                ui.set_height(page_size.y);
                let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

                let max_width = options.max_width(ui);
                ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    // Compute the slice parameters and release the scroll_cache borrow
                    // before the push_id closure so that `cache` is freely accessible
                    // inside it.
                    let viewport_height = viewport.max.y - viewport.min.y;
                    let render_below = viewport.max.y + viewport_height;
                    let (skip_height, skip_count, take_count) = {
                        let scroll_cache = scroll_cache(cache, &source_id);
                        scroll_cache.last_viewport_top_y = viewport.min.y;
                        scroll_cache.last_viewport_height = viewport_height;
                        let preceding_split = scroll_cache
                            .split_points
                            .iter()
                            .rfind(|(_, _, vend, _)| vend.y < viewport.min.y)
                            .cloned();
                        let (_first_event_index, _, first_end_position, _) = preceding_split
                            .clone()
                            .unwrap_or((0, Pos2::ZERO, Pos2::ZERO, 0..0));
                        let last_event_index = scroll_cache
                            .split_points
                            .iter()
                            .find(|(_, vstart, _, _)| vstart.y > render_below)
                            .map_or(num_rows, |(index, _, _, _)| *index);
                        let skip_height = first_end_position.y.max(0.0);
                        // When a preceding split was found, its End(Block) is already
                        // accounted for in skip_height — re-processing it would add a
                        // duplicate newline. Start from the next event instead.
                        let (skip_count, take_count) = if let Some((idx, _, _, _)) = preceding_split
                        {
                            self.line.should_not_start_newline_forced = false;
                            // last_event_index should always be >= idx because
                            // split-points are ordered, but guard against stale
                            // cache or tiny documents producing an underflow.
                            let take = last_event_index.saturating_sub(idx);
                            (idx + 1, take)
                        } else {
                            (0, last_event_index)
                        };
                        (skip_height, skip_count, take_count)
                    }; // scroll_cache borrow released here

                    // Set content_origin_y to the screen Y of virtual-Y=0 (the
                    // document top) for this frame. This makes match Ys recorded
                    // by `event_text` comparable with viewport.min.y (the virtual
                    // scroll offset). Matches in the rendered slice get their
                    // exact pixel Y; those outside get the default 0.0 and are
                    // treated as not-in-viewport by sync_scrollable_active_match.
                    self.content_origin_y = ui.clip_rect().min.y - viewport.min.y;
                    self.search_match_ys_scratch.clear();

                    let mut events = events
                        .into_iter()
                        .enumerate()
                        .skip(skip_count)
                        .take(take_count)
                        .peekable();

                    // Give the viewport render a distinct widget parent_id namespace
                    // so that egui's warn_if_rect_changes_id check never fires.
                    //
                    // The check fires when the same screen rect has different widget
                    // IDs *and* at least one widget shares a parent_id between
                    // consecutive frames.  Widget IDs within this push_id scope are
                    // counter-based (sequential from 0 each frame).  The counter
                    // resets at the start of each rendered slice, so when skip_count
                    // advances by 1 (viewport crosses a split-point boundary), every
                    // widget in the visible overlap shifts its ID by 1 — same rect,
                    // same parent_id, different ID → warning fires.
                    //
                    // Fix: bake skip_count into the push_id salt.  Consecutive frames
                    // with different skip_count values get different parent_ids, so the
                    // parent_id match condition is never met.  When skip_count is
                    // stable (viewport moves within a split-point interval), the same
                    // slice renders with identical counter values → same IDs → no
                    // warning then either.
                    //
                    // The salt tuple also differs from the full-render path's implicit
                    // parent (no push_id), so the transition-frame guard from the
                    // full→viewport fix remains intact.
                    //
                    // IMPORTANT: push_id must be called BEFORE allocate_space so that
                    // the cursor is still at (0, 0) — the left edge of a full-width
                    // row.  Calling it after allocate_space leaves the cursor at
                    // (max_width, …) (right edge), making available_rect_before_wrap()
                    // return near-zero width and collapsing all content to a thin strip.
                    ui.push_id(("__cm_viewport", skip_count), |ui| {
                        // Skip over off-screen content by reserving its vertical space.
                        // Full width is essential: a narrower allocation would leave
                        // the cursor mid-row, misaligning the first visible block.
                        ui.allocate_space(egui::vec2(max_width, skip_height));

                        // If this pass will be discarded (blind scroll toward an
                        // off-screen search match), skip expensive widget rendering
                        // entirely. The space allocation above is still needed so
                        // that egui has the correct total height for scroll
                        // calculations. `want_scroll_to_active_match` stays true,
                        // so `retry_scroll_to_active_match` below re-arms the flag
                        // for pass 2, which renders normally at the new offset.
                        if !ui.ctx().will_discard() {
                            while let Some((i, (e, src_span))) = events.next() {
                                if events.peek().is_none() {
                                    self.line.should_end_newline_forced = false;
                                }
                                self.process_event(
                                    ui,
                                    &mut events,
                                    e,
                                    src_span,
                                    cache,
                                    options,
                                    max_width,
                                );
                                if i == 0 {
                                    self.line.should_not_start_newline_forced = false;
                                }
                            }

                            // Mirror show()'s deferred flush so that clicking a #fragment
                            // link while in the viewport path triggers a scroll next frame.
                            *cache.scroll_to_id_target_mut() =
                                self.deferred_scroll_to_heading.take();

                            // Flush the per-match virtual-Y positions collected by
                            // `event_text` into the cache, exactly as the non-scrollable
                            // show() path does. Skipped on discard frames (blind scroll
                            // toward an off-screen match) because no widgets rendered.
                            cache.update_show_viewport(
                                self.search_match_ys_scratch.drain(..),
                                viewport.min.y,
                                viewport_height,
                            );
                        }
                    });
                });
            });

        // If any image in this render reported zero height, split points are stale.
        // Discard them so the next frame falls back to a full render.
        if self.any_image_loading {
            let sc = scroll_cache(cache, &source_id);
            sc.page_size = None;
            sc.split_points.clear();
            sc.heading_y_positions.clear();
        }

        // The active match wasn't inside the slice we just rendered. Re-arm
        // the request (bounded — see `retry_scroll_to_active_match`); the
        // blind scroll toward its approximate position (recomputed fresh
        // next frame from `pending_match_scroll_y`) should bring it into a
        // rendered slice within a frame or two, entirely within the cheap
        // viewport-only path. We only ever fall back to a full render if
        // there are no split points at all to approximate a position from
        // (e.g. a document with no top-level paragraphs/headings/code
        // blocks), which is the same data a full render would need to
        // populate anyway.
        if self.want_scroll_to_active_match && cache.retry_scroll_to_active_match() {
            let sc = scroll_cache(cache, &source_id);
            if sc.split_points.is_empty() {
                sc.page_size = None;
            }
        }

        // Invalidate the cache when the available *width* changes (e.g. window resize).
        // Height changes — such as the search bar or TOC panel opening/closing —
        // do not affect split points or page_size: text wrapping only depends on
        // width, so a height-only change must never force a full render, especially on
        // a large document.
        let scroll_cache = scroll_cache(cache, &source_id);
        let width_changed = (available_size.x - scroll_cache.available_size.x).abs() > 0.5;
        scroll_cache.available_size = available_size; // always keep Y bookkeeping current
        if width_changed {
            scroll_cache.page_size = None;
            scroll_cache.split_points.clear();
            scroll_cache.heading_y_positions.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_event<'e>(
        &mut self,
        ui: &mut Ui,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        self.event(ui, event, src_span, cache, options, max_width);

        self.def_list_def_wrapping(events, max_width, cache, options, ui);
        self.item_list_wrapping(events, max_width, cache, options, ui);
        self.table(events, cache, options, ui, max_width);
        self.blockquote(events, max_width, cache, options, ui);
    }

    fn def_list_def_wrapping<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.def_list.is_def_list_def {
            self.def_list.is_def_list_def = false;

            let item_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::DefinitionListDefinition)
            });

            let mut events_iter = item_events.into_iter().enumerate().peekable();

            self.line.try_insert_start(ui);

            // Process a single event separately so that we do not insert spaces where we do not
            // want them
            self.line.should_start_newline = false;
            if let Some((_, (e, src_span))) = events_iter.next() {
                self.process_event(ui, &mut events_iter, e, src_span, cache, options, max_width);
            }

            ui.label(" ".repeat(options.indentation_spaces));
            self.line.should_start_newline = true;
            self.line.should_end_newline = false;
            // Required to ensure that the content is aligned with the indentation
            ui.horizontal_wrapped(|ui| {
                while let Some((_, (e, src_span))) = events_iter.next() {
                    self.process_event(
                        ui,
                        &mut events_iter,
                        e,
                        src_span,
                        cache,
                        options,
                        max_width,
                    );
                }
            });
            self.line.should_end_newline = true;

            // Only end the definition items line if it is not the last element in the list
            if !matches!(
                events.peek(),
                Some((
                    _,
                    (
                        pulldown_cmark::Event::End(pulldown_cmark::TagEnd::DefinitionList),
                        _
                    )
                ))
            ) {
                self.line.try_insert_end(ui);
            }
        }
    }

    fn item_list_wrapping<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_list_item {
            self.is_list_item = false;

            let item_events = delayed_events_list_item(events);
            let mut events_iter = item_events.into_iter().enumerate().peekable();

            // Required to ensure that the content of the list item is aligned with
            // the * or - when wrapping
            ui.horizontal_wrapped(|ui| {
                while let Some((_, (e, src_span))) = events_iter.next() {
                    self.process_event(
                        ui,
                        &mut events_iter,
                        e,
                        src_span,
                        cache,
                        options,
                        max_width,
                    );
                }
            });
        }
    }

    fn blockquote<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::BlockQuote(_))
            });
            self.line.try_insert_start(ui);

            // Currently the blockquotes are made in such a way that they need a newline at the end
            // and the start so when this is the first element in the markdown the newline must be
            // manually enabled
            self.line.should_not_start_newline_forced = false;

            // Capture the source byte range of the first Text event *before*
            // parse_alerts deletes it.  This is the position of the alert keyword
            // (e.g. "[!NOTE]") in the source; update_search_matches uses the same
            // range as the anchor for title matches, so the renderer can identify
            // them here and apply `label_with_search_highlight` to the title label.
            let identifier_src_range: Range<usize> = collected_events
                .iter()
                .find(|(e, _)| matches!(e, pulldown_cmark::Event::Text(_)))
                .map(|(_, r)| r.clone())
                .unwrap_or(0..0);

            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                let ident_src = identifier_src_range;

                blockquote(ui, alert.accent_color, |ui| {
                    newline(ui);
                    ui.colored_label(alert.accent_color, alert.icon.to_string());
                    ui.add_space(3.0);

                    // Render the alert title with search highlighting when the active
                    // query matched `identifier_rendered` (e.g. "Note" for [!NOTE]).
                    // update_search_matches stored the keyword's source bytes as the
                    // match range, so we check overlap with `ident_src` to decide.
                    let (has_title_match, is_title_active, title_global_idx) = {
                        let ranges = cache.search_ranges();
                        let has_match = !ranges.is_empty()
                            && ranges
                                .iter()
                                .any(|r| r.start < ident_src.end && r.end > ident_src.start);
                        let is_active = has_match
                            && cache.active_search_range().is_some_and(|a| {
                                a.start < ident_src.end && a.end > ident_src.start
                            });
                        let global_idx = if has_match {
                            ranges
                                .iter()
                                .enumerate()
                                .find(|(_, r)| r.start < ident_src.end && r.end > ident_src.start)
                                .map(|(i, _)| i)
                        } else {
                            None
                        };
                        (has_match, is_active, global_idx)
                    }; // all immutable borrows of cache released here

                    if has_title_match {
                        let title_len = alert.identifier_rendered.len();
                        let intervals = vec![(0..title_len, is_title_active)];
                        let (_, active_rect, all_rects) = label_with_search_highlight(
                            ui,
                            RichText::new(&alert.identifier_rendered).color(alert.accent_color),
                            &intervals,
                            options.search_match_bg(ui),
                            options.search_active_match_bg(ui),
                        );
                        if let Some(idx) = title_global_idx
                            && let Some(Some(rect)) = all_rects.first()
                        {
                            self.search_match_ys_scratch
                                .push((idx, rect.min.y - self.content_origin_y));
                        }
                        if self.want_scroll_to_active_match
                            && let Some(rect) = active_rect
                        {
                            ui.scroll_to_rect(rect, Some(egui::Align::Center));
                            self.want_scroll_to_active_match = false;
                        }
                    } else {
                        ui.colored_label(alert.accent_color, &alert.identifier_rendered);
                    }

                    newline(ui);
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                });
            } else {
                blockquote(ui, ui.visuals().weak_text_color(), |ui| {
                    self.text_style.quote = true;
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                    self.text_style.quote = false;
                });
            }

            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            self.line.try_insert_end(ui);
            self.is_blockquote = false;
        }
    }

    fn table<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
        max_width: f32,
    ) {
        if self.is_table {
            self.line.try_insert_start(ui);

            let id = ui.id().with("_table").with(self.curr_table);
            self.curr_table += 1;

            // egui's ScrollArea intercepts scroll_to_rect calls for ALL dimensions,
            // even those it doesn't scroll (see egui source: "We always take both
            // scroll targets regardless of which scroll axes are enabled").  This
            // means scroll_to_rect called from event_text() inside the table's
            // horizontal scroll area silently discards the vertical component,
            // preventing the outer vertical scroll area from bringing the row into
            // view.  Snapshot the scroll-request state before the table renders;
            // if the table consumed it we re-issue on the outer `ui` afterwards.
            let want_scroll_before = self.want_scroll_to_active_match;
            let scratch_start = self.search_match_ys_scratch.len();

            egui::Frame::group(ui.style()).show(ui, |ui| {
                let Table { header, rows } = parse_table(events);

                ui.spacing_mut().scroll.content_margin.bottom = ui.spacing().scroll.bar_width as i8;
                egui::ScrollArea::horizontal()
                    .id_salt(id.with("_hscroll"))
                    .show(ui, |ui| {
                        egui::Grid::new(id).striped(true).show(ui, |ui| {
                            for col in header {
                                ui.horizontal(|ui| {
                                    for (e, src_span) in col {
                                        let tmp_start = std::mem::replace(
                                            &mut self.line.should_start_newline,
                                            false,
                                        );
                                        let tmp_end = std::mem::replace(
                                            &mut self.line.should_end_newline,
                                            false,
                                        );
                                        self.event(ui, e, src_span, cache, options, max_width);
                                        self.line.should_start_newline = tmp_start;
                                        self.line.should_end_newline = tmp_end;
                                    }
                                });
                            }

                            ui.end_row();

                            for row in rows {
                                for col in row {
                                    ui.horizontal(|ui| {
                                        for (e, src_span) in col {
                                            let tmp_start = std::mem::replace(
                                                &mut self.line.should_start_newline,
                                                false,
                                            );
                                            let tmp_end = std::mem::replace(
                                                &mut self.line.should_end_newline,
                                                false,
                                            );
                                            self.event(ui, e, src_span, cache, options, max_width);
                                            self.line.should_start_newline = tmp_start;
                                            self.line.should_end_newline = tmp_end;
                                        }
                                    });
                                }

                                ui.end_row();
                            }
                        });
                    });
            });

            // If the active-match scroll was satisfied inside the table, the
            // scroll_to_rect call from event_text() was intercepted by the
            // horizontal scroll area (see comment above).  Re-issue it on the
            // outer `ui` using the virtual Y we recorded during the table render
            // so the outer vertical scroll area can bring the row into view.
            if want_scroll_before
                && !self.want_scroll_to_active_match
                && let Some(active_idx) = cache.active_match()
            {
                let match_virtual_y = self.search_match_ys_scratch[scratch_start..]
                    .iter()
                    .find(|(gi, _)| *gi == active_idx)
                    .map(|(_, y)| *y);
                if let Some(vy) = match_virtual_y {
                    let screen_y = self.content_origin_y + vy;
                    let line_height = ui.text_style_height(&TextStyle::Body);
                    let target_rect = egui::Rect::from_min_size(
                        egui::pos2(0.0, screen_y),
                        egui::vec2(1.0, line_height),
                    );
                    ui.scroll_to_rect(target_rect, Some(egui::Align::Center));
                }
            }

            self.is_table = false;
            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            self.line.try_insert_end(ui);
        }
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, cache, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => {
                self.event_text(text, src_span, ui, cache, options);
            }
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                self.event_text(text, src_span, ui, cache, options);
                self.text_style.code = false;
            }
            pulldown_cmark::Event::InlineHtml(text) => {
                self.event_text(text, src_span, ui, cache, options);
            }

            pulldown_cmark::Event::Html(text) => {
                if options.html_fn.is_some() {
                    self.html_block.push_str(&text);
                } else {
                    self.event_text(text, src_span, ui, cache, options);
                }
            }
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                footnote_start(ui, &footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                soft_break(ui);
            }
            pulldown_cmark::Event::HardBreak => newline(ui),
            pulldown_cmark::Event::Rule => {
                self.line.try_insert_start(ui);
                rule(ui, self.line.can_insert_end());
            }
            pulldown_cmark::Event::TaskListMarker(mut checkbox) => {
                if options.mutable {
                    if ui
                        .add(egui::Checkbox::without_text(&mut checkbox))
                        .clicked()
                    {
                        self.checkbox_events.push(CheckboxClickEvent {
                            checked: checkbox,
                            span: src_span,
                        });
                    }
                } else {
                    ui.add(ImmutableCheckbox::without_text(&mut checkbox));
                }
            }
            pulldown_cmark::Event::InlineMath(tex) => {
                if let Some(math_fn) = options.math_fn {
                    math_fn(ui, &tex, true);
                }
            }
            pulldown_cmark::Event::DisplayMath(tex) => {
                if let Some(math_fn) = options.math_fn {
                    math_fn(ui, &tex, false);
                }
            }
        }
    }

    fn event_text(
        &mut self,
        text: CowStr,
        src_span: Range<usize>,
        ui: &mut Ui,
        cache: &CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        if let Some(image) = &mut self.image {
            image.push_alt_text(self.text_style.to_richtext(ui, &text), src_span);
        } else if let Some(block) = &mut self.code_block {
            block.push_text(&text, src_span);
        } else if let Some(link) = &mut self.link {
            link.push_text(self.text_style.to_richtext(ui, &text), src_span);
        } else {
            let rich_text = self.text_style.to_richtext(ui, &text);
            let ranges = cache.search_ranges();
            if ranges.is_empty() {
                ui.label(rich_text);
                return;
            }

            let intervals =
                search_intervals(ranges, cache.active_search_range(), &src_span, text.len());
            let (_, active_rect, all_rects) = label_with_search_highlight(
                ui,
                rich_text,
                &intervals,
                options.search_match_bg(ui),
                options.search_active_match_bg(ui),
            );

            // Record the virtual-y (scroll-independent) position for each
            // global match that falls in this text run. global_match_indices[j]
            // corresponds to intervals[j] and all_rects[j]: both iterate
            // search_ranges in document order with the same filter, so the
            // j-th surviving entry is the same match in both.
            let content_origin_y = self.content_origin_y;
            let global_match_indices: Vec<usize> = cache
                .search_ranges()
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    r.start < r.end && r.start < src_span.end && r.end > src_span.start
                })
                .map(|(i, _)| i)
                .collect();
            for (global_idx, maybe_rect) in global_match_indices.iter().zip(all_rects.iter()) {
                if let Some(rect) = maybe_rect {
                    self.search_match_ys_scratch
                        .push((*global_idx, rect.min.y - content_origin_y));
                }
            }

            if self.want_scroll_to_active_match
                && let Some(rect) = active_rect
            {
                ui.scroll_to_rect(rect, Some(egui::Align::Center));
                self.want_scroll_to_active_match = false;
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
                self.line.try_insert_start(ui);
            }
            pulldown_cmark::Tag::Heading { level, id, .. } => {
                if let Some(scroll_target) = cache.scroll_to_id_target()
                    && let Some(id) = id
                    && id.into_string() == scroll_target
                {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                    cache.scroll_to_id_target_mut().take();
                }

                // Headings should always insert a newline even if it is at the start.
                // Whether this is okay in all scenarios is a different question.
                newline(ui);
                self.text_style.heading = Some(match level {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                });
            }

            // deliberately not using the built in alerts from pulldown-cmark as
            // the markdown itself cannot be localized :( e.g: [!TIP]
            pulldown_cmark::Tag::BlockQuote(_) => {
                self.is_blockquote = true;
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                match c {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: Some(lang.to_string()),
                            content: "".to_string(),
                            chunks: Vec::new(),
                        });
                    }
                    pulldown_cmark::CodeBlockKind::Indented => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: None,
                            content: "".to_string(),
                            chunks: Vec::new(),
                        });
                    }
                }
                // A code block always needs a line of its own. Inside a list
                // `should_start_newline` is off, so `try_insert_start` would do
                // nothing and the block would be laid out after the item's text,
                // leaving it only the width remaining on that line.
                if !self.line.should_not_start_newline_forced {
                    newline(ui);
                }
            }

            pulldown_cmark::Tag::List(point) => {
                if !self.list.is_inside_a_list() && self.line.can_insert_start() {
                    newline(ui);
                }

                if let Some(number) = point {
                    self.list.start_level_with_number(number);
                } else {
                    self.list.start_level_without_number();
                }
                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
            }

            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.list.start_item(ui, options);
            }

            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.line.try_insert_start(ui);

                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
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
            pulldown_cmark::Tag::Link { dest_url, .. } => {
                self.link = Some(crate::Link {
                    destination: dest_url.to_string(),
                    ..Default::default()
                });
            }
            pulldown_cmark::Tag::Image { dest_url, .. } => {
                self.image = Some(crate::Image::new(&dest_url, options));
            }
            pulldown_cmark::Tag::HtmlBlock => {
                self.line.try_insert_start(ui);
            }
            pulldown_cmark::Tag::MetadataBlock(_) => {}

            pulldown_cmark::Tag::DefinitionList => {
                self.line.try_insert_start(ui);
                self.def_list.is_first_item = true;
            }
            pulldown_cmark::Tag::DefinitionListTitle => {
                // we disable newline as the first title should not insert a newline
                // as we have already done that upon the DefinitionList Tag
                if !self.def_list.is_first_item {
                    self.line.try_insert_start(ui)
                } else {
                    self.def_list.is_first_item = false;
                }
            }
            pulldown_cmark::Tag::DefinitionListDefinition => {
                self.def_list.is_def_list_def = true;
            }
            // Not yet supported
            pulldown_cmark::Tag::Superscript | pulldown_cmark::Tag::Subscript => {}
        }
    }

    fn end_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::TagEnd,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match tag {
            pulldown_cmark::TagEnd::Paragraph => {
                self.line.try_insert_end(ui);
            }
            pulldown_cmark::TagEnd::Heading { .. } => {
                self.line.try_insert_end(ui);
                self.text_style.heading = None;
            }
            pulldown_cmark::TagEnd::BlockQuote(_) => {}
            pulldown_cmark::TagEnd::CodeBlock => {
                self.end_code_block(ui, cache, options, max_width);
            }

            pulldown_cmark::TagEnd::List(_) => {
                if self.list.is_last_level() {
                    self.line.should_start_newline = true;
                    self.line.should_end_newline = true;
                }

                self.list.end_level(ui, self.line.can_insert_end());

                if !self.list.is_inside_a_list() {
                    // Reset all the state and make it ready for the next list that occurs
                    self.list = List::default();
                }
            }
            pulldown_cmark::TagEnd::Item => {}
            pulldown_cmark::TagEnd::FootnoteDefinition => {
                self.line.should_start_newline = true;
                self.line.should_end_newline = true;
                self.line.try_insert_end(ui);
            }
            pulldown_cmark::TagEnd::Table => {}
            pulldown_cmark::TagEnd::TableHead => {}
            pulldown_cmark::TagEnd::TableRow => {}
            pulldown_cmark::TagEnd::TableCell => {
                // Ensure space between cells
                ui.label("  ");
            }
            pulldown_cmark::TagEnd::Emphasis => {
                self.text_style.emphasis = false;
            }
            pulldown_cmark::TagEnd::Strong => {
                self.text_style.strong = false;
            }
            pulldown_cmark::TagEnd::Strikethrough => {
                self.text_style.strikethrough = false;
            }
            pulldown_cmark::TagEnd::Link => {
                if let Some(link) = self.link.take() {
                    let (scrolled, match_ys) = link.end(
                        ui,
                        cache,
                        options,
                        &mut self.deferred_scroll_to_heading,
                        self.want_scroll_to_active_match,
                        self.content_origin_y,
                    );
                    if scrolled {
                        self.want_scroll_to_active_match = false;
                    }
                    self.search_match_ys_scratch.extend(match_ys);
                }
            }
            pulldown_cmark::TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    let (height, scrolled, match_ys) = image.end(
                        ui,
                        cache,
                        options,
                        self.want_scroll_to_active_match,
                        self.content_origin_y,
                    );
                    if height < 1.0 {
                        self.any_image_loading = true;
                    }
                    if scrolled {
                        self.want_scroll_to_active_match = false;
                    }
                    self.search_match_ys_scratch.extend(match_ys);
                }
            }
            pulldown_cmark::TagEnd::HtmlBlock => {
                if let Some(html_fn) = options.html_fn {
                    html_fn(ui, &self.html_block);
                    self.html_block.clear();
                }
            }

            pulldown_cmark::TagEnd::MetadataBlock(_) => {}

            pulldown_cmark::TagEnd::DefinitionList => self.line.try_insert_end(ui),
            pulldown_cmark::TagEnd::DefinitionListTitle
            | pulldown_cmark::TagEnd::DefinitionListDefinition => {}
            pulldown_cmark::TagEnd::Superscript | pulldown_cmark::TagEnd::Subscript => {}
        }
    }

    fn end_code_block(
        &mut self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        if let Some(block) = self.code_block.take() {
            let (scrolled, match_ys) = block.end(
                ui,
                cache,
                options,
                max_width,
                self.want_scroll_to_active_match,
                self.content_origin_y,
            );
            if scrolled {
                self.want_scroll_to_active_match = false;
            }
            self.search_match_ys_scratch.extend(match_ys);
            if self.line.should_end_newline_forced {
                newline(ui);
            }
        }
    }
}

fn apply_pending_scroll_delta(cache: &mut CommonMarkCache, ui: &Ui) {
    let delta = std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);
    if delta != egui::Vec2::ZERO {
        ui.scroll_with_delta(delta);
    }
}

#[cfg(test)]
mod perf_tests {
    use super::*;
    use crate::{CommonMarkCache, CommonMarkViewer};

    /// Returns the number of full-document renders recorded for `source_id`
    /// since the process started (or since the entry was first created). Each
    /// source_id has its own counter, so parallel tests do not interfere.
    fn full_render_count_for(source_id: &str) -> usize {
        let id = egui::Id::new(source_id);
        *FULL_RENDER_COUNTS.lock().unwrap().get(&id).unwrap_or(&0)
    }

    fn big_document() -> String {
        let mut text = String::new();
        for i in 1..=1024_usize {
            text += &format!(
                "\n## Section {i}\n\nThis is section {i}.\n\n```rs\nvec.push({i});\n```\n\n"
            );
        }
        text
    }

    fn windowed_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        }
    }

    /// Reproduces the reported bug: clicking Next/Previous was pathologically
    /// slow even when the target match was already on the currently visible
    /// page, because every call to `scroll_to_active_search_match` forced a
    /// full document re-render regardless of visibility.
    #[test]
    fn clicking_next_on_visible_match_is_fast() {
        let doc = big_document();
        // Three matches, all near the very top of the document (sections
        // 1-3), so they're all visible in the initial (unscrolled) viewport.
        let ranges: Vec<std::ops::Range<usize>> = [1, 2, 3]
            .iter()
            .map(|i| {
                let query = format!("This is section {i}.");
                let pos = doc.find(&query).expect("expected to find section text");
                pos..pos + query.len()
            })
            .collect();
        let mut cache = CommonMarkCache::default();
        cache.set_search_ranges(ranges.clone());
        cache.set_active_search_range(ranges.first().cloned());

        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());

        // Frame 0: cold render, populates page_size/split_points. Not timed.
        let output = ctx.run_ui(windowed_input(), |ui| {
            ui.set_min_height(600.0);
            CommonMarkViewer::new()
                .viewport_cache(true)
                .show_scrollable("perf_test_next_click", ui, &mut cache, &doc);
        });
        output.drop_without_applying_deltas();

        // Now simulate repeatedly clicking "Next", cycling among the 3
        // nearby matches. None of these require scrolling far, so each
        // should resolve within the fast viewport-only path.
        let mut worst: std::time::Duration = std::time::Duration::ZERO;
        for i in 0..12 {
            let active = &ranges[i % ranges.len()];
            cache.set_active_search_range(Some(active.clone()));
            cache.scroll_to_active_search_match();

            let click_start = std::time::Instant::now();
            let output = ctx.run_ui(windowed_input(), |ui| {
                ui.set_min_height(600.0);
                CommonMarkViewer::new()
                    .viewport_cache(true)
                    .show_scrollable("perf_test_next_click", ui, &mut cache, &doc);
            });
            output.drop_without_applying_deltas();
            worst = worst.max(click_start.elapsed());
        }

        assert!(
            worst < std::time::Duration::from_millis(500),
            "clicking Next/Previous on an on-screen match should be fast, \
             worst frame took {worst:?}"
        );
    }

    /// Jumping to a match far outside the current viewport (e.g. near the
    /// end of a 1024-section document) must converge via cheap blind-scroll
    /// nudges over a few frames, never by forcing a full document re-render.
    #[test]
    fn jumping_to_offscreen_match_does_not_force_full_render() {
        let doc = big_document();
        let query = "This is section 1000.";
        let pos = doc.find(query).expect("expected to find section text");
        let target = pos..pos + query.len();

        let mut cache = CommonMarkCache::default();
        cache.set_search_ranges(vec![target.clone()]);

        // Real fonts, not `FontDefinitions::empty()`: with empty fonts, text
        // rows collapse to a few pixels tall, packing far more pulldown-cmark
        // events into any given viewport-height window than would ever
        // happen with real font metrics, which would make the per-frame
        // timing assertion below meaningless.
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());

        // Frame 0: cold render at the top of the document, populates
        // page_size/split_points.
        let output = ctx.run_ui(windowed_input(), |ui| {
            ui.set_min_height(600.0);
            CommonMarkViewer::new()
                .viewport_cache(true)
                .show_scrollable("perf_test_offscreen_jump", ui, &mut cache, &doc);
        });
        output.drop_without_applying_deltas();

        cache.set_active_search_range(Some(target));
        cache.scroll_to_active_search_match();

        let before = full_render_count_for("perf_test_offscreen_jump");
        let mut worst = std::time::Duration::ZERO;
        for _ in 0..10 {
            let frame_start = std::time::Instant::now();
            let output = ctx.run_ui(windowed_input(), |ui| {
                ui.set_min_height(600.0);
                CommonMarkViewer::new()
                    .viewport_cache(true)
                    .show_scrollable("perf_test_offscreen_jump", ui, &mut cache, &doc);
            });
            output.drop_without_applying_deltas();
            worst = worst.max(frame_start.elapsed());
        }
        let delta = full_render_count_for("perf_test_offscreen_jump") - before;

        assert_eq!(
            delta, 0,
            "jumping to an off-screen match must not force a full re-render"
        );
        assert!(
            worst < std::time::Duration::from_millis(250),
            "every frame while converging on an off-screen match should stay cheap, \
             worst frame took {worst:?}"
        );
    }

    #[test]
    fn steady_state_search_does_not_force_full_render_every_frame() {
        let doc = big_document();
        // A handful of matches, similar to a real search with few hits.
        let query = "push(123)";
        let mut ranges = Vec::new();
        let mut start = 0;
        while let Some(pos) = doc[start..].find(query) {
            let s = start + pos;
            ranges.push(s..s + query.len());
            start = s + query.len();
        }
        assert!(
            !ranges.is_empty(),
            "test setup: expected at least one match"
        );

        let mut cache = CommonMarkCache::default();
        cache.set_search_ranges(ranges.clone());
        cache.set_active_search_range(ranges.first().cloned());
        // Note: scroll_to_active_search_match() is deliberately NOT called here;
        // we're testing the steady state where matches exist but no scroll/jump
        // has been requested (e.g. the user has merely typed a query).

        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());

        // Each test uses its own source_id, so full_render_count_for() is
        // keyed per source and cannot be polluted by other tests running in
        // parallel.
        let before = full_render_count_for("perf_test_doc");
        const FRAMES: usize = 5;
        for _ in 0..FRAMES {
            let output = ctx.run_ui(windowed_input(), |ui| {
                ui.set_min_height(600.0);
                CommonMarkViewer::new()
                    .viewport_cache(true)
                    .show_scrollable("perf_test_doc", ui, &mut cache, &doc);
            });
            output.drop_without_applying_deltas();
        }

        let delta = full_render_count_for("perf_test_doc") - before;
        assert!(
            delta <= 1,
            "expected at most 1 full render across {FRAMES} steady-state frames, got {delta}"
        );
    }
}
