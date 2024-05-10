#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::ops::Range;

use egui_commonmark_backend::{
    alerts::{Alert, AlertBundle},
    misc::{CommonMarkCache, Style},
    pulldown::*,
    CommonMarkOptions, FencedCodeBlock, Image,
};

use egui::{self, Id, Pos2, TextStyle, Ui, Vec2};

use proc_macro2::TokenStream;
use pulldown_cmark::{CowStr, HeadingLevel, Options};
use quote::quote;
use syn::Expr;

struct ListLevel {
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

    pub fn start_item(&mut self, options: &CommonMarkOptions) -> TokenStream {
        let mut stream = TokenStream::new();
        let len = self.items.len();
        if let Some(item) = self.items.last_mut() {
            let spaces = " ".repeat((len - 1) * options.indentation_spaces);
            stream.extend(quote!(
                ::egui_commonmark_backend::newline(ui);
                ui.label(#spaces);
            ));

            if let Some(number) = &mut item.current_number {
                let num = number.to_string();
                stream.extend(quote!( ::egui_commonmark_backend::number_point(ui, #num);));
                *number += 1;
            } else if len > 1 {
                stream.extend(quote!( ::egui_commonmark_backend::bullet_point_hollow(ui);));
            } else {
                stream.extend(quote!( ::egui_commonmark_backend::bullet_point(ui);));
            }
        } else {
            unreachable!();
        }

        stream.extend(quote!( ui.add_space(4.0); ));
        stream
    }

    pub fn end_level(&mut self) -> TokenStream {
        let mut stream = TokenStream::new();
        self.items.pop();

        if self.items.is_empty() {
            stream.extend(quote!( ::egui_commonmark_backend::newline(ui); ));
        }

        stream
    }
}

/// To prevent depending on ui during macro evalation we must store the current
/// style and text temporarily
pub(crate) struct StyledText {
    style: Style,
    text: String,
}

impl StyledText {
    pub fn new(style: Style, text: impl Into<String>) -> Self {
        Self {
            style,
            text: text.into(),
        }
    }
}

pub struct StyledLink {
    pub destination: String,
    pub text: Vec<StyledText>,
}

pub struct StyledImage {
    pub uri: String,
    pub alt_text: Vec<StyledText>,
}

pub(crate) struct CommonMarkViewerInternal {
    pub source_id: Id,
    pub curr_table: usize,
    pub text_style: Style,
    pub list: List,
    pub link: Option<StyledLink>,
    pub image: Option<StyledImage>,
    pub should_insert_newline: bool,
    pub fenced_code_block: Option<FencedCodeBlock>,
    pub is_list_item: bool,
    pub is_table: bool,
    pub is_blockquote: bool,
    pub checkbox_events: Vec<CheckboxClickEvent>,

    /// Informs that a calculation of heading sizes is required.
    /// This will dump min and max text size at the top of the macro output
    /// to reduce code duplication.
    pub dumps_heading: bool,
}

pub(crate) struct CheckboxClickEvent {
    checked: bool,
    span: Range<usize>,
}

impl CommonMarkViewerInternal {
    pub fn new(source_id: Id) -> Self {
        Self {
            source_id,
            curr_table: 0,
            text_style: Style::default(),
            list: List::default(),
            link: None,
            image: None,
            should_insert_newline: true,
            is_list_item: false,
            fenced_code_block: None,
            is_table: false,
            is_blockquote: false,
            checkbox_events: Vec::new(),
            dumps_heading: false,
        }
    }
}

impl CommonMarkViewerInternal {
    pub fn show(&mut self, ui: Expr, cache: Expr, text: &str) -> TokenStream {
        let mut events = pulldown_cmark::Parser::new_ext(text, parser_options())
            .into_offset_iter()
            .enumerate();

        let options = CommonMarkOptions::default();
        let mut stream = TokenStream::new();

        let mut event_stream = TokenStream::new();
        while let Some((index, (e, src_span))) = events.next() {
            let e = self.process_event(&mut events, e, src_span, &cache, &options, 500.0);
            event_stream.extend(e);
        }

        // FIXME: max_width

        stream.extend(quote!(
            ::egui_commonmark_backend::prepare_show(#cache, ui.ctx());
            let options = ::egui_commonmark_backend::CommonMarkOptions::default();
            let max_width = options.max_width(ui);
            let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

            ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let height = ui.text_style_height(&egui::TextStyle::Body);
                ui.set_row_height(height);
                #event_stream
            })
        ));

        let heights = if self.dumps_heading {
            dump_heading_heights()
        } else {
            TokenStream::new()
        };

        // Place all code within a block to prevent it from leaking into unrelated code
        //
        // we manually rename #ui to ui to prevent borrowing issues if #ui is not named ui and
        // there is a different ui also in scope that is called ui
        quote!({
            let ui: &mut egui::Ui = #ui;
            #heights
            #stream
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn process_event<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> TokenStream {
        let mut stream = self.event(event, src_span, cache, options, max_width);

        stream.extend(self.item_list_wrapping(events, max_width, cache, options));
        stream.extend(self.fenced_code_block(events, max_width, cache, options));
        stream.extend(self.table(events, cache, options, max_width));
        stream.extend(self.blockquote(events, max_width, cache, options));
        stream
    }

    fn item_list_wrapping<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.is_list_item {
            self.is_list_item = false;

            let item_events = delayed_events_list_item(events);
            let mut events_iter = item_events.into_iter().enumerate();

            let mut inner = TokenStream::new();

            while let Some((_, (e, src_span))) = events_iter.next() {
                inner.extend(self.process_event(
                    &mut events_iter,
                    e,
                    src_span,
                    cache,
                    options,
                    max_width,
                ));
            }

            // Required to ensure that the content of the list item is aligned with
            // the * or - when wrapping
            stream.extend(quote!(ui.horizontal_wrapped(|ui| {
                    #inner
            });));
        }

        stream
    }

    fn blockquote<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, pulldown_cmark::TagEnd::BlockQuote);

            if self.should_insert_newline {
                stream.extend(quote!( ::egui_commonmark_backend::newline(ui);));
            }

            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                let Alert {
                    accent_color,
                    icon,
                    identifier,
                    identifier_rendered,
                } = alert;

                let mut inner = TokenStream::new();
                for (event, src_span) in collected_events.into_iter() {
                    inner.extend(self.event(event, src_span, cache, options, max_width));
                }

                let r = accent_color.r();
                let g = accent_color.g();
                let b = accent_color.b();
                let a = accent_color.a();
                // FIXME: Figure out what rgba function to use
                stream.extend(quote!(
                ::egui_commonmark_backend::alert_ui(&egui_commonmark_backend::Alert {
                    accent_color: egui::Color32::from_rgba_premultiplied(#r, #g, #b, #a),
                    icon: #icon,
                    identifier: #identifier.to_owned(),
                    identifier_rendered: #identifier_rendered.to_owned()
                }, ui, |ui| {
                    #inner
                });));
            } else {
                let mut inner = TokenStream::new();

                self.text_style.quote = true;
                for (event, src_span) in collected_events {
                    inner.extend(self.event(event, src_span, cache, options, max_width));
                }
                self.text_style.quote = false;

                stream.extend(quote!(::egui_commonmark_backend::blockquote(ui, ui.visuals().weak_text_color(), |ui| {#inner});));
            }

            if self.should_insert_newline {
                stream.extend(quote!( ::egui_commonmark_backend::newline(ui);));
            }

            self.is_blockquote = false;
        }
        stream
    }

    fn fenced_code_block<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        while self.fenced_code_block.is_some() {
            if let Some((_, (e, src_span))) = events.next() {
                stream.extend(self.event(e, src_span, cache, options, max_width));
            } else {
                break;
            }
        }

        stream
    }

    fn table<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.is_table {
            stream.extend(quote!( ::egui_commonmark_backend::newline(ui);));

            let id = self.source_id.with(self.curr_table);
            self.curr_table += 1;

            let Table { header, rows } = parse_table(events);

            let mut header_stream = TokenStream::new();
            for col in header {
                let mut inner = TokenStream::new();
                for (e, src_span) in col {
                    self.should_insert_newline = false;
                    inner.extend(self.event(e, src_span, cache, options, max_width));
                }

                header_stream.extend(quote!(ui.horizontal(|ui| {#inner});));
            }

            let mut content_stream = TokenStream::new();
            for row in rows {
                let mut row_stream = TokenStream::new();
                for col in row {
                    let mut inner = TokenStream::new();
                    for (e, src_span) in col {
                        self.should_insert_newline = false;
                        inner.extend(self.event(e, src_span, cache, options, max_width));
                    }

                    row_stream.extend(quote!(ui.horizontal(|ui| {#inner});));
                }
                content_stream.extend(quote!(#row_stream ui.end_row();))
            }

            let hash = id.value();

            // FIXME: Hash is not the original
            stream.extend(quote!(
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::Grid::new(egui::Id::new(#hash)).striped(true).show(ui, |ui| {

                    #header_stream

                    ui.end_row();

                    #content_stream

                    ui.end_row();
                    });
                });
            ));

            self.is_table = false;
            self.should_insert_newline = true;
            stream.extend(quote!( ::egui_commonmark_backend::newline(ui);));
        }

        stream
    }

    fn event(
        &mut self,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> TokenStream {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(tag, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => self.event_text(text),
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                let s = self.event_text(text);
                self.text_style.code = false;
                s
            }
            pulldown_cmark::Event::InlineHtml(_) | pulldown_cmark::Event::Html(_) => {
                TokenStream::new()
            }
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                let footnote = footnote.to_string();
                quote!(::egui_commonmark_backend::footnote_start(ui, #footnote);)
            }
            pulldown_cmark::Event::SoftBreak => {
                quote!(::egui_commonmark_backend::soft_break(ui);)
            }
            pulldown_cmark::Event::HardBreak => {
                quote!(::egui_commonmark_backend::newline(ui);)
            }
            pulldown_cmark::Event::Rule => {
                quote!(
                ::egui_commonmark_backend::newline(ui);
                ui.add(egui::Separator::default().horizontal());
                // This does not add a new line, but instead ends the separator
                ::egui_commonmark_backend::newline(ui);
                )
            }
            pulldown_cmark::Event::TaskListMarker(checkbox) => {
                if options.mutable {
                    // FIXME: Unsupported for now
                    TokenStream::new()
                } else {
                    quote!(ui.add(::egui_commonmark_backend::ImmutableCheckbox::without_text(&mut #checkbox));)
                }
            }
        }
    }

    fn event_text(&mut self, text: CowStr) -> TokenStream {
        if let Some(image) = &mut self.image {
            image
                .alt_text
                .push(StyledText::new(self.text_style.clone(), text.to_string()));
        } else if let Some(block) = &mut self.fenced_code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            link.text
                .push(StyledText::new(self.text_style.clone(), text.to_string()));
        } else {
            let rich_text = self.richtext_tokenstream(&self.text_style.clone(), &text);
            return quote!(
                ui.label(#rich_text);
            );
        }

        TokenStream::new()
    }

    fn start_tag(&mut self, tag: pulldown_cmark::Tag, options: &CommonMarkOptions) -> TokenStream {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                let s = if self.should_insert_newline {
                    quote!(::egui_commonmark_backend::newline(ui);)
                } else {
                    TokenStream::new()
                };

                self.should_insert_newline = true;

                s
            }
            pulldown_cmark::Tag::Heading { level, .. } => {
                self.text_style.heading = Some(match level {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                });

                quote!(::egui_commonmark_backend::newline(ui);)
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.is_blockquote = true;
                TokenStream::new()
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                let mut s = TokenStream::new();
                if let pulldown_cmark::CodeBlockKind::Fenced(lang) = c {
                    self.fenced_code_block = Some(FencedCodeBlock {
                        lang: lang.to_string(),
                        content: "".to_string(),
                    });

                    if self.should_insert_newline {
                        s.extend(quote!(::egui_commonmark_backend::newline(ui);));
                    }
                }

                self.text_style.code = true;
                s
            }
            pulldown_cmark::Tag::List(point) => {
                if let Some(number) = point {
                    self.list.start_level_with_number(number);
                } else {
                    self.list.start_level_without_number();
                }

                TokenStream::new()
            }
            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.should_insert_newline = false;
                self.list.start_item(options)
            }
            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.should_insert_newline = false;
                let note = note.to_string();
                quote!(::egui_commonmark_backend::footnote(ui, #note);)
            }
            pulldown_cmark::Tag::Table(_) => {
                self.is_table = true;
                TokenStream::new()
            }
            pulldown_cmark::Tag::TableHead
            | pulldown_cmark::Tag::TableRow
            | pulldown_cmark::Tag::TableCell => TokenStream::new(),
            pulldown_cmark::Tag::Emphasis => {
                self.text_style.emphasis = true;
                TokenStream::new()
            }
            pulldown_cmark::Tag::Strong => {
                self.text_style.strong = true;
                // TODO: Return optional
                TokenStream::new()
            }
            pulldown_cmark::Tag::Strikethrough => {
                self.text_style.strikethrough = true;
                TokenStream::new()
            }
            pulldown_cmark::Tag::Link { dest_url, .. } => {
                self.link = Some(StyledLink {
                    destination: dest_url.to_string(),
                    text: Vec::new(),
                });
                TokenStream::new()
            }
            pulldown_cmark::Tag::Image { dest_url, .. } => {
                let tmp = Image::new(&dest_url, options);
                self.image = Some(StyledImage {
                    uri: tmp.uri,
                    alt_text: Vec::new(),
                });

                TokenStream::new()
            }
            pulldown_cmark::Tag::HtmlBlock | pulldown_cmark::Tag::MetadataBlock(_) => {
                TokenStream::new()
            }
        }
    }

    fn end_tag(
        &mut self,
        tag: pulldown_cmark::TagEnd,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> TokenStream {
        match tag {
            pulldown_cmark::TagEnd::Paragraph => {
                quote!( ::egui_commonmark_backend::newline(ui);)
            }
            pulldown_cmark::TagEnd::Heading { .. } => {
                let newline = quote!( ::egui_commonmark_backend::newline(ui););
                self.text_style.heading = None;
                newline
            }
            pulldown_cmark::TagEnd::BlockQuote => TokenStream::new(),
            pulldown_cmark::TagEnd::CodeBlock => self.end_code_block(cache, options, max_width),
            pulldown_cmark::TagEnd::List(_) => {
                let s = self.list.end_level();

                if !self.list.is_inside_a_list() {
                    self.should_insert_newline = true;
                }

                s
            }
            pulldown_cmark::TagEnd::Item
            | pulldown_cmark::TagEnd::FootnoteDefinition
            | pulldown_cmark::TagEnd::Table
            | pulldown_cmark::TagEnd::TableHead
            | pulldown_cmark::TagEnd::TableRow => TokenStream::new(),
            pulldown_cmark::TagEnd::TableCell => {
                // Ensure space between cells
                quote!(ui.label("  ");)
            }
            pulldown_cmark::TagEnd::Emphasis => {
                self.text_style.emphasis = false;
                TokenStream::new()
            }
            pulldown_cmark::TagEnd::Strong => {
                self.text_style.strong = false;
                TokenStream::new()
            }
            pulldown_cmark::TagEnd::Strikethrough => {
                self.text_style.strikethrough = false;
                TokenStream::new()
            }
            pulldown_cmark::TagEnd::Link { .. } => {
                if let Some(link) = self.link.take() {
                    let StyledLink { destination, text } = link;
                    // TODO: text
                    quote!(
                    ::egui_commonmark_backend::Link {
                        destination: #destination.to_owned(),
                        text: Vec::new()
                    }.end(ui, #cache);)
                } else {
                    TokenStream::new()
                }
            }
            pulldown_cmark::TagEnd::Image { .. } => {
                let mut stream = TokenStream::new();
                if let Some(image) = self.image.take() {
                    // FIXME: Try to reduce code duplication here
                    //
                    // FIXME: Split options into runtime options and static options
                    // options.max_width is dynamic but for instance options.show_alt_text_on_hover
                    // is static here and does not need to be included in the generated code
                    let StyledImage { uri, alt_text } = image;

                    stream.extend(quote!(
                    let response = ui.add(
                        egui::Image::from_uri(#uri)
                            .fit_to_original_size(1.0)
                            .max_width(options.max_width(ui)),
                    );
                    ));

                    if !alt_text.is_empty() && options.show_alt_text_on_hover {
                        let mut alt_text_stream = TokenStream::new();
                        for alt in alt_text {
                            let text = self.richtext_tokenstream(&alt.style, &alt.text);
                            alt_text_stream.extend(quote!(ui.label(#text);));
                        }

                        stream.extend(quote!(
                        response.on_hover_ui_at_pointer(|ui| {
                            #alt_text_stream
                        });));
                    }
                }
                stream
            }
            pulldown_cmark::TagEnd::HtmlBlock | pulldown_cmark::TagEnd::MetadataBlock(_) => {
                TokenStream::new()
            }
        }
    }

    fn end_code_block(
        &mut self,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if let Some(block) = self.fenced_code_block.take() {
            let lang = block.lang;
            let content = block.content;

            stream.extend(
                quote!(
                ::egui_commonmark_backend::FencedCodeBlock {lang: #lang.to_owned(), content: #content.to_owned()}
                    .end(ui, #cache, &options, max_width);
            ));

            self.text_style.code = false;
            if self.should_insert_newline {
                stream.extend(quote!(::egui_commonmark_backend::newline(ui);));
            }
        }

        stream
    }

    fn richtext_tokenstream(&mut self, s: &Style, text: &str) -> TokenStream {
        // Try to write a compact stream

        let mut stream = TokenStream::new();
        if let Some(level) = s.heading {
            stream.extend(quote!(egui::RichText::new(#text)));

            match level {
                0 => {
                    // no dumps_heading here as it does not depend on min_height and diff
                    stream.extend(quote!(.strong().heading()));
                }
                1 => {
                    self.dumps_heading = true;
                    stream.extend(quote!(.strong().size(min_height + diff * 0.835)));
                }
                2 => {
                    self.dumps_heading = true;
                    stream.extend(quote!(.strong().size(min_height + diff * 0.668)));
                }
                3 => {
                    self.dumps_heading = true;
                    stream.extend(quote!(.strong().size(min_height + diff * 0.501)));
                }
                4 => {
                    self.dumps_heading = true;
                    stream.extend(quote!(.size(min_height + diff * 0.334)));
                }
                // We only support 6 levels
                5.. => {
                    self.dumps_heading = true;
                    stream.extend(quote!(.size(min_height + diff * 0.167)));
                }
            }
        } else {
            stream.extend(quote!(egui::RichText::new(#text)));
        }

        if s.quote {
            stream.extend(quote!(.weak()));
        }

        if s.strong {
            stream.extend(quote!(.strong()));
        }

        if s.emphasis {
            stream.extend(quote!(.italics()));
        }

        if s.strikethrough {
            stream.extend(quote!(.strikethrough()));
        }

        if s.code {
            stream.extend(quote!(.code()));
        }

        stream
    }
}

fn dump_heading_heights() -> TokenStream {
    quote!(
    let max_height = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Heading)
        .map_or(32.0, |d| d.size);
    let min_height = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map_or(14.0, |d| d.size);
    let diff = max_height - min_height;
    )
}
