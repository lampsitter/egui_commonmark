//! Layout of markdown tables.
//!
//! Columns are sized so that the table fits the available width when it can.
//! Cells whose content is wider than their column wrap their text.

use std::ops::Range;

use egui::{
    Align, Layout, Rect, RichText, Sense, Shape, TextStyle, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::misc::Style;

/// Rounding of the striped row backgrounds.
const STRIPE_ROUNDING: f32 = 2.0;

/// The smallest width a column is shrunk to, in em (body font size).
const MIN_COLUMN_EM: f32 = 4.0;

/// The rich text of a cell, used to measure its natural width.
///
/// `None` if the cell contains something that cannot be measured as text,
/// such as an image or a line break.
pub type CellText = Option<Vec<RichText>>;

/// Convert a table column alignment to a horizontal [`Align`].
pub fn align_from_alignment(alignment: pulldown_cmark::Alignment) -> Align {
    match alignment {
        pulldown_cmark::Alignment::None | pulldown_cmark::Alignment::Left => Align::Min,
        pulldown_cmark::Alignment::Center => Align::Center,
        pulldown_cmark::Alignment::Right => Align::Max,
    }
}

/// Collect the inline text of a table cell together with its style.
///
/// Returns `None` if the cell contains anything that is not plain inline text.
pub fn cell_text_pieces<'e>(
    base_style: &Style,
    events: &[(pulldown_cmark::Event<'e>, Range<usize>)],
) -> Option<Vec<(Style, pulldown_cmark::CowStr<'e>)>> {
    let mut style = base_style.clone();
    let mut pieces = Vec::new();

    for (event, _) in events {
        match event {
            pulldown_cmark::Event::Text(text) | pulldown_cmark::Event::InlineHtml(text) => {
                pieces.push((style.clone(), text.clone()));
            }
            pulldown_cmark::Event::Code(text) => {
                let code_style = Style {
                    code: true,
                    ..style.clone()
                };
                pieces.push((code_style, text.clone()));
            }
            pulldown_cmark::Event::SoftBreak => {
                pieces.push((style.clone(), " ".into()));
            }
            pulldown_cmark::Event::Start(tag) => match tag {
                pulldown_cmark::Tag::Emphasis => style.emphasis = true,
                pulldown_cmark::Tag::Strong => style.strong = true,
                pulldown_cmark::Tag::Strikethrough => style.strikethrough = true,
                pulldown_cmark::Tag::Link { .. }
                | pulldown_cmark::Tag::TableHead
                | pulldown_cmark::Tag::TableRow
                | pulldown_cmark::Tag::TableCell => {}
                _ => return None,
            },
            pulldown_cmark::Event::End(tag) => match tag {
                pulldown_cmark::TagEnd::Emphasis => style.emphasis = false,
                pulldown_cmark::TagEnd::Strong => style.strong = false,
                pulldown_cmark::TagEnd::Strikethrough => style.strikethrough = false,
                pulldown_cmark::TagEnd::Link
                | pulldown_cmark::TagEnd::TableHead
                | pulldown_cmark::TagEnd::TableRow
                | pulldown_cmark::TagEnd::TableCell => {}
                _ => return None,
            },
            _ => return None,
        }
    }

    Some(pieces)
}

/// Collect the inline text of a table cell as [`RichText`], styled like the renderer would.
///
/// Returns `None` if the cell contains anything that is not plain inline text.
pub fn cell_text(
    ui: &Ui,
    base_style: &Style,
    events: &[(pulldown_cmark::Event<'_>, Range<usize>)],
) -> CellText {
    let pieces = cell_text_pieces(base_style, events)?;
    Some(
        pieces
            .iter()
            .map(|(style, text)| style.to_richtext(ui, text))
            .collect(),
    )
}

/// The width of the given texts laid out on a single line.
pub fn measure_texts(ui: &Ui, texts: &[RichText]) -> f32 {
    texts
        .iter()
        .map(|text| {
            egui::WidgetText::from(text.clone())
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    TextStyle::Body,
                )
                .size()
                .x
        })
        .sum()
}

/// The natural width of a cell, if it can be measured.
pub fn measure_cell(ui: &Ui, cell: &CellText) -> Option<f32> {
    cell.as_ref().map(|texts| measure_texts(ui, texts))
}

/// Distribute `available` width over columns with the given natural (unwrapped) widths.
///
/// If everything fits, each column gets its natural width.
/// Otherwise the wide columns shrink, sharing the remaining width in proportion
/// to their natural widths, while narrow columns keep their natural width.
/// No column is shrunk below `min_col`, so the result can be wider than `available`.
pub fn column_widths(natural: &[f32], available: f32, min_col: f32) -> Vec<f32> {
    let available = available.max(0.0);
    let mut widths: Vec<f32> = natural.iter().map(|w| w.max(0.0)).collect();
    let mut is_fixed = vec![false; widths.len()];

    loop {
        let fixed_sum: f32 = std::iter::zip(&widths, &is_fixed)
            .filter(|(_, fixed)| **fixed)
            .map(|(w, _)| w)
            .sum();
        let remaining = available - fixed_sum;

        let flexible: Vec<usize> = (0..widths.len()).filter(|&i| !is_fixed[i]).collect();
        let flexible_natural: f32 = flexible.iter().map(|&i| natural[i].max(0.0)).sum();

        if flexible_natural <= remaining || flexible_natural <= 0.0 {
            for i in flexible {
                widths[i] = natural[i].max(0.0);
            }
            return widths;
        }

        // Columns narrower than their fair share keep their natural width:
        let fair = remaining.max(0.0) / flexible.len() as f32;
        let mut changed = false;
        for &i in &flexible {
            let natural_width = natural[i].max(0.0);
            if natural_width <= fair {
                widths[i] = natural_width;
                is_fixed[i] = true;
                changed = true;
            }
        }
        if changed {
            continue;
        }

        // The rest share the remaining width in proportion to their natural width:
        for i in flexible {
            let natural_width = natural[i].max(0.0);
            let target = remaining.max(0.0) * natural_width / flexible_natural;
            if target < min_col {
                widths[i] = min_col.min(natural_width);
                is_fixed[i] = true;
                changed = true;
            } else {
                widths[i] = target;
            }
        }

        if !changed {
            return widths;
        }
    }
}

/// Lays out one table: computes column widths up front, then places
/// each cell in a fixed-width column so that long text wraps.
pub struct TableLayout {
    /// Full width of each column, including padding.
    col_widths: Vec<f32>,
    aligns: Vec<Align>,
    cell_padding: Vec2,
    available_width: f32,

    left: f32,
    row: usize,
    col: usize,
    row_top: f32,
    row_bottom: f32,
    header_bottom: Option<f32>,
}

impl TableLayout {
    /// `natural_widths` is the widest natural cell width per column, excluding padding.
    pub fn new(ui: &Ui, natural_widths: &[f32], aligns: Vec<Align>, available_width: f32) -> Self {
        let cell_padding = vec2(
            2.0 * ui.spacing().button_padding.x,
            ui.spacing().item_spacing.y,
        );
        let em = TextStyle::Body.resolve(ui.style()).size;
        let min_col = MIN_COLUMN_EM * em + 2.0 * cell_padding.x;

        let natural: Vec<f32> = natural_widths
            .iter()
            .map(|w| w + 2.0 * cell_padding.x)
            .collect();
        let col_widths = column_widths(&natural, available_width, min_col);

        Self {
            col_widths,
            aligns,
            cell_padding,
            available_width,
            left: 0.0,
            row: 0,
            col: 0,
            row_top: 0.0,
            row_bottom: 0.0,
            header_bottom: None,
        }
    }

    /// Width of the whole table, including cell padding.
    pub fn total_width(&self) -> f32 {
        self.col_widths.iter().sum()
    }

    /// Show the table. Call [`Self::row`] once per row inside `add_rows`, header first.
    ///
    /// Tables that cannot fit even with wrapped cells scroll horizontally.
    pub fn show(&mut self, ui: &mut Ui, id: egui::Id, add_rows: impl FnOnce(&mut Self, &mut Ui)) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            if self.available_width < self.total_width() {
                ui.spacing_mut().scroll.content_margin.bottom = ui.spacing().scroll.bar_width as i8;
                egui::ScrollArea::horizontal()
                    .id_salt(id.with("_hscroll"))
                    .show(ui, |ui| self.show_rows(ui, add_rows));
            } else {
                self.show_rows(ui, add_rows);
            }
        });
    }

    fn show_rows(&mut self, ui: &mut Ui, add_rows: impl FnOnce(&mut Self, &mut Ui)) {
        self.left = ui.cursor().left();
        add_rows(self, ui);

        if let Some(header_bottom) = self.header_bottom {
            let stroke = egui::Stroke::new(1.0, ui.visuals().weak_text_color());
            let y = ui.painter().round_to_pixel_center(header_bottom);
            ui.painter()
                .hline(self.left..=self.left + self.total_width(), y, stroke);
        }
    }

    /// Add one row. Call [`Self::cell`] once per column inside `add_cells`.
    pub fn row(
        &mut self,
        ui: &mut Ui,
        is_header: bool,
        add_cells: impl FnOnce(&mut Self, &mut Ui),
    ) {
        let background = ui.painter().add(Shape::Noop);

        self.col = 0;
        self.row_top = ui.cursor().top();
        self.row_bottom = self.row_top + ui.text_style_height(&TextStyle::Body);
        add_cells(self, ui);

        let rect = Rect::from_min_max(
            pos2(self.left, self.row_top),
            pos2(
                self.left + self.total_width(),
                self.row_bottom + 2.0 * self.cell_padding.y,
            ),
        );

        if is_header {
            self.header_bottom = Some(rect.bottom());
        } else if self.row % 2 == 1 {
            ui.painter().set(
                background,
                Shape::rect_filled(rect, STRIPE_ROUNDING, ui.visuals().faint_bg_color),
            );
        }

        ui.allocate_rect(rect, Sense::hover());
        self.row += 1;
    }

    /// Add one cell to the current row.
    ///
    /// `natural_width` is the unwrapped width of the content, used for center
    /// and right alignment. Cells that cannot be measured are left aligned.
    pub fn cell(
        &mut self,
        ui: &mut Ui,
        natural_width: Option<f32>,
        add_contents: impl FnOnce(&mut Ui),
    ) {
        let col = self.col;
        self.col += 1;
        let Some(&col_width) = self.col_widths.get(col) else {
            return;
        };

        let x = self.left + self.col_widths[..col].iter().sum::<f32>() + self.cell_padding.x;
        let inner_width = (col_width - 2.0 * self.cell_padding.x).max(0.0);

        let align = self.aligns.get(col).copied().unwrap_or(Align::Min);
        let slack = natural_width.map_or(0.0, |w| (inner_width - w).max(0.0));
        let offset = match align {
            Align::Min => 0.0,
            Align::Center => 0.5 * slack,
            Align::Max => slack,
        };

        let cell_rect = Rect::from_min_size(
            pos2(x + offset, self.row_top + self.cell_padding.y),
            vec2(inner_width - offset, 0.0),
        );
        let layout = Layout::left_to_right(Align::BOTTOM).with_main_wrap(true);
        let mut cell_ui = ui.new_child(UiBuilder::new().max_rect(cell_rect).layout(layout));
        cell_ui.spacing_mut().item_spacing.x = 0.0;
        cell_ui.set_row_height(cell_ui.text_style_height(&TextStyle::Body));

        add_contents(&mut cell_ui);

        self.row_bottom = self.row_bottom.max(cell_ui.min_rect().bottom());
    }
}

