use std::iter::Peekable;

use egui_commonmark_backend::{
    alerts::Alert, misc::Style, pulldown::*, CodeBlock, CommonMarkOptions, Image,
};

use proc_macro2::TokenStream;
use pulldown_cmark::{CowStr, HeadingLevel};
use quote::quote;
use syn::Expr;

struct Newline {
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
            // Default as false as the first line should not have a newline above it
            should_start_newline: false,
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
        self.should_start_newline
    }

    #[must_use]
    pub fn try_insert_start(&self) -> TokenStream {
        if self.should_start_newline {
            quote!(egui_commonmark_backend::newline(ui);)
        } else {
            TokenStream::new()
        }
    }

    #[must_use]
    pub fn try_insert_end(&self) -> TokenStream {
        if self.can_insert_end() {
            quote!(egui_commonmark_backend::newline(ui);)
        } else {
            TokenStream::new()
        }
    }
}

struct ListLevel {
    current_number: Option<u64>,
}

#[derive(Default)]
pub(crate) struct List {
    items: Vec<ListLevel>,
    has_list_begun: bool,
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

        // To ensure that newlines are only inserted within the list and not before it
        if self.has_list_begun {
            stream.extend(quote!(egui_commonmark_backend::newline(ui);));
        } else {
            self.has_list_begun = true;
        }

