//! Utilities for highlighting search matches inside rendered markdown.
//!
//! The design goal here is that highlighting a search match must never change
//! the number (or order) of widgets that get rendered. If it did, egui's
//! auto-generated widget IDs for every widget *after* the affected text would
//! shift whenever the search state changed, which can trigger
//! `warn_if_rect_changes_id` false positives (same on-screen rect, different
//! ID, because the widget counter sequence changed while the rect did not).
//!
//! Instead, matches are painted as backgrounds baked directly into the
//! [`egui::text::LayoutJob`] that is already being built for a run of text
//! (be that a body-text label or a syntax-highlighted code block). This is a
//! purely visual change: the same widgets are created regardless of how many
//! matches they contain.

use egui::Color32;
use egui::epaint::text::ByteRangeExt;
use egui::text::{ByteIndex, LayoutJob, LayoutSection};
use std::ops::Range;

/// The default background colour for a passive (non-active) search match:
/// the current theme's warning colour, semi-transparent so that it still
/// lets any existing syntax colour show through. Themed (rather than a fixed
/// literal) so that custom [`egui::Visuals`] are respected; callers that want
/// something else can override it (see
/// [`crate::misc::CommonMarkOptions::search_match_bg`]).
pub fn default_match_bg(visuals: &egui::Visuals) -> Color32 {
    with_alpha(visuals.warn_fg_color, 90)
}

/// The default background colour for the currently active (focused) search
/// match: the current theme's error colour (more attention-grabbing than the
/// passive match colour), semi-transparent. See
/// [`crate::misc::CommonMarkOptions::search_active_match_bg`] for overriding
/// it.
pub fn default_active_match_bg(visuals: &egui::Visuals) -> Color32 {
    with_alpha(visuals.error_fg_color, 150)
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

/// Given the set of global search match byte ranges (in the original source
/// text) and the byte span of some text run within that source, compute the
/// list of intervals *local* to that text run (0-based, relative to
/// `src_span.start`) that should be highlighted, along with whether each one
/// is the active match.
///
/// Returned intervals are sorted by start position and clamped to
/// `0..text_len`. Empty intervals are omitted.
pub fn search_intervals(
    ranges: &[Range<usize>],
    active: Option<&Range<usize>>,
    src_span: &Range<usize>,
    text_len: usize,
) -> Vec<(Range<usize>, bool)> {
    let mut intervals: Vec<(Range<usize>, bool)> = ranges
        .iter()
        .filter(|r| r.start < r.end && r.start < src_span.end && r.end > src_span.start)
        .filter_map(|r| {
            let start = r.start.saturating_sub(src_span.start).min(text_len);
            let end = r.end.saturating_sub(src_span.start).min(text_len);
            if start < end {
                let is_active = active.is_some_and(|a| a.start == r.start && a.end == r.end);
                Some((start..end, is_active))
            } else {
                None
            }
        })
        .collect();

    intervals.sort_by_key(|(r, _)| r.start);
    intervals
}

/// Rewrite `job`'s sections so that any byte range covered by `intervals`
/// gets its `egui::TextFormat::background` set to `match_bg` (or
/// `active_bg` for the active match), splitting existing sections at the
/// interval boundaries as needed. All other formatting (font, color,
/// italics, etc.) is preserved unchanged.
///
/// Does nothing if there are no intervals.
pub fn apply_search_highlights(
    job: &mut LayoutJob,
    intervals: &[(Range<usize>, bool)],
    match_bg: Color32,
    active_bg: Color32,
) {
    if intervals.is_empty() || job.sections.is_empty() {
        return;
    }

    let text_len = job.text.len();
    let mut new_sections = Vec::with_capacity(job.sections.len() + intervals.len() * 2);

    for section in job.sections.drain(..) {
        let sec_range = section.byte_range.as_usize();
        let sec_start = sec_range.start.min(text_len);
        let sec_end = sec_range.end.min(text_len);
        if sec_start >= sec_end {
            continue;
        }

        let mut points = vec![sec_start, sec_end];
        for (range, _) in intervals {
            if range.start > sec_start && range.start < sec_end {
                points.push(range.start);
            }
            if range.end > sec_start && range.end < sec_end {
                points.push(range.end);
            }
        }
        points.sort_unstable();
        points.dedup();

        for pair in points.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            if start >= end {
                continue;
            }

            let mut format = section.format.clone();
            if let Some((_, is_active)) = intervals
                .iter()
                .find(|(range, _)| range.start <= start && range.end >= end)
            {
                format.background = if *is_active { active_bg } else { match_bg };
            }

            let leading_space = if start == sec_start {
                section.leading_space
            } else {
                0.0
            };

            new_sections.push(LayoutSection {
                leading_space,
                byte_range: ByteIndex(start)..ByteIndex(end),
                format,
            });
        }
    }

    job.sections = new_sections;
}