#[cfg(test)]
mod tests {
    use super::column_widths;

    fn assert_widths(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
        for (a, e) in std::iter::zip(actual, expected) {
            assert!((a - e).abs() < 0.01, "{actual:?} vs {expected:?}");
        }
    }

    #[test]
    fn empty() {
        assert!(column_widths(&[], 100.0, 10.0).is_empty());
    }

    #[test]
    fn fits() {
        assert_widths(&column_widths(&[30.0, 70.0], 100.0, 10.0), &[30.0, 70.0]);
        assert_widths(&column_widths(&[30.0, 50.0], 100.0, 10.0), &[30.0, 50.0]);
    }

    #[test]
    fn one_long_column_shrinks() {
        assert_widths(
            &column_widths(&[20.0, 30.0, 500.0], 200.0, 10.0),
            &[20.0, 30.0, 150.0],
        );
    }

    #[test]
    fn columns_within_fair_share_keep_natural_width() {
        assert_widths(
            &column_widths(&[100.0, 300.0], 200.0, 10.0),
            &[100.0, 100.0],
        );
    }

    #[test]
    fn all_long_columns_shrink_proportionally() {
        assert_widths(&column_widths(&[300.0, 500.0], 200.0, 10.0), &[75.0, 125.0]);
    }

    #[test]
    fn columns_stop_at_min_width() {
        let widths = column_widths(&[100.0, 900.0], 100.0, 40.0);
        assert_widths(&widths, &[40.0, 60.0]);

        let widths = column_widths(&[500.0, 500.0], 50.0, 40.0);
        assert_widths(&widths, &[40.0, 40.0]);
    }

    #[test]
    fn narrow_columns_stay_narrow_below_min() {
        assert_widths(&column_widths(&[10.0, 900.0], 100.0, 40.0), &[10.0, 90.0]);
    }

    #[test]
    fn zero_available() {
        assert_widths(&column_widths(&[10.0, 900.0], 0.0, 40.0), &[10.0, 40.0]);
    }
}
