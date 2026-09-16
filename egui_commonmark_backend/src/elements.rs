use egui::{self, NumExt, RichText, Sense, TextBuffer, TextStyle, Ui, Vec2, epaint};
use egui::{Color32, Pos2, Rect, WidgetText, text::CCursor};
use std::ops::Range;

#[inline]
pub fn rule(ui: &mut Ui, end_line: bool) {
    ui.add(egui::Separator::default().horizontal());
    // This does not add a new line, but instead ends the separator
    if end_line {
        newline(ui);
    }
}

#[inline]
pub fn soft_break(ui: &mut Ui) {
    ui.label(" ");
}

#[inline]
pub fn newline(ui: &mut Ui) {
    ui.label("\n");
}

pub fn bullet_point(ui: &mut Ui) {
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

pub fn bullet_point_hollow(ui: &mut Ui) {
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

pub fn number_point(ui: &mut Ui, number: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        format!("{number}."),
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}

#[inline]
pub fn footnote_start(ui: &mut Ui, note: &str) {
    ui.label(RichText::new(note).raised().strong().small());
}

pub fn footnote(ui: &mut Ui, text: &str) {
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
    ui.fonts_mut(|f| f.glyph_width(&id, ' '))
}

/// Render a run of text as a single label, optionally painting search-match
/// backgrounds behind some of its characters.
///
/// This always creates exactly one widget, regardless of how many (if any)
/// `intervals` are supplied, so that toggling or changing search matches
/// never shifts egui's auto-generated widget IDs for subsequent widgets.
///
/// Returns the widget's response, the on-screen rect of the active match
/// (if `intervals` contains one), and a `Vec` with one `Option<Rect>` per
/// interval (in the same order as `intervals`): `Some(rect)` for each
/// match whose byte range is non-empty, `None` otherwise. All rects are
/// computed unconditionally even when the label is outside the clip rect,
/// so callers can use them to track match positions during scrolling.
pub fn label_with_search_highlight(
    ui: &mut Ui,
    text: RichText,
    intervals: &[(Range<usize>, bool)],
    match_bg: Color32,
    active_bg: Color32,
) -> (egui::Response, Option<Rect>, Vec<Option<Rect>>) {
    let widget_text: WidgetText = if intervals.is_empty() {
        text.into()
    } else {
        let valign = ui.text_valign();
        let job_arc: std::sync::Arc<egui::text::LayoutJob> = WidgetText::from(text)
            .into_layout_job(ui.style(), egui::FontSelection::Default, valign);
        let mut job = std::sync::Arc::unwrap_or_clone(job_arc);
        crate::search::apply_search_highlights(&mut job, intervals, match_bg, active_bg);
        job.into()
    };

    let (pos, galley, response) = egui::Label::new(widget_text).layout_in_ui(ui);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), galley.text())
    });

    // Compute rects for all intervals unconditionally: `pos` and `galley` hold
    // valid screen-coordinate data even when the label is outside the clip
    // rect. The active-match rect is used to scroll it into view; the full
    // set is returned so callers can track every match's position during
    // scrolling. Only the *painting* step below stays inside `is_rect_visible`.
    let all_rects: Vec<Option<Rect>> = intervals
        .iter()
        .map(|(range, _)| highlight_rect_for_byte_range(&galley, pos, range.clone()))
        .collect();
    let active_rect = intervals
        .iter()
        .enumerate()
        .find(|(_, (_, active))| *active)
        .and_then(|(i, _)| all_rects[i]);

    if ui.is_rect_visible(response.rect) {
        let response_color = ui.style().visuals.text_color();
        let selectable = ui.style().interaction.selectable_labels;
        if selectable {
            egui::text_selection::LabelSelectionState::label_text_selection(
                ui,
                &response,
                pos,
                galley,
                response_color,
                egui::Stroke::NONE,
            );
        } else {
            ui.painter()
                .add(epaint::TextShape::new(pos, galley, response_color));
        }
    }

    (response, active_rect, all_rects)
}

/// The rect (in screen coordinates) spanned by the given byte range within
/// `galley`'s text, or `None` if the range is empty. Used to scroll the
/// active search match into view and to record per-match virtual-Y positions.
pub fn highlight_rect_for_byte_range(
    galley: &std::sync::Arc<egui::Galley>,
    pos: Pos2,
    local_byte_range: Range<usize>,
) -> Option<Rect> {
    let text = &galley.job.text;
    if local_byte_range.start >= local_byte_range.end {
        return None;
    }

    let char_count_up_to = |byte_idx: usize| -> usize {
        text.get(..byte_idx.min(text.len()))
            .map_or_else(|| text.chars().count(), |s| s.chars().count())
    };

    let start_char = char_count_up_to(local_byte_range.start);
    let end_char = char_count_up_to(local_byte_range.end);

    let start_rect = galley.pos_from_cursor(CCursor::new(start_char));
    let end_rect = galley.pos_from_cursor(CCursor::new(end_char));

    Some(Rect::from_two_pos(start_rect.min, end_rect.max).translate(pos.to_vec2()))
}

