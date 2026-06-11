use std::iter::Peekable;
use std::ops::Range;
use std::{cell::RefCell, collections::HashMap};

use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, TextStyle, Ui};

use crate::List;
use egui_commonmark_backend::elements::*;
use egui_commonmark_backend::misc::*;
use egui_commonmark_backend::pulldown::*;
use pulldown_cmark::{CowStr, HeadingLevel};

/// Newline logic is constructed by the following:
/// All elements try to insert a newline before them (if they are allowed)
/// and end their own line.
#[derive(Clone)]
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

#[derive(Default, Clone)]
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
}

pub(crate) struct CheckboxClickEvent {
    pub(crate) checked: bool,
    pub(crate) span: Range<usize>,
}

const CHECKPOINT_STRIDE_EVENTS: usize = 4;
const CHECKPOINT_OVERSCAN: usize = 0;

#[derive(Clone)]
struct RenderCheckpoint {
    event_index: usize,
    start_y: f32,
    end_y: f32,
    snapshot: RenderSnapshot,
}

#[derive(Clone)]
struct RenderSnapshot {
    curr_table: usize,
    text_style: Style,
    list_numbers: Vec<Option<u64>>,
    list_has_begun: bool,
    line: Newline,
    is_list_item: bool,
    def_list: DefinitionList,
    is_table: bool,
    is_blockquote: bool,
}

#[derive(Default)]
struct LocalVirtualizationCache {
    checkpoints: Vec<RenderCheckpoint>,
    page_height: f32,
    text_ptr: usize,
    text_len: usize,
    parser_options_bits: u32,
    available_width_bits: u32,
    just_changed: bool,
}

