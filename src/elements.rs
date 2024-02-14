use egui::{self, epaint, NumExt, RichText, Sense, TextStyle, Ui, Vec2};

pub(crate) fn soft_break(ui: &mut Ui) {
    ui.label(" ");
}

pub(crate) fn newline(ui: &mut Ui) {
    ui.label("\n");
}

pub(crate) fn bullet_point(ui: &mut Ui) {
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

pub(crate) fn bullet_point_hollow(ui: &mut Ui) {
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

pub(crate) fn number_point(ui: &mut Ui, number: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        format!("{number}. "),
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}

pub(crate) fn footnote_start(ui: &mut Ui, note: &str) {
    ui.label(RichText::new(note).raised().strong().small());
}

pub(crate) fn footnote(ui: &mut Ui, text: &str) {
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
    ui.fonts(|f| f.glyph_width(&id, ' '))
}

/// Enhanced/specialized version of egui's code blocks. This one features copy button and borders
pub fn code_block<'t>(
    ui: &mut Ui,
    max_width: f32,
    text: &str,
    layouter: &'t mut dyn FnMut(&Ui, &str, f32) -> std::sync::Arc<egui::Galley>,
) {
    let pre_text_edit_position = ui.next_widget_position();
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

    // Background color + frame (This is lost when TextEdit it not editable)
    let frame_rect = output.response.rect;
    ui.painter().set(
        where_to_put_background,
        epaint::RectShape::new(
            frame_rect,
            ui.style().noninteractive().rounding,
            ui.visuals().extreme_bg_color,
            ui.visuals().widgets.noninteractive.bg_stroke,
        ),
    );

    // Copy icon
    let spacing = &ui.style().spacing;
    let position = pre_text_edit_position
        + egui::vec2(
            max_width - spacing.icon_width - spacing.button_padding.x,
            -spacing.icon_width + spacing.button_padding.y,
        );

    let copy_button = ui.put(
        egui::Rect {
            min: position,
            max: position,
        },
        egui::Button::new("🗐")
            .small()
            .frame(false)
            .fill(egui::Color32::TRANSPARENT),
    );

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
        ui.ctx().copy_text(copy_text);
    }
}

// Stripped down version of egui's Checkbox. The only difference is that this
// creates a noninteractive checkbox. ui.add_enabled could have been used instead,
// but it makes the checkbox too grey.
pub(crate) struct Checkbox<'a> {
    checked: &'a mut bool,
}

impl<'a> Checkbox<'a> {
    pub fn without_text(checked: &'a mut bool) -> Self {
        Checkbox { checked }
    }
}

impl<'a> egui::Widget for Checkbox<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let Checkbox { checked } = self;

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
                visuals.rounding,
                visuals.bg_fill,
                visuals.bg_stroke,
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

pub(crate) struct ListLevel {
    current_number: Option<u64>,
}

#[derive(Default)]
pub(crate) struct List {
    items: Vec<ListLevel>,
}

impl List {
    pub fn start_level_with_number(&mut self, start_number: u64) {
        self.items.push(ListLevel {
            current_number: Some(start_number),
        });
    }

    pub fn start_level_without_number(&mut self) {
        self.items.push(ListLevel {
            current_number: None,
        });
    }

    pub fn is_inside_a_list(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn start_item(&mut self, ui: &mut Ui, options: &crate::CommonMarkOptions) {
        let len = self.items.len();
        if let Some(item) = self.items.last_mut() {
            newline(ui);
            ui.label(" ".repeat((len - 1) * options.indentation_spaces));

            if let Some(number) = &mut item.current_number {
                number_point(ui, &number.to_string());
                *number += 1;
            } else if len > 1 {
                bullet_point_hollow(ui);
            } else {
                bullet_point(ui);
            }
        } else {
            unreachable!();
        }
    }

    pub fn end_level(&mut self, ui: &mut Ui) {
        self.items.pop();

        if self.items.is_empty() {
            newline(ui);
        }
    }
}