/// Like [`search_intervals`], but for content assembled from multiple
/// markdown text events (`chunks`), each with its own local byte range
/// within the final rendered text and source byte range in the original
/// document. Used for both code blocks and link text.
pub fn chunked_search_intervals(
    chunks: &[(Range<usize>, Range<usize>)],
    ranges: &[Range<usize>],
    active: Option<&Range<usize>>,
) -> Vec<(Range<usize>, bool)> {
    let mut out = Vec::new();

    for (local_chunk, src_chunk) in chunks {
        let chunk_len = local_chunk.end.saturating_sub(local_chunk.start);
        let sub = search_intervals(ranges, active, src_chunk, chunk_len);
        out.extend(sub.into_iter().map(|(r, is_active)| {
            (
                (r.start + local_chunk.start)..(r.end + local_chunk.start),
                is_active,
            )
        }));
    }

    out.sort_by_key(|(r, _)| r.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overlap_yields_no_intervals() {
        let ranges = vec![10..15];
        let intervals = search_intervals(&ranges, None, &(0..5), 5);
        assert!(intervals.is_empty());
    }

    #[test]
    fn fully_contained_range_is_translated_to_local_offsets() {
        let ranges = vec![12..15];
        // src_span 10..20 -> local text "0123456789"
        let intervals = search_intervals(&ranges, None, &(10..20), 10);
        assert_eq!(intervals, vec![(2..5, false)]);
    }

    #[test]
    fn range_overlapping_start_is_clipped() {
        let ranges = vec![5..15];
        let intervals = search_intervals(&ranges, None, &(10..20), 10);
        assert_eq!(intervals, vec![(0..5, false)]);
    }

    #[test]
    fn range_overlapping_end_is_clipped() {
        let ranges = vec![15..25];
        let intervals = search_intervals(&ranges, None, &(10..20), 10);
        assert_eq!(intervals, vec![(5..10, false)]);
    }

    #[test]
    fn active_range_is_flagged() {
        let ranges = vec![12..15, 16..18];
        let active = 16..18;
        let intervals = search_intervals(&ranges, Some(&active), &(10..20), 10);
        assert_eq!(intervals, vec![(2..5, false), (6..8, true)]);
    }

    #[test]
    fn apply_highlights_splits_sections() {
        let mut job = LayoutJob::default();
        job.append("hello world", 0.0, egui::TextFormat::default());

        let intervals = vec![(2..5, false)];
        apply_search_highlights(
            &mut job,
            &intervals,
            Color32::YELLOW,
            Color32::from_rgb(255, 140, 0),
        );

        assert_eq!(job.sections.len(), 3);
        assert_eq!(job.sections[0].byte_range.as_usize(), 0..2);
        assert_eq!(job.sections[1].byte_range.as_usize(), 2..5);
        assert_eq!(job.sections[2].byte_range.as_usize(), 5..11);
        assert_eq!(job.sections[1].format.background, Color32::YELLOW);
        assert_eq!(job.sections[0].format.background, Color32::TRANSPARENT);
        assert_eq!(job.sections[2].format.background, Color32::TRANSPARENT);
    }

    #[test]
    fn apply_highlights_handles_interval_spanning_multiple_sections() {
        let mut job = LayoutJob::default();
        job.append("foo", 0.0, egui::TextFormat::default());
        job.append(
            "bar",
            0.0,
            egui::TextFormat {
                italics: true,
                ..Default::default()
            },
        );

        // Highlight "ob" which spans across the section boundary at byte 3.
        let intervals = vec![(2..4, true)];
        apply_search_highlights(&mut job, &intervals, Color32::YELLOW, Color32::RED);

        assert_eq!(job.sections.len(), 4);
        assert_eq!(job.sections[0].byte_range.as_usize(), 0..2); // "fo"
        assert_eq!(job.sections[1].byte_range.as_usize(), 2..3); // "o"
        assert_eq!(job.sections[2].byte_range.as_usize(), 3..4); // "b"
        assert_eq!(job.sections[3].byte_range.as_usize(), 4..6); // "ar"
        assert_eq!(job.sections[1].format.background, Color32::RED);
        assert_eq!(job.sections[2].format.background, Color32::RED);
        assert!(!job.sections[1].format.italics);
        assert!(job.sections[2].format.italics);
    }
}