/// Enhanced/specialized version of egui's code blocks. This one features copy button and borders
/// Returns `(galley_pos, galley)` so the caller can compute per-match
/// virtual-Y positions via [`highlight_rect_for_byte_range`] without a
/// second layout pass.
pub fn code_block<'t>(
    ui: &mut Ui,
    max_width: f32,
    text: &str,
    layouter: &'t mut dyn FnMut(&Ui, &dyn TextBuffer, f32) -> std::sync::Arc<egui::Galley>,
    scroll_to_active_match: Option<Range<usize>>,
) -> (egui::Pos2, std::sync::Arc<egui::Galley>) {
    let mut text = text.strip_suffix('\n').unwrap_or(text);

    // To manually add background color to the code block, we imitate what
    // TextEdit does internally
    let where_to_put_background = ui.painter().add(egui::Shape::Noop);

    // We use a `TextEdit` to make the text selectable.
    // Note that we take a `&mut` to a non-`mut` `&str`, which is
    // the how to tell `egui` that the text is not editable.
    let output = egui::TextEdit::multiline(&mut text)
        .layouter(layouter)
        .desired_width(max_width)
        // prevent trailing lines
        .desired_rows(1)
        .show(ui);

    if let Some(range) = scroll_to_active_match
        && let Some(rect) = highlight_rect_for_byte_range(&output.galley, output.galley_pos, range)
    {
        ui.scroll_to_rect(rect, Some(egui::Align::Center));
    }

    // Background color + frame (This is lost when TextEdit it not editable)
    let frame_rect = output.response.rect;
    ui.painter().set(
        where_to_put_background,
        epaint::RectShape::new(
            frame_rect,
            ui.style().noninteractive().corner_radius,
            ui.visuals().extreme_bg_color,
            ui.visuals().widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Outside,
        ),
    );

    // Copy icon
    let spacing = &ui.style().spacing;
    let position = egui::pos2(
        frame_rect.right_top().x - spacing.icon_width * 0.5 - spacing.button_padding.x,
        frame_rect.right_top().y + spacing.button_padding.y * 2.0,
    );

    // Check if we should show ✔ instead of 🗐 if the text was copied and the mouse is hovered
    let persistent_id = ui.make_persistent_id(output.response.id);
    let copied_icon = ui.memory_mut(|m| *m.data.get_temp_mut_or_default::<bool>(persistent_id));

    let copy_button = ui
        .put(
            egui::Rect {
                min: position,
                max: position,
            },
            egui::Button::new(if copied_icon { "✔" } else { "🗐" })
                .small()
                .frame(false)
                .fill(egui::Color32::TRANSPARENT),
        )
        // workaround for a regression after egui 0.27 where the edit cursor was shown even when
        // hovering over the button. We try interact_cursor first to allow the cursor to be
        // overridden
        .on_hover_cursor(
            ui.visuals()
                .interact_cursor
                .unwrap_or(egui::CursorIcon::Default),
        );

    // Update icon state in persistent memory
    if copied_icon && !copy_button.hovered() {
        ui.memory_mut(|m| *m.data.get_temp_mut_or_default(persistent_id) = false);
    }
    if !copied_icon && copy_button.clicked() {
        ui.memory_mut(|m| *m.data.get_temp_mut_or_default(persistent_id) = true);
    }

    if copy_button.clicked() {
        use egui::TextBuffer as _;
        let copy_text = if let Some(cursor) = output.cursor_range {
            let selected_chars = cursor.as_sorted_char_range();
            let selected_text = text.char_range(selected_chars);
            if selected_text.is_empty() {
                text.to_owned()
            } else {
                selected_text.to_owned()
            }
        } else {
            text.to_owned()
        };
        ui.copy_text(copy_text);
    }

    (output.galley_pos, output.galley)
}

// Stripped down version of egui's Checkbox. The only difference is that this
// creates a noninteractive checkbox. ui.add_enabled could have been used instead,
// but it makes the checkbox too grey.
pub struct ImmutableCheckbox<'a> {
    checked: &'a mut bool,
}

impl<'a> ImmutableCheckbox<'a> {
    pub fn without_text(checked: &'a mut bool) -> Self {
        ImmutableCheckbox { checked }
    }
}

impl egui::Widget for ImmutableCheckbox<'_> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let ImmutableCheckbox { checked } = self;

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
                visuals.corner_radius,
                visuals.bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
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

pub fn blockquote(ui: &mut Ui, accent: egui::Color32, add_contents: impl FnOnce(&mut Ui)) {
    let start = ui.painter().add(egui::Shape::Noop);
    let response = egui::Frame::new()
        // offset the frame so that we can use the space for the horizontal line and other stuff
        // By not using a separator we have better control
        .outer_margin(egui::Margin {
            left: 10,
            ..Default::default()
        })
        .show(ui, add_contents)
        .response;

    // FIXME: Add some rounding

    ui.painter().set(
        start,
        egui::epaint::Shape::line_segment(
            [
                egui::pos2(response.rect.left_top().x, response.rect.left_top().y + 5.0),
                egui::pos2(
                    response.rect.left_bottom().x,
                    response.rect.left_bottom().y - 5.0,
                ),
            ],
            egui::Stroke::new(3.0, accent),
        ),
    );
}