thread_local! {
    static LOCAL_VIRTUAL_CACHE: RefCell<HashMap<Id, LocalVirtualizationCache>> = RefCell::new(HashMap::new());
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
        }
    }

    fn can_checkpoint(&self) -> bool {
        self.link.is_none()
            && self.image.is_none()
            && self.code_block.is_none()
            && self.html_block.is_empty()
    }

    fn snapshot(&self) -> RenderSnapshot {
        let (list_numbers, list_has_begun) = self.list.checkpoint();
        RenderSnapshot {
            curr_table: self.curr_table,
            text_style: self.text_style.clone(),
            list_numbers,
            list_has_begun,
            line: self.line.clone(),
            is_list_item: self.is_list_item,
            def_list: self.def_list.clone(),
            is_table: self.is_table,
            is_blockquote: self.is_blockquote,
        }
    }

    fn restore_from_snapshot(&mut self, snapshot: &RenderSnapshot) {
        self.curr_table = snapshot.curr_table;
        self.text_style = snapshot.text_style.clone();
        self.list = List::from_checkpoint(snapshot.list_numbers.clone(), snapshot.list_has_begun);
        self.line = snapshot.line.clone();
        self.is_list_item = snapshot.is_list_item;
        self.def_list = snapshot.def_list.clone();
        self.is_table = snapshot.is_table;
        self.is_blockquote = snapshot.is_blockquote;
        self.link = None;
        self.image = None;
        self.code_block = None;
        self.html_block.clear();
        self.checkbox_events.clear();
        self.deferred_scroll_to_heading = None;
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

fn clamp_to_char_boundary(text: &str, mut index: usize) -> usize {
    if index > text.len() {
        index = text.len();
    }
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn fence_marker(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent = line.len().saturating_sub(trimmed.len());
    if indent > 3 {
        return None;
    }
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let marker = bytes[0];
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let run_len = bytes.iter().take_while(|b| **b == marker).count();
    if run_len >= 3 {
        Some((marker, run_len))
    } else {
        None
    }
}

fn inside_unclosed_fence(prefix: &str) -> bool {
    let mut current_fence: Option<(u8, usize)> = None;
    for line in prefix.lines() {
        if let Some((marker, run_len)) = fence_marker(line) {
            if let Some((open_marker, open_len)) = current_fence {
                if marker == open_marker && run_len >= open_len {
                    current_fence = None;
                }
            } else {
                current_fence = Some((marker, run_len));
            }
        }
    }
    current_fence.is_some()
}

fn incremental_reparse_start(text: &str, previous_len: usize) -> usize {
    // Be conservative for correctness: reparse from the last blank-line boundary
    // in a larger context window, since markdown block semantics can depend on
    // lines before the append point.
    let context_start = clamp_to_char_boundary(text, previous_len.saturating_sub(64 * 1024));
    let window = &text[context_start..previous_len];
    let candidate = if let Some(blank_line_pos) = window.rfind("\n\n") {
        context_start + blank_line_pos + 2
    } else if let Some(last_newline) = window.as_bytes().iter().rposition(|b| *b == b'\n') {
        context_start + last_newline + 1
    } else {
        0
    };

    if candidate > 0 && inside_unclosed_fence(&text[..candidate]) {
        0
    } else {
        candidate
    }
}

fn width_cache_key(width: f32) -> u32 {
    width.max(0.0).round() as u32
}

fn appended_text_needs_full_reparse(appended: &str) -> bool {
    // Reference-style link / footnote definitions appended later can affect
    // earlier inline parsing, so incremental tail-only reparsing is unsafe.
    appended.lines().any(|line| {
        let trimmed = line.trim_start_matches([' ', '\t']);
        trimmed.starts_with('[') && trimmed.contains("]:")
    })
}

fn try_incremental_append_parse(
    scroll_cache: &mut ScrollableCache,
    text: &str,
    parser_options: pulldown_cmark::Options,
    text_len: usize,
) -> Option<usize> {
    if scroll_cache.parsed_events.is_empty()
        || scroll_cache.parser_options_bits != parser_options.bits()
        || text_len <= scroll_cache.parsed_text_len
    {
        return None;
    }

    let previous_len = scroll_cache.parsed_text_len;
    if appended_text_needs_full_reparse(&text[previous_len..text_len]) {
        return None;
    }
    let reparse_start = incremental_reparse_start(text, previous_len);
    scroll_cache
        .parsed_events
        .retain(|(_, span)| span.end <= reparse_start);
    let changed_event_start = scroll_cache.parsed_events.len();

    let reparsed_tail = pulldown_cmark::Parser::new_ext(&text[reparse_start..], parser_options)
        .into_offset_iter()
        .map(|(event, span)| {
            (
                event.into_static(),
                (span.start + reparse_start)..(span.end + reparse_start),
            )
        });
    scroll_cache.parsed_events.extend(reparsed_tail);
    scroll_cache.parsed_text_len = text_len;
    Some(changed_event_start)
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
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        let re = ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);
            let parser_options =
                parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading);
            let text_ptr = text.as_ptr() as usize;
            let text_len = text.len();
            let mut collect_checkpoints = false;
            let mut top_spacer_height = 0.0_f32;
            let mut bottom_spacer_height = 0.0_f32;
            let mut restore_snapshot = None;
            let mut collected_checkpoints = Vec::<RenderCheckpoint>::new();
            let first_position_y = ui.next_widget_position().y;
            let (indexed_events, total_event_count, text_changed_this_frame) =
                if let Some(source_id) = split_points_id {
                    let available_size = ui.available_size();
                    let viewport = ui.clip_rect();
                    let scroll_cache = scroll_cache(cache, &source_id);
                    let size_changed = (scroll_cache.available_size.x - available_size.x).abs()
                        > 0.5
                        || (scroll_cache.available_size.y - available_size.y).abs() > 0.5;
                    let mut changed_event_start = None;

                    let cache_is_stale = scroll_cache.parsed_text_ptr != text_ptr
                        || scroll_cache.parsed_text_len != text_len
                        || scroll_cache.parser_options_bits != parser_options.bits()
                        || scroll_cache.parsed_events.is_empty()
                        || size_changed;

                    if cache_is_stale {
                        changed_event_start = try_incremental_append_parse(
                            scroll_cache,
                            text,
                            parser_options,
                            text_len,
                        );
                        if changed_event_start.is_none() {
                            scroll_cache.parsed_events =
                                pulldown_cmark::Parser::new_ext(text, parser_options)
                                    .into_offset_iter()
                                    .map(|(event, span)| (event.into_static(), span))
                                    .collect();
                            changed_event_start = Some(0);
                        }
                        scroll_cache.parsed_text_ptr = text_ptr;
                        scroll_cache.parsed_text_len = text_len;
                        scroll_cache.parser_options_bits = parser_options.bits();
                        scroll_cache.page_size = None;
                        scroll_cache.split_points.clear();
                    }

                    let total_events = scroll_cache.parsed_events.len();
                    let available_width_bits = width_cache_key(available_size.x);
                    let local = LOCAL_VIRTUAL_CACHE.with(|cache_by_id| {
                        let mut cache_by_id = cache_by_id.borrow_mut();
                        cache_by_id.remove(&source_id).unwrap_or_default()
                    });
                    let text_changed = local.text_ptr != text_ptr || local.text_len != text_len;
                    let text_appended = text_len > local.text_len;
                    let stale_non_append_text = text_len < local.text_len
                        || (text_len == local.text_len && local.text_ptr != text_ptr);
                    let stale_parser = local.parser_options_bits != parser_options.bits();
                    let stale_width = local.available_width_bits != available_width_bits;
                    let stale_checkpoints = local.checkpoints.len() < 2;
                    let stale_just_changed = !text_changed && local.just_changed;
                    let local_is_stale = stale_non_append_text
                        || stale_parser
                        || stale_width
                        || stale_checkpoints
                        || stale_just_changed;
                    let mut partial_rebuild_state: Option<(usize, Vec<RenderCheckpoint>)> = None;
                    let mut append_offscreen_safe_reuse = false;
                    if text_changed && text_appended && !local_is_stale {
                        if let Some(changed_start) = changed_event_start {
                            let viewport_max = viewport.max.y - first_position_y;
                            let near_bottom = viewport_max >= (local.page_height - 1.0);
                            let viewport_end_checkpoint_index = local
                                .checkpoints
                                .iter()
                                .position(|checkpoint| checkpoint.start_y >= viewport_max)
                                .unwrap_or(local.checkpoints.len().saturating_sub(1))
                                .saturating_add(CHECKPOINT_OVERSCAN)
                                .min(local.checkpoints.len().saturating_sub(1));
                            let viewport_end_event_index = local.checkpoints
                                [viewport_end_checkpoint_index]
                                .event_index
                                .min(total_events);
                            let safety_margin_events = CHECKPOINT_STRIDE_EVENTS.saturating_mul(2);
                            append_offscreen_safe_reuse = !near_bottom
                                && changed_start
                                    > viewport_end_event_index.saturating_add(safety_margin_events);
                            let changed_rebuild_pos = local
                                .checkpoints
                                .iter()
                                .rposition(|checkpoint| checkpoint.event_index <= changed_start);
                            let viewport_min = viewport.min.y - first_position_y;
                            let viewport_rebuild_pos = local
                                .checkpoints
                                .iter()
                                .rposition(|checkpoint| checkpoint.end_y <= viewport_min)
                                .unwrap_or(0)
                                .saturating_sub(CHECKPOINT_OVERSCAN);
                            if let Some(changed_rebuild_pos) = changed_rebuild_pos
                                && !append_offscreen_safe_reuse
                            {
                                let rebuild_pos = changed_rebuild_pos.min(viewport_rebuild_pos);
                                let rebuild_checkpoint = local.checkpoints[rebuild_pos].clone();
                                top_spacer_height = rebuild_checkpoint.start_y.max(0.0);
                                restore_snapshot = Some(rebuild_checkpoint.snapshot.clone());
                                let prefix = local.checkpoints[..=rebuild_pos].to_vec();
                                partial_rebuild_state =
                                    Some((rebuild_checkpoint.event_index, prefix));
                            }
                        }
                    }

                    let events = if local_is_stale
                        || (text_changed
                            && partial_rebuild_state.is_none()
                            && !append_offscreen_safe_reuse)
                    {
                        collect_checkpoints = true;
                        collected_checkpoints.push(RenderCheckpoint {
                            event_index: 0,
                            start_y: 0.0,
                            end_y: 0.0,
                            snapshot: self.snapshot(),
                        });
                        scroll_cache.available_size = available_size;
                        scroll_cache
                            .parsed_events
                            .iter()
                            .cloned()
                            .enumerate()
                            .collect::<Vec<_>>()
                    } else if let Some((rebuild_start_event_index, prefix_checkpoints)) =
                        partial_rebuild_state
                    {
                        collect_checkpoints = true;
                        collected_checkpoints = prefix_checkpoints;
                        scroll_cache.available_size = available_size;
                        scroll_cache
                            .parsed_events
                            .iter()
                            .cloned()
                            .enumerate()
                            .skip(rebuild_start_event_index)
                            .collect::<Vec<_>>()
                    } else {
                        let checkpoints = local.checkpoints;
                        let viewport_min = viewport.min.y - first_position_y;
                        let viewport_max = viewport.max.y - first_position_y;
                        let start_checkpoint_index = checkpoints
                            .iter()
                            .rposition(|checkpoint| checkpoint.end_y <= viewport_min)
                            .unwrap_or(0)
                            .saturating_sub(CHECKPOINT_OVERSCAN);

                        let end_checkpoint_index = checkpoints
                            .iter()
                            .position(|checkpoint| checkpoint.start_y >= viewport_max)
                            .unwrap_or(checkpoints.len().saturating_sub(1))
                            .saturating_add(CHECKPOINT_OVERSCAN)
                            .min(checkpoints.len().saturating_sub(1));

                        let start_checkpoint = &checkpoints[start_checkpoint_index];
                        let end_checkpoint = &checkpoints[end_checkpoint_index];
                        let start_event_index = start_checkpoint.event_index.min(total_events);
                        let render_to_tail = text_appended && !append_offscreen_safe_reuse;
                        let end_event_index = if render_to_tail {
                            total_events
                        } else {
                            end_checkpoint
                                .event_index
                                .max(start_event_index)
                                .min(total_events)
                        };

                        restore_snapshot = Some(start_checkpoint.snapshot.clone());
                        top_spacer_height = start_checkpoint.start_y.max(0.0);
                        bottom_spacer_height = if render_to_tail {
                            0.0
                        } else {
                            (local.page_height - end_checkpoint.start_y).max(0.0)
                        };

                        let selected = scroll_cache
                            .parsed_events
                            .iter()
                            .cloned()
                            .enumerate()
                            .skip(start_event_index)
                            .take(end_event_index.saturating_sub(start_event_index))
                            .collect::<Vec<_>>();
                        LOCAL_VIRTUAL_CACHE.with(|cache_by_id| {
                            cache_by_id.borrow_mut().insert(
                                source_id,
                                LocalVirtualizationCache {
                                    checkpoints,
                                    page_height: local.page_height,
                                    text_ptr,
                                    text_len,
                                    parser_options_bits: parser_options.bits(),
                                    available_width_bits,
                                    just_changed: false,
                                },
                            );
                        });

                        selected
                    };

                    (events, total_events, text_changed)
                } else {
                    let events = pulldown_cmark::Parser::new_ext(text, parser_options)
                        .into_offset_iter()
                        .map(|(event, span)| (event.into_static(), span))
                        .enumerate()
                        .collect::<Vec<_>>();
                    let total_events = events.len();
                    (events, total_events, false)
                };

            if let Some(snapshot) = restore_snapshot.as_ref() {
                self.restore_from_snapshot(snapshot);
            }

            if top_spacer_height > 0.0 {
                ui.allocate_space(egui::vec2(0.0, top_spacer_height));
            }

            let mut events = indexed_events.into_iter().peekable();

            while let Some((index, (e, src_span))) = events.next() {
                let start_position = ui.next_widget_position();
                let should_checkpoint = collect_checkpoints
                    && index > 0
                    && index % CHECKPOINT_STRIDE_EVENTS == 0
                    && collected_checkpoints
                        .last()
                        .is_none_or(|checkpoint| checkpoint.event_index != index)
                    && self.can_checkpoint();
                if should_checkpoint {
                    collected_checkpoints.push(RenderCheckpoint {
                        event_index: index,
                        start_y: start_position.y - first_position_y,
                        end_y: start_position.y - first_position_y,
                        snapshot: self.snapshot(),
                    });
                }

                if events.peek().is_none() && index + 1 == total_event_count {
                    self.line.should_end_newline_forced = false;
                }

                self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                if index == 0 {
                    self.line.should_not_start_newline_forced = false;
                }
            }
            if bottom_spacer_height > 0.0 {
                ui.allocate_space(egui::vec2(0.0, bottom_spacer_height));
            }

            // deferral to make it consistent no matter whether the target is before or after the link
            *cache.scroll_to_id_target_mut() = self.deferred_scroll_to_heading.take();

            if let Some(source_id) = split_points_id {
                if collect_checkpoints {
                    let page_size = ui.next_widget_position().to_vec2();
                    let page_height = (page_size.y - first_position_y).max(0.0);
                    let scroll_cache = scroll_cache(cache, &source_id);
                    scroll_cache.page_size = Some(page_size);
                    scroll_cache.split_points.clear();

                    let total_events = scroll_cache.parsed_events.len();
                    let mut checkpoints = collected_checkpoints;
                    let needs_sentinel = checkpoints
                        .last()
                        .is_none_or(|checkpoint| checkpoint.event_index != total_events);
                    if needs_sentinel {
                        checkpoints.push(RenderCheckpoint {
                            event_index: total_events,
                            start_y: page_height,
                            end_y: page_height,
                            snapshot: self.snapshot(),
                        });
                    }
                    for i in 0..checkpoints.len() {
                        let next_start = checkpoints
                            .get(i + 1)
                            .map_or(page_height, |next| next.start_y);
                        checkpoints[i].end_y = next_start;
                    }

                    let available_width_bits = width_cache_key(scroll_cache.available_size.x);
                    LOCAL_VIRTUAL_CACHE.with(|cache_by_id| {
                        cache_by_id.borrow_mut().insert(
                            source_id,
                            LocalVirtualizationCache {
                                checkpoints,
                                page_height,
                                text_ptr,
                                text_len,
                                parser_options_bits: parser_options.bits(),
                                available_width_bits,
                                just_changed: text_changed_this_frame,
                            },
                        );
                    });
                } else if restore_snapshot.is_some() {
                    let scroll_cache = scroll_cache(cache, &source_id);
                    if scroll_cache.page_size.is_none() {
                        scroll_cache.page_size = Some(ui.next_widget_position().to_vec2());
                    }
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

        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show(ui, |ui| {
                self.show(ui, cache, options, text, Some(source_id));
            });

        // Forcing full re-render to repopulate split points for the new size
        let scroll_cache = scroll_cache(cache, &source_id);
        if (available_size.x - scroll_cache.available_size.x).abs() > 0.5
            || (available_size.y - scroll_cache.available_size.y).abs() > 0.5
        {
            scroll_cache.available_size = available_size;
            scroll_cache.page_size = None;
            scroll_cache.split_points.clear();
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

            // Proccess a single event separately so that we do not insert spaces where we do not
            // want them
            self.line.should_start_newline = false;
            if let Some((_, (e, src_span))) = events_iter.next() {
                self.process_event(ui, &mut events_iter, e, src_span, cache, options, max_width);
            }

            ui.label(" ".repeat(options.indentation_spaces));
            self.line.should_start_newline = true;
            self.line.should_end_newline = false;
            // Required to ensure that the content is aligned with the identation
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
        max_width: f32,
    ) {
        if self.is_table {
            self.line.try_insert_start(ui);

            let id = ui.id().with("_table").with(self.curr_table);
            self.curr_table += 1;

            egui::Frame::group(ui.style()).show(ui, |ui| {
                let Table { header, rows } = parse_table(events);

                egui::Grid::new(id).striped(true).show(ui, |ui| {
                    for col in header {
                        ui.horizontal(|ui| {
                            for (e, src_span) in col {
                                let tmp_start =
                                    std::mem::replace(&mut self.line.should_start_newline, false);
                                let tmp_end =
                                    std::mem::replace(&mut self.line.should_end_newline, false);
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
                                    let tmp_end =
                                        std::mem::replace(&mut self.line.should_end_newline, false);
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
                    link.end(ui, cache, options, &mut self.deferred_scroll_to_heading);
                }
            }
            pulldown_cmark::TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    image.end(ui, options);
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
