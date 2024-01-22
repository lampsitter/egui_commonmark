use crate::elements::*;
use crate::{CommonMarkCache, CommonMarkOptions};

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};

use egui::{self, Id, TextStyle, Ui};

pub struct CommonMarkViewerInternal {
    source_id: Id,
    curr_table: usize,
    text_style: crate::Style,
    list_point: Option<u64>,
    link: Option<crate::Link>,
    indentation: i64,
    image: Option<crate::Image>,
    should_insert_newline: bool,
    fenced_code_block: Option<crate::FencedCodeBlock>,
}

impl CommonMarkViewerInternal {
    pub fn new(source_id: Id) -> Self {
        Self {
            source_id,
            curr_table: 0,
            text_style: crate::Style::default(),
            list_point: None,
            link: None,
            indentation: -1,
            image: None,
            should_insert_newline: true,
            fenced_code_block: None,
        }
    }
}

impl CommonMarkViewerInternal {
    /// Be aware that this acquires egui::Context internally.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let arena = Arena::new();
            let mut parse_opt = Options::default();
            parse_opt.extension.strikethrough = true;
            parse_opt.extension.table = true;
            parse_opt.extension.tasklist = true;
            parse_opt.extension.footnotes = true;

            let root = parse_document(&arena, text, &parse_opt);

