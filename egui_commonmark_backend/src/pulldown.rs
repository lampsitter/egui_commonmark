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
    /// [`ScrollableCache::virtual_y_for_byte_offset`] approximate on-screen
    /// positions of arbitrary offsets (e.g. search matches) without a fresh
    /// full render.
    pub src_span: Range<usize>,
}

#[derive(Default, Debug)]
pub struct ScrollableCache {
    pub available_size: Vec2,
    pub page_size: Option<Vec2>,
    /// One [`SplitPoint`] per top-level block at a safe renderer restart
    /// boundary, in document order.
    pub split_points: Vec<SplitPoint>,
    /// Heading slug → virtual Y (content-relative; 0 = document top).
    /// Populated during the full render; used by the viewport path to
    /// scroll to headings outside the currently rendered slice.
    pub heading_y_positions: HashMap<String, f32>,
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

impl ScrollableCache {
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
    pub fn virtual_y_for_byte_offset(&self, offset: usize) -> Option<f32> {
        if let Some(sp) = self
            .split_points
            .iter()
            .find(|sp| sp.src_span.contains(&offset))
        {
            return Some(sp.vstart.y);
        }

        // Not inside any tracked block, e.g. it's inside a list/table/
        // blockquote, which aren't tracked individually. Use the nearest
        // preceding tracked block as a reasonable approximation, same as
        // `show_scrollable`'s own slice calculation does.
        self.split_points
            .iter()
            .rev()
            .find(|sp| sp.src_span.start <= offset)
            .map(|sp| sp.vstart.y)
            .or_else(|| self.split_points.first().map(|sp| sp.vstart.y))
    }

    /// The inverse of [`Self::virtual_y_for_byte_offset`]: approximate the
    /// source byte offset of whatever is at (or just before) the given
    /// virtual Y, using the same split points. Returns `None` only if there
    /// are no split points at all yet.
    pub fn byte_offset_for_virtual_y(&self, y: f32) -> Option<usize> {
        self.split_points
            .iter()
            .rev()
            .find(|sp| sp.vstart.y <= y)
            .map(|sp| sp.src_span.start)
            .or_else(|| self.split_points.first().map(|sp| sp.src_span.start))
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
