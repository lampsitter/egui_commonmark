use std::iter::Peekable;
use std::ops::Range;

use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, TextStyle, Ui};

use crate::List;
use egui_commonmark_backend::elements::*;
use egui_commonmark_backend::misc::*;
use egui_commonmark_backend::pulldown::*;
use egui_commonmark_backend::table::{TableLayout, align_from_alignment, cell_text, measure_cell};
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
    table_alignments: Vec<pulldown_cmark::Alignment>,
    is_blockquote: bool,
    checkbox_events: Vec<CheckboxClickEvent>,
    deferred_scroll_to_heading: Option<String>,
    /// Set during a full render if any image rendered at zero height (texture
    /// still loading). When true, split points are discarded and the full render
    /// repeats next frame until all images have stable heights.
    any_image_loading: bool,
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
            table_alignments: Vec::new(),
            is_blockquote: false,
            checkbox_events: Vec::new(),
            deferred_scroll_to_heading: None,
            any_image_loading: false,
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
        self.any_image_loading = false;
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        let re = ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

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

            // Cursor at the visual top of the current block, captured at
            // Start(Block) so that vstart reflects the block top, not its bottom.
            let mut block_start_position: Option<Pos2> = None;

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
                }

                // Record virtual y for each named heading so the viewport path
                // can jump to headings that are outside the rendered slice.
                if let (
                    Some(sid),
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading {
                        id: Some(id), ..
                    }),
                ) = (split_points_id, &e)
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

                self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                if let Some(source_id) = split_points_id
                    && is_safe_block_end
                {
                    let scroll_cache = scroll_cache(cache, &source_id);
                    let end_position = ui.next_widget_position();

                    let split_point_exists = scroll_cache
                        .split_points
                        .iter()
                        .any(|(i, _, _)| *i == index);

                    if !split_point_exists {
                        // Use block_start_position (Start event) not start_position
                        // (cursor just before End) so that vstart is the block top.
                        let raw_vstart = block_start_position.take().unwrap_or(start_position);
                        let vstart = egui::pos2(raw_vstart.x, raw_vstart.y - content_origin_y);
                        let vend = egui::pos2(end_position.x, end_position.y - content_origin_y);
                        scroll_cache.split_points.push((index, vstart, vend));
                    }
                }

                if index == 0 {
                    self.line.should_not_start_newline_forced = false;
                }
            }

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
            }
        });

        (re, std::mem::take(&mut self.checkbox_events))
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
            // Simple path: render the full document every frame; egui clips
            // what is off-screen. Clears any stale cache from a previous run.
            {
                let sc = scroll_cache(cache, &source_id);
                sc.page_size = None;
                sc.split_points.clear();
                sc.heading_y_positions.clear();
            }
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.show(ui, cache, options, text, None);
                    apply_pending_scroll_delta(cache, ui);
                });
            return;
        }

        let Some(page_size) = scroll_cache(cache, &source_id).page_size else {
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.show(ui, cache, options, text, Some(source_id));
                    apply_pending_scroll_delta(cache, ui);
                });
            scroll_cache(cache, &source_id).available_size = available_size;
            return;
        };

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
            let slug_owned = cache.scroll_to_id_target().map(|s| s.to_owned());
            if let Some(ref slug) = slug_owned {
                let sc = scroll_cache(cache, &source_id);
                if let Some(&y) = sc.heading_y_positions.get(slug) {
                    cache.scroll_to_id_target_mut().take();
                    Some(y)
                } else {
                    None
                }
            } else {
                None
            }
        };
        let pending_delta = std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);

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
                        let preceding_split = scroll_cache
                            .split_points
                            .iter()
                            .rfind(|(_, _, vend)| vend.y < viewport.min.y)
                            .copied();
                        let (_first_event_index, _, first_end_position) =
                            preceding_split.unwrap_or((0, Pos2::ZERO, Pos2::ZERO));
                        let last_event_index = scroll_cache
                            .split_points
                            .iter()
                            .find(|(_, vstart, _)| vstart.y > render_below)
                            .map(|(index, _, _)| *index)
                            .unwrap_or(num_rows);
                        let skip_height = first_end_position.y.max(0.0);
                        // When a preceding split was found, its End(Block) is already
                        // accounted for in skip_height — re-processing it would add a
                        // duplicate newline. Start from the next event instead.
                        let (skip_count, take_count) = if let Some((idx, _, _)) = preceding_split {
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
                        *cache.scroll_to_id_target_mut() = self.deferred_scroll_to_heading.take();
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

        // Invalidate the cache when the available size changes (e.g. window resize).
        let scroll_cache = scroll_cache(cache, &source_id);
        if available_size != scroll_cache.available_size {
            scroll_cache.available_size = available_size;
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
        self.table(events, cache, options, ui);
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
            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                egui_commonmark_backend::alert_ui(alert, ui, |ui| {
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                })
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
    ) {
        if !self.is_table {
            return;
        }

        self.line.try_insert_start(ui);

        let id = ui.id().with("_table").with(self.curr_table);
        self.curr_table += 1;

        let Table { header, rows } = parse_table(events);
        let aligns: Vec<egui::Align> = std::mem::take(&mut self.table_alignments)
            .into_iter()
            .map(align_from_alignment)
            .collect();

        let header_style = Style {
            strong: true,
            ..self.text_style.clone()
        };
        let measure_row = |ui: &Ui, row: &Row<'_>, style: &Style| -> Vec<Option<f32>> {
            row.iter()
                .map(|cell| measure_cell(ui, &cell_text(ui, style, cell)))
                .collect()
        };
        let header_widths = measure_row(ui, &header, &header_style);
        let row_widths: Vec<Vec<Option<f32>>> = rows
            .iter()
            .map(|row| measure_row(ui, row, &self.text_style))
            .collect();

        let num_columns = header.len();
        let mut natural_widths = vec![0.0_f32; num_columns];
        for widths in std::iter::once(&header_widths).chain(&row_widths) {
            for (natural, width) in std::iter::zip(&mut natural_widths, widths) {
                *natural = natural.max(width.unwrap_or(0.0));
            }
        }

        let available_width = ui.available_width();
        let mut layout = TableLayout::new(ui, &natural_widths, aligns, available_width);
        layout.show(ui, id, |layout, ui| {
            layout.row(ui, true, |layout, ui| {
                for (cell, width) in std::iter::zip(header, header_widths) {
                    layout.cell(ui, width, |ui| {
                        let was_strong = self.text_style.strong;
                        self.text_style.strong = true;
                        self.table_cell_contents(ui, cell, cache, options, true);
                        self.text_style.strong = was_strong;
                    });
                }
            });

            for (row, widths) in std::iter::zip(rows, row_widths) {
                layout.row(ui, false, |layout, ui| {
                    for (cell, width) in std::iter::zip(row, widths) {
                        layout.cell(ui, width, |ui| {
                            self.table_cell_contents(ui, cell, cache, options, false);
                        });
                    }
                });
            }
        });

        self.is_table = false;
        if events.peek().is_none() {
            self.line.should_end_newline_forced = false;
        }

        self.line.try_insert_end(ui);
    }

    /// Render the events of one table cell, without the newlines that would
    /// normally surround block elements.
    fn table_cell_contents(
        &mut self,
        ui: &mut Ui,
        cell: Column<'_>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        is_header: bool,
    ) {
        let max_width = ui.max_rect().width();
        for (e, src_span) in cell {
            let tmp_start = std::mem::replace(&mut self.line.should_start_newline, false);
            let tmp_end = std::mem::replace(&mut self.line.should_end_newline, false);
            self.event(ui, e, src_span, cache, options, max_width);
            self.line.should_start_newline = tmp_start;
            self.line.should_end_newline = tmp_end;
            if is_header {
                // Header cells stay strong even after inline `**bold**` ends.
                self.text_style.strong = true;
            }
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
                self.event_text(text, ui);
            }
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                self.event_text(text, ui);
                self.text_style.code = false;
            }
            pulldown_cmark::Event::InlineHtml(text) => {
                self.event_text(text, ui);
            }

            pulldown_cmark::Event::Html(text) => {
                if options.html_fn.is_some() {
                    self.html_block.push_str(&text);
                } else {
                    self.event_text(text, ui);
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

    fn event_text(&mut self, text: CowStr, ui: &mut Ui) {
        let rich_text = self.text_style.to_richtext(ui, &text);
        if let Some(image) = &mut self.image {
            image.alt_text.push(rich_text);
        } else if let Some(block) = &mut self.code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            link.text.push(rich_text);
        } else {
            ui.label(rich_text);
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
                        });
                    }
                    pulldown_cmark::CodeBlockKind::Indented => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: None,
                            content: "".to_string(),
                        });
                    }
                }
                self.line.try_insert_start(ui);
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
            pulldown_cmark::Tag::Table(alignments) => {
                self.is_table = true;
                self.table_alignments = alignments;
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
                    text: Vec::new(),
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
            pulldown_cmark::TagEnd::TableCell => {}
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
                    link.end(ui, cache, options, &mut self.deferred_scroll_to_heading);
                }
            }
            pulldown_cmark::TagEnd::Image => {
                if let Some(image) = self.image.take()
                    && image.end(ui, options) < 1.0
                {
                    self.any_image_loading = true;
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
            block.end(ui, cache, options, max_width);
            self.line.try_insert_end(ui);
        }
    }
}

fn apply_pending_scroll_delta(cache: &mut CommonMarkCache, ui: &mut Ui) {
    let delta = std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);
    if delta != egui::Vec2::ZERO {
        ui.scroll_with_delta(delta);
    }
}