            self.render(ui, cache, options, max_width, root);
        });
    }

    // FIXME: recursion limit...
    fn render<'a>(
        &mut self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
        node: &'a AstNode<'a>,
    ) {
        for c in node.children() {
            match &c.data.borrow().value {
                NodeValue::Document => self.render(ui, cache, options, max_width, c),
                NodeValue::FrontMatter(_front_matter) => {}

                NodeValue::BlockQuote => {
                    self.text_style.quote = true;
                    ui.add(egui::Separator::default().horizontal());

                    self.render(ui, cache, options, max_width, c);

                    self.text_style.quote = false;
                    ui.add(egui::Separator::default().horizontal());
                    newline(ui);
                }

                NodeValue::List(list) => {
                    self.indentation += 1;
                    if list.list_type == comrak::nodes::ListType::Ordered {
                        self.list_point = Some(list.start as u64);
                    }

                    self.render(ui, cache, options, max_width, c);

                    self.indentation -= 1;
                    if self.indentation == -1 {
                        newline(ui);
                        self.should_insert_newline = true;
                    }
                }

                NodeValue::Item(_item) => {
                    self.start_item(ui, options);
                    self.render(ui, cache, options, max_width, c);
                }

                NodeValue::DescriptionList => {}
                NodeValue::DescriptionItem(_desc_item) => {}
                NodeValue::DescriptionTerm => {}
                NodeValue::DescriptionDetails => {}

                NodeValue::CodeBlock(code_block) => {
                    if code_block.fenced {
                        // FIXME:? the default setup is made to collect the events. This is not
                        // needed here
                        self.fenced_code_block = Some(crate::FencedCodeBlock {
                            lang: code_block.info.to_string(),
                            content: code_block.literal.to_string(),
                        });

                        newline(ui);
                    }

                    self.text_style.code = true;
                    self.render(ui, cache, options, max_width, c);

                    if let Some(block) = self.fenced_code_block.take() {
                        block.end(ui, cache, options, max_width);
                    }
                    self.text_style.code = false;
                }

                NodeValue::HtmlBlock(_) => {} // not supported
                //
                NodeValue::Paragraph => {
                    // FIXME: Copy newline logic? based on image stuff, yes most definetly
                    if self.should_insert_newline {
                        newline(ui);
                        // we deliberately do not set it to false after this
                    }

                    self.render(ui, cache, options, max_width, c);
                    // To end the inlines
                    // FIXME: Add to the end of all blocks that contain inlines
                    newline(ui);
                }

                NodeValue::Heading(heading) => {
                    self.text_style.heading = Some(heading.level);
                    self.render(ui, cache, options, max_width, c);
                    self.text_style.heading = None;

                    // end the line
                    newline(ui);

                    // add a new line
                    newline(ui);
                }

                NodeValue::ThematicBreak => {
                    newline(ui);
                    ui.add(egui::Separator::default().horizontal());
                    // This does not add a new line, but instead ends the separator
                    newline(ui);
                }

                NodeValue::FootnoteDefinition(f) => {
                    self.should_insert_newline = false;
                    footnote(ui, &f.name);
                    self.render(ui, cache, options, max_width, c);
                }

                NodeValue::Table(_table) => {
                    newline(ui);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        let id = self.source_id.with(self.curr_table);
                        self.curr_table += 1;
                        egui::Grid::new(id).striped(true).show(ui, |ui| {
                            self.should_insert_newline = false;
                            self.render(ui, cache, options, max_width, c);
                        });
                    });

                    newline(ui);
                }

                NodeValue::TableRow(_is_header) => {
                    self.render(ui, cache, options, max_width, c);
                    ui.end_row();
                }
                NodeValue::TableCell => {
                    self.render(ui, cache, options, max_width, c);
                    // Ensure space between cells
                    ui.label("  ");
                }
                NodeValue::TaskItem(item) => {
                    // FIXME: support custom icon
                    if item.is_some() {
                        ui.add(Checkbox::without_text(&mut true));
                    } else {
                        ui.add(Checkbox::without_text(&mut false));
                    }

                    self.render(ui, cache, options, max_width, c);
                }
                // inlines
                NodeValue::Text(text) => self.event_text(text, ui),

                NodeValue::SoftBreak => {
                    // FIXME: Abstract over backends
                    ui.label(" ");
                }
                NodeValue::LineBreak => {
                    newline(ui);
                }

                NodeValue::Strikethrough => {
                    self.text_style.strikethrough = true;
                    self.render(ui, cache, options, max_width, c);
                    self.text_style.strikethrough = false;
                }

                NodeValue::Code(node) => {
                    self.text_style.code = true;
                    self.event_text(&node.literal, ui);
                    self.text_style.code = false;
                }
                NodeValue::HtmlInline(_) => {} // not supported
                NodeValue::Emph => {
                    self.text_style.emphasis = true;
                    self.render(ui, cache, options, max_width, c);
                    self.text_style.emphasis = false;
                }

                NodeValue::Strong => {
                    self.text_style.strong = true;
                    self.render(ui, cache, options, max_width, c);
                    self.text_style.strong = false;
                }

                NodeValue::Superscript => {}
                NodeValue::Link(link) => {
                    self.link = Some(crate::Link {
                        destination: link.url.to_owned(),
                        text: vec![link.title.to_owned().into()],
                    });

                    self.render(ui, cache, options, max_width, c);

                    if let Some(link) = self.link.take() {
                        link.end(ui, cache);
                    }
                }

                NodeValue::Image(image) => {
                    self.image = Some(crate::Image::new(&image.url, options));
                    // FIXME:

                    self.image
                        .as_mut()
                        .unwrap()
                        .alt_text
                        .push(image.title.to_owned().into());

                    self.render(ui, cache, options, max_width, c);

                    if let Some(image) = self.image.take() {
                        image.end(ui, options);
                    }
                }

                NodeValue::FootnoteReference(footnote) => {
                    self.should_insert_newline = false;
                    footnote_start(ui, &footnote.name);
                    self.render(ui, cache, options, max_width, c);
                }
            }
        }
    }

    fn event_text(&mut self, text: &str, ui: &mut Ui) {
        let rich_text = self.text_style.to_richtext(ui, &text);
        if let Some(image) = &mut self.image {
            image.alt_text.push(rich_text);
        } else if let Some(block) = &mut self.fenced_code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            link.text.push(rich_text);
        } else {
            ui.label(rich_text);
        }
    }

    // FIXME: Duplicate
    fn start_item(&mut self, ui: &mut Ui, options: &CommonMarkOptions) {
        newline(ui);
        ui.label(" ".repeat(self.indentation as usize * options.indentation_spaces));

        self.should_insert_newline = false;
        if let Some(mut number) = self.list_point.take() {
            number_point(ui, &number.to_string());
            number += 1;
            self.list_point = Some(number);
        } else if self.indentation >= 1 {
            bullet_point_hollow(ui);
        } else {
            bullet_point(ui);
        }
    }
}