        let len = self.items.len();
        if let Some(item) = self.items.last_mut() {
            let spaces = " ".repeat((len - 1) * options.indentation_spaces);
            stream.extend(quote!( ui.label(#spaces); ));

            if let Some(number) = &mut item.current_number {
                let num = number.to_string();
                stream.extend(quote!( egui_commonmark_backend::number_point(ui, #num);));
                *number += 1;
            } else if len > 1 {
                stream.extend(quote!( egui_commonmark_backend::bullet_point_hollow(ui);));
            } else {
                stream.extend(quote!( egui_commonmark_backend::bullet_point(ui);));
            }
        } else {
            unreachable!();
        }

        stream.extend(quote!( ui.add_space(4.0); ));
        stream
    }

    pub fn end_level(&mut self, newline: bool) -> TokenStream {
        let mut stream = TokenStream::new();
        self.items.pop();

        if self.items.is_empty() && newline {
            stream.extend(quote!( egui_commonmark_backend::newline(ui); ));
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

#[derive(Default)]
struct DefinitionList {
    is_first_item: bool,
    is_def_list_def: bool,
}

pub(crate) struct CommonMarkViewerInternal {
    curr_table: usize,
    text_style: Style,
    list: List,
    link: Option<StyledLink>,
    image: Option<StyledImage>,
    line: Newline,
    code_block: Option<CodeBlock>,
    is_list_item: bool,
    def_list: DefinitionList,
    is_table: bool,
    is_blockquote: bool,

    /// Informs that a calculation of heading sizes is required.
    /// This will dump min and max text size at the top of the macro output
    /// to reduce code duplication.
    dumps_heading: bool,
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
            is_table: false,
            is_blockquote: false,
            dumps_heading: false,
        }
    }
}

impl CommonMarkViewerInternal {
    pub fn show(&mut self, ui: Expr, cache: Expr, text: &str) -> TokenStream {
        let mut events = pulldown_cmark::Parser::new_ext(text, parser_options())
            .into_offset_iter()
            .enumerate()
            .peekable();

        let options = CommonMarkOptions::default();
        let mut stream = TokenStream::new();

        let mut event_stream = TokenStream::new();
        while let Some((i, (e, _))) = events.next() {
            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            let e = self.process_event(&mut events, e, &cache, &options);

            if i == 0 {
                self.line.should_start_newline = true;
            }

            event_stream.extend(e);
        }

        stream.extend(quote!(
            egui_commonmark_backend::prepare_show(#cache, ui.ctx());
            let options = egui_commonmark_backend::CommonMarkOptions::default();
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

        let crate_import = crate::resolve_backend_crate_import();
        // Place all code within a block to prevent it from leaking into unrelated code
        //
        // we manually rename #ui to ui to prevent borrowing issues if #ui is not named ui and
        // there is a different ui also in scope that is called ui
        quote!({
            #crate_import
            let ui: &mut egui::Ui = #ui;
            #heights
            #stream
        })
    }

    fn process_event<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        event: pulldown_cmark::Event,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = self.event(event, cache, options);

        stream.extend(self.item_list_wrapping(events, cache, options));
        stream.extend(self.def_list_def_wrapping(events, cache, options));
        stream.extend(self.table(events, cache, options));
        stream.extend(self.blockquote(events, cache, options));
        stream
    }

    fn def_list_def_wrapping<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.def_list.is_def_list_def {
            self.def_list.is_def_list_def = false;

            let item_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::DefinitionListDefinition)
            });

            let mut events_iter = item_events.into_iter().enumerate().peekable();

            let mut inner = TokenStream::new();

            stream.extend(self.line.try_insert_start());

            // Proccess a single event separately so that we do not insert spaces where we do not
            // want them
            self.line.should_start_newline = false;
            if let Some((_, (e, _))) = events_iter.next() {
                inner.extend(self.process_event(&mut events_iter, e, cache, options));
            }

            self.line.should_start_newline = true;
            self.line.should_end_newline = false;
            while let Some((_, (e, _))) = events_iter.next() {
                inner.extend(self.process_event(&mut events_iter, e, cache, options));
            }
            self.line.should_end_newline = true;

            let spaces = " ".repeat(options.indentation_spaces);
            stream.extend(quote!(ui.label(#spaces);));

            // Required to ensure that the content is aligned with the identation
            stream.extend(quote!(ui.horizontal_wrapped(|ui| {
                    #inner
            });));

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
                stream.extend(self.line.try_insert_end());
            }
        }

        stream
    }

    fn item_list_wrapping<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.is_list_item {
            self.is_list_item = false;

            let item_events = delayed_events_list_item(events);
            let mut events_iter = item_events.into_iter().enumerate().peekable();

            let mut inner = TokenStream::new();

            while let Some((_, (e, _))) = events_iter.next() {
                inner.extend(self.process_event(&mut events_iter, e, cache, options));
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
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::BlockQuote(_))
            });
            stream.extend(self.line.try_insert_start());

            self.line.should_start_newline = true;
            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                let Alert {
                    accent_color,
                    icon,
                    identifier,
                    identifier_rendered,
                } = alert;

                let mut inner = TokenStream::new();
                for (event, _) in collected_events.into_iter() {
                    inner.extend(self.event(event, cache, options));
                }

                let r = accent_color.r();
                let g = accent_color.g();
                let b = accent_color.b();
                let a = accent_color.a();
                stream.extend(quote!(
                egui_commonmark_backend::alert_ui(&egui_commonmark_backend::Alert {
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
                for (event, _) in collected_events {
                    inner.extend(self.event(event, cache, options));
                }
                self.text_style.quote = false;

                stream.extend(quote!(egui_commonmark_backend::blockquote(ui, ui.visuals().weak_text_color(), |ui| {#inner});));
            }

            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            stream.extend(self.line.try_insert_end());

            self.is_blockquote = false;
        }
        stream
    }

    fn table<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        let mut stream = TokenStream::new();
        if self.is_table {
            stream.extend(self.line.try_insert_start());

            let Table { header, rows } = parse_table(events);

            let mut header_stream = TokenStream::new();
            for col in header {
                let mut inner = TokenStream::new();
                for (e, _) in col {
                    self.line.should_start_newline = false;
                    self.line.should_end_newline = false;
                    inner.extend(self.event(e, cache, options));
                    self.line.should_start_newline = true;
                    self.line.should_end_newline = true;
                }

                header_stream.extend(quote!(ui.horizontal(|ui| {#inner});));
            }

            let mut content_stream = TokenStream::new();
            for row in rows {
                let mut row_stream = TokenStream::new();
                for col in row {
                    let mut inner = TokenStream::new();
                    for (e, _) in col {
                        self.line.should_start_newline = false;
                        self.line.should_end_newline = false;
                        inner.extend(self.event(e, cache, options));
                        self.line.should_start_newline = true;
                        self.line.should_end_newline = true;
                    }

                    row_stream.extend(quote!(ui.horizontal(|ui| {#inner});));
                }

                if !row_stream.is_empty() {
                    content_stream.extend(quote!(#row_stream ui.end_row();))
                }
            }

            let curr_table = self.curr_table;
            stream.extend(quote!(
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    let id = ui.id().with("_table").with(#curr_table);
                    egui::Grid::new(id).striped(true).show(ui, |ui| {

                    #header_stream

                    ui.end_row();

                    #content_stream
                    });
                });
            ));

            self.curr_table += 1;

            self.is_table = false;

            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            stream.extend(self.line.try_insert_end());
        }

        stream
    }

    fn event(
        &mut self,
        event: pulldown_cmark::Event,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(tag, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(tag, cache, options),
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
                quote!(egui_commonmark_backend::footnote_start(ui, #footnote);)
            }
            pulldown_cmark::Event::SoftBreak => {
                quote!(egui_commonmark_backend::soft_break(ui);)
            }
            pulldown_cmark::Event::HardBreak => {
                quote!(egui_commonmark_backend::newline(ui);)
            }
            pulldown_cmark::Event::Rule => {
                let mut stream = TokenStream::new();
                stream.extend(self.line.try_insert_start());

                let end = self.line.can_insert_end();
                stream.extend(quote!(egui_commonmark_backend::rule(ui, #end);));
                stream
            }
            pulldown_cmark::Event::TaskListMarker(checkbox) => {
                if options.mutable {
                    // FIXME: Unsupported for now
                    TokenStream::new()
                } else {
                    quote!(ui.add(egui_commonmark_backend::ImmutableCheckbox::without_text(&mut #checkbox));)
                }
            }

            pulldown_cmark::Event::InlineMath(_) | pulldown_cmark::Event::DisplayMath(_) => {
                TokenStream::new()
            }
        }
    }

    fn event_text(&mut self, text: CowStr) -> TokenStream {
        if let Some(image) = &mut self.image {
            image
                .alt_text
                .push(StyledText::new(self.text_style.clone(), text.to_string()));
        } else if let Some(block) = &mut self.code_block {
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
            pulldown_cmark::Tag::Paragraph => self.line.try_insert_start(),

            pulldown_cmark::Tag::Heading { level, .. } => {
                self.text_style.heading = Some(match level {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                });

                quote!(egui_commonmark_backend::newline(ui);)
            }
            pulldown_cmark::Tag::BlockQuote(_) => {
                self.is_blockquote = true;
                TokenStream::new()
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                match c {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        self.code_block = Some(CodeBlock {
                            lang: Some(lang.to_string()),
                            content: "".to_string(),
                        });
                    }
                    pulldown_cmark::CodeBlockKind::Indented => {
                        self.code_block = Some(CodeBlock {
                            lang: None,
                            content: "".to_string(),
                        });
                    }
                }

                self.line.try_insert_start()
            }

            pulldown_cmark::Tag::List(point) => {
                let mut stream = TokenStream::new();

                if !self.list.is_inside_a_list() && self.line.can_insert_start() {
                    stream.extend(quote!( egui_commonmark_backend::newline(ui);));
                }

                if let Some(number) = point {
                    self.list.start_level_with_number(number);
                } else {
                    self.list.start_level_without_number();
                }

                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
                stream
            }
            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.list.start_item(options)
            }
            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                let mut stream = self.line.try_insert_start();

                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
                let note = note.to_string();
                stream.extend(quote!(egui_commonmark_backend::footnote(ui, #note);));
                stream
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
            pulldown_cmark::Tag::DefinitionList => {
                let s = self.line.try_insert_start();
                self.def_list.is_first_item = true;
                s
            }
            pulldown_cmark::Tag::DefinitionListTitle => {
                if !self.def_list.is_first_item {
                    self.line.try_insert_start()
                } else {
                    self.def_list.is_first_item = false;
                    TokenStream::new()
                }
            }
            pulldown_cmark::Tag::DefinitionListDefinition => {
                self.def_list.is_def_list_def = true;
                TokenStream::new()
            }
        }
    }

    fn end_tag(
        &mut self,
        tag: pulldown_cmark::TagEnd,
        cache: &Expr,
        options: &CommonMarkOptions,
    ) -> TokenStream {
        match tag {
            pulldown_cmark::TagEnd::Paragraph => self.line.try_insert_end(),
            pulldown_cmark::TagEnd::Heading { .. } => {
                self.text_style.heading = None;
                self.line.try_insert_end()
            }
            pulldown_cmark::TagEnd::BlockQuote(_) => TokenStream::new(),
            pulldown_cmark::TagEnd::CodeBlock => self.end_code_block(cache),
            pulldown_cmark::TagEnd::List(_) => {
                let s = self.list.end_level(self.line.can_insert_end());

                if !self.list.is_inside_a_list() {
                    // Reset all the state and make it ready for the next list that occurs
                    self.list = List::default();
                }

                self.line.should_start_newline = true;
                self.line.should_end_newline = true;
                s
            }
            pulldown_cmark::TagEnd::FootnoteDefinition => {
                self.line.should_start_newline = true;
                self.line.should_end_newline = true;
                self.line.try_insert_end()
            }
            pulldown_cmark::TagEnd::Item
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
                    let mut text_stream = TokenStream::new();
                    for text_style in text {
                        text_stream
                            .extend(self.richtext_tokenstream(&text_style.style, &text_style.text));
                        text_stream.extend(quote!(,));
                    }

                    quote!(
                    egui_commonmark_backend::Link {
                        destination: #destination.to_owned(),
                        text: vec![#text_stream]
                    }.end(ui, #cache);)
                } else {
                    TokenStream::new()
                }
            }
            pulldown_cmark::TagEnd::Image { .. } => {
                let mut stream = TokenStream::new();
                if let Some(image) = self.image.take() {
                    // FIXME: Try to reduce code duplication here
                    let StyledImage { uri, alt_text } = image;

                    stream.extend(quote!(
                    let response = ui.add(
                        egui::Image::from_uri(#uri)
                            .fit_to_original_size(1.0)
                            .max_width(max_width)
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
            pulldown_cmark::TagEnd::DefinitionList => self.line.try_insert_end(),
            pulldown_cmark::TagEnd::DefinitionListTitle => TokenStream::new(),
            pulldown_cmark::TagEnd::DefinitionListDefinition => TokenStream::new(),
        }
    }

    fn end_code_block(&mut self, cache: &Expr) -> TokenStream {
        let mut stream = TokenStream::new();
        if let Some(block) = self.code_block.take() {
            let content = block.content;

            stream.extend(if let Some(lang) = block.lang {
                quote!(egui_commonmark_backend::CodeBlock {
                    lang: Some(#lang.to_owned()), content: #content.to_owned()}
                    .end(ui, #cache, &options, max_width);)
            } else {
                quote!(egui_commonmark_backend::CodeBlock {
                    lang: None, content: #content.to_owned()}
                    .end(ui, #cache, &options, max_width);)
            });

            stream.extend(self.line.try_insert_end());
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
