#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use proc_macro::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream, Result};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Expr, LitStr, Token};
// TODO: Remove

struct Parameters {
    ui: Expr,
    cache: Expr,
    // options: Expr,
    markdown: LitStr,
}

impl Parse for Parameters {
    fn parse(input: ParseStream) -> Result<Self> {
        let ui: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let cache: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        let markdown: LitStr = input.parse()?;

        Ok(Parameters {
            ui,
            cache,
            // options,
            markdown,
        })
    }
}

fn commonmark_impl(ui: Expr, cache: Expr, text: String) -> TokenStream {
    let stream = CommonMarkViewerInternal::new("id aaaaa".into()).show(ui, cache, &text);
    println!("fn main() {{");
    println!("{}", stream.to_string());
    println!("}}");

    stream
}

#[proc_macro]
pub fn commonmark(input: TokenStream) -> TokenStream {
    let Parameters {
        ui,
        cache,
        // options,
        markdown,
    } = parse_macro_input!(input as Parameters);

    commonmark_impl(ui, cache, markdown.value())
}

#[proc_macro]
pub fn commonmark_str(input: TokenStream) -> TokenStream {
    let Parameters {
        ui,
        cache,
        // options,
        markdown,
    } = parse_macro_input!(input as Parameters);

    let path = markdown.value();

    let Ok(md) = std::fs::read_to_string(path) else {
        // FIXME: error
        return TokenStream::new();
    };

    commonmark_impl(ui, cache, md)
}

use std::ops::Range;

use egui_commonmark::{elements::*, Alert, AlertBundle};
use egui_commonmark::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, TextStyle, Ui, Vec2};

use egui_commonmark::*;
use pulldown_cmark::{CowStr, HeadingLevel, Options};

struct ListLevel {
    current_number: Option<u64>,
}
#[derive(Default)]
struct List {
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

    pub fn start_item(
        &mut self,
        ui: &Expr,
        options: &crate::CommonMarkOptions,
    ) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        let len = self.items.len();
        if let Some(item) = self.items.last_mut() {
            let spaces = " ".repeat((len - 1) * options.indentation_spaces);
            stream.extend(quote!(
                egui_commonmark::elements::newline(#ui);
                #ui.label(#spaces);
            ));

            if let Some(number) = &mut item.current_number {
                let num = number.to_string();
                stream.extend(quote!( egui_commonmark::elements::number_point(#ui, #num);));
                *number += 1;
            } else if len > 1 {
                stream.extend(quote!( egui_commonmark::elements::bullet_point_hollow(#ui);));
            } else {
                stream.extend(quote!( egui_commonmark::elements::bullet_point(#ui);));
            }
        } else {
            unreachable!();
        }

        stream.extend(quote!( #ui.add_space(4.0); ));
        stream
    }

    pub fn end_level(&mut self, ui: &Expr) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        self.items.pop();

        if self.items.is_empty() {
            stream.extend(quote!( egui_commonmark::elements::newline(#ui); ));
        }

        stream
    }
}

type EventIteratorItem<'e> = (usize, (pulldown_cmark::Event<'e>, Range<usize>));

/// Parse events until a desired end tag is reached or no more events are found.
/// This is needed for multiple events that must be rendered inside a single widget
fn delayed_events<'e>(
    events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
    end_at: pulldown_cmark::TagEnd,
) -> Vec<(pulldown_cmark::Event<'e>, Range<usize>)> {
    let mut curr_event = events.next();
    let mut total_events = Vec::new();
    loop {
        if let Some(event) = curr_event.take() {
            total_events.push(event.1.clone());
            if let (_, (pulldown_cmark::Event::End(tag), _range)) = event {
                if end_at == tag {
                    return total_events;
                }
            }
        } else {
            return total_events;
        }

        curr_event = events.next();
    }
}

fn delayed_events_list_item<'e>(
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

struct Table<'e> {
    header: Row<'e>,
    rows: Vec<Row<'e>>,
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

fn parse_table<'e>(events: &mut impl Iterator<Item = EventIteratorItem<'e>>) -> Table<'e> {
    let mut all_events = delayed_events(events, pulldown_cmark::TagEnd::Table)
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

/// try to parse events as an alert quote block. This ill modify the events
/// to remove the parsed text that should not be rendered.
/// Assumes that the first element is a Paragraph
fn parse_alerts<'a>(
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

        let alert = alerts.try_get_alert(&alert_ident);

        if alert.is_some() {
            // remove the text that identifies it as an alert so that it won't end up in the
            // render
            //
            // FIMXE: performance improvement potential
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
fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
}

struct CommonMarkViewerInternal {
    pub source_id: Id,
    pub curr_table: usize,
    pub text_style: Style,
    pub list: List,
    pub link: Option<Link>,
    pub image: Option<Image>,
    pub should_insert_newline: bool,
    pub fenced_code_block: Option<FencedCodeBlock>,
    pub is_list_item: bool,
    pub is_table: bool,
    pub is_blockquote: bool,
    pub checkbox_events: Vec<CheckboxClickEvent>,
}

struct CheckboxClickEvent {
    checked: bool,
    span: Range<usize>,
}

impl CommonMarkViewerInternal {
    fn new(source_id: Id) -> Self {
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
        }
    }
}

impl CommonMarkViewerInternal {
    fn show(&mut self, ui: Expr, cache: Expr, text: &str) -> TokenStream {
        let mut events = pulldown_cmark::Parser::new_ext(text, parser_options())
            .into_offset_iter()
            .enumerate();

        let options = CommonMarkOptions::default();
        let mut stream = proc_macro2::TokenStream::new();

        // FIXME: To make it easier specify all uses at the top
        stream.extend(quote!(
            use egui::TextStyle;
        ));
        // struct CommonMarkViewerInternal {
        //     source_id: egui::Id,
        //     curr_table: usize,
        //     text_style: egui_commonmark::Style,
        //     list: egui_commonmark::elements::List,
        //     link: Option<egui_commonmark::Link>,
        //     image: Option<egui_commonmark::Image>,
        //     should_insert_newline: bool,
        //     fenced_code_block: Option<egui_commonmark::FencedCodeBlock>,
        //     is_list_item: bool,
        //     is_table: bool,
        //     is_blockquote: bool,
        // }

        // impl CommonMarkViewerInternal {
        //     fn new(source_id: egui::Id) -> Self {
        //         Self {
        //             source_id,
        //             curr_table: 0,
        //             text_style: egui_commonmark::Style::default(),
        //             list: egui_commonmark::elements::List::default(),
        //             link: None,
        //             image: None,
        //             should_insert_newline: true,
        //             is_list_item: false,
        //             fenced_code_block: None,
        //             is_table: false,
        //             is_blockquote: false,
        //         }
        //     }
        // }

        // let mut internal = CommonMarkViewerInternal::new("aaaaaaaaaa".into());
        //         ));

        // FIXME: max_width

        let mut event_stream = proc_macro2::TokenStream::new();
        while let Some((index, (e, src_span))) = events.next() {
            let e = self.process_event(&ui, &mut events, e, src_span, &cache, &options, 500.0);
            event_stream.extend(e);
        }

        stream.extend(quote!(
        let options = CommonMarkOptions::default();
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);
            #event_stream
        });
        ));

        // Place all code within a block to prevent it from leaking into unrelated code
        quote!({#stream}).into()
    }

    #[allow(clippy::too_many_arguments)]
    fn process_event<'e>(
        &mut self,
        ui: &Expr,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> proc_macro2::TokenStream {
        let mut stream = self.event(ui, event, src_span, cache, options, max_width);

        stream.extend(self.item_list_wrapping(events, max_width, cache, options, ui));
        stream.extend(self.fenced_code_block(events, max_width, cache, options, ui));
        stream.extend(self.table(events, cache, options, ui, max_width));
        stream.extend(self.blockquote(events, max_width, cache, options, ui));
        stream
    }

    fn item_list_wrapping<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &Expr,
        options: &CommonMarkOptions,
        ui: &Expr,
    ) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        if self.is_list_item {
            self.is_list_item = false;

            let item_events = delayed_events_list_item(events);
            let mut events_iter = item_events.into_iter().enumerate();

            let mut inner = proc_macro2::TokenStream::new();

            while let Some((_, (e, src_span))) = events_iter.next() {
                inner.extend(self.process_event(
                    ui,
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
            stream.extend(quote!(#ui.horizontal_wrapped(|ui| {
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
        ui: &Expr,
    ) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, pulldown_cmark::TagEnd::BlockQuote);

            if self.should_insert_newline {
                stream.extend(quote!( egui_commonmark::elements::newline(#ui);));
            }

            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                let Alert {
                    accent_color,
                    icon,
                    identifier,
                    identifier_rendered,
                } = alert;

                let mut inner = proc_macro2::TokenStream::new();
                for (event, src_span) in collected_events.into_iter() {
                    inner.extend(self.event(ui, event, src_span, cache, options, max_width));
                }

                let r = accent_color.r();
                let g = accent_color.g();
                let b = accent_color.b();
                let a = accent_color.a();
                // FIXME: Figure out what rgba function to use
                stream.extend(quote!(
                egui_commonmark::Alert {
                    accent_color: egui::Color32::from_rgba_premultiplied(#r, #g, #b, #a),
                    icon: #icon,
                    identifier: #identifier.to_owned(),
                    identifier_rendered: #identifier_rendered.to_owned()
                }.ui(#ui, |ui| {
                    #inner
                });));
            } else {
                let mut inner = proc_macro2::TokenStream::new();

                self.text_style.quote = true;
                for (event, src_span) in collected_events {
                    inner.extend(self.event(ui, event, src_span, cache, options, max_width));
                }
                self.text_style.quote = false;

                stream.extend(quote!(egui_commonmark::elements::blockquote(#ui, #ui.visuals().weak_text_color(), |ui| {#inner});));
            }

            if self.should_insert_newline {
                stream.extend(quote!( egui_commonmark::elements::newline(#ui);));
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
        ui: &Expr,
    ) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        while self.fenced_code_block.is_some() {
            if let Some((_, (e, src_span))) = events.next() {
                stream.extend(self.event(ui, e, src_span, cache, options, max_width));
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
        ui: &Expr,
        max_width: f32,
    ) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        if self.is_table {
            stream.extend(quote!( egui_commonmark::elements::newline(#ui);));

            let id = self.source_id.with(self.curr_table);
            self.curr_table += 1;

            let Table { header, rows } = parse_table(events);

            let mut header_stream = proc_macro2::TokenStream::new();
            for col in header {
                let mut inner = proc_macro2::TokenStream::new();
                for (e, src_span) in col {
                    self.should_insert_newline = false;
                    inner.extend(self.event(ui, e, src_span, cache, options, max_width));
                }

                header_stream.extend(quote!(#ui.horizontal(|ui| {inner});));
            }

            let mut content_stream = proc_macro2::TokenStream::new();
            for row in rows {
                for col in row {
                    let mut inner = proc_macro2::TokenStream::new();
                    for (e, src_span) in col {
                        self.should_insert_newline = false;
                        inner.extend(self.event(ui, e, src_span, cache, options, max_width));
                    }

                    content_stream.extend(quote!(#ui.horizontal(|ui| {inner});));
                }
            }

            let frame = quote!(

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::Grid::new(id).striped(true).show(ui, |ui| {
                    #header_stream
                    ui.end_row();
                    #content_stream
                    ui.end_row();

                    });
                });
            );

            self.is_table = false;
            self.should_insert_newline = true;
            stream.extend(quote!( egui_commonmark::elements::newline(#ui);));
        }

        stream
    }

    fn event(
        &mut self,
        ui: &Expr,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> proc_macro2::TokenStream {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => self.event_text(text, ui),
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                let s = self.event_text(text, ui);
                self.text_style.code = false;
                s
            }
            pulldown_cmark::Event::InlineHtml(_) | pulldown_cmark::Event::Html(_) => {
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                let footnote = footnote.to_string();
                quote!(egui_commonmark::elements::footnote_start(ui, #footnote);)
            }
            pulldown_cmark::Event::SoftBreak => {
                quote!(egui_commonmark::elements::soft_break(#ui);)
            }
            pulldown_cmark::Event::HardBreak => quote!(egui_commonmark::elements::newline(#ui);),
            pulldown_cmark::Event::Rule => {
                quote!(
                egui_commonmark::elements::newline(#ui);
                ui.add(egui::Separator::default().horizontal());
                // This does not add a new line, but instead ends the separator
                egui_commonmark::elements::newline(#ui);
                )
            }
            // pulldown_cmark::Event::TaskListMarker(mut checkbox) => {
            //     if options.mutable {
            //         if ui
            //             .add(egui::Checkbox::without_text(&mut checkbox))
            //             .clicked()
            //         {
            //             self.checkbox_events.push(CheckboxClickEvent {
            //                 checked: checkbox,
            //                 span: src_span,
            //             });
            //         }
            //     } else {
            //         ui.add(ImmutableCheckbox::without_text(&mut checkbox));
            //     }
            // }
            // TODO: Remove
            _ => proc_macro2::TokenStream::new(),
        }
    }

    fn event_text(&mut self, text: CowStr, ui: &Expr) -> proc_macro2::TokenStream {
        // FIXME: Store text with Style instead
        // let rich_text = self.text_style.to_richtext(ui, &text);
        if let Some(image) = &mut self.image {
            // image.alt_text.push(rich_text);
        } else if let Some(block) = &mut self.fenced_code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            // link.text.push(rich_text);
        } else {
            let rich_text = to_richtext_tokenstream(&self.text_style, ui, &text);
            return quote!(
                #ui.label(#rich_text);
            );
        }

        proc_macro2::TokenStream::new()
    }

    fn start_tag(
        &mut self,
        ui: &Expr,
        tag: pulldown_cmark::Tag,
        options: &CommonMarkOptions,
    ) -> proc_macro2::TokenStream {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                let s = if self.should_insert_newline {
                    quote!( egui_commonmark::elements::newline(#ui);)
                } else {
                    proc_macro2::TokenStream::new()
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

                quote!(egui_commonmark::elements::newline(#ui);)
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.is_blockquote = true;
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                let mut s = proc_macro2::TokenStream::new();
                if let pulldown_cmark::CodeBlockKind::Fenced(lang) = c {
                    self.fenced_code_block = Some(FencedCodeBlock {
                        lang: lang.to_string(),
                        content: "".to_string(),
                    });

                    if self.should_insert_newline {
                        s.extend(quote!(egui_commonmark::elements::newline(#ui);));
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

                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.should_insert_newline = false;
                self.list.start_item(ui, options)
            }
            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.should_insert_newline = false;
                let note = note.to_string();
                quote!(egui_commonmark::elements::footnote(#ui, #note);)
            }
            pulldown_cmark::Tag::Table(_) => {
                self.is_table = true;
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::TableHead
            | pulldown_cmark::Tag::TableRow
            | pulldown_cmark::Tag::TableCell => proc_macro2::TokenStream::new(),
            pulldown_cmark::Tag::Emphasis => {
                self.text_style.emphasis = true;
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::Strong => {
                self.text_style.strong = true;
                // TODO: Return optional
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::Strikethrough => {
                self.text_style.strikethrough = true;
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::Link { dest_url, .. } => {
                self.link = Some(Link {
                    destination: dest_url.to_string(),
                    text: Vec::new(),
                });
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::Image { dest_url, .. } => {
                self.image = Some(Image::new(&dest_url, options));
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::Tag::HtmlBlock | pulldown_cmark::Tag::MetadataBlock(_) => {
                proc_macro2::TokenStream::new()
            }
        }
    }

    fn end_tag(
        &mut self,
        ui: &Expr,
        tag: pulldown_cmark::TagEnd,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> proc_macro2::TokenStream {
        match tag {
            pulldown_cmark::TagEnd::Paragraph => {
                quote!( egui_commonmark::elements::newline(#ui);)
            }
            pulldown_cmark::TagEnd::Heading { .. } => {
                let newline = quote!( egui_commonmark::elements::newline(#ui););
                self.text_style.heading = None;
                newline
            }
            pulldown_cmark::TagEnd::BlockQuote => proc_macro2::TokenStream::new(),
            pulldown_cmark::TagEnd::CodeBlock => self.end_code_block(ui, cache, options, max_width),
            pulldown_cmark::TagEnd::List(_) => {
                let s = self.list.end_level(ui);

                if !self.list.is_inside_a_list() {
                    self.should_insert_newline = true;
                }

                s
            }
            pulldown_cmark::TagEnd::Item
            | pulldown_cmark::TagEnd::FootnoteDefinition
            | pulldown_cmark::TagEnd::Table
            | pulldown_cmark::TagEnd::TableHead
            | pulldown_cmark::TagEnd::TableRow => proc_macro2::TokenStream::new(),
            pulldown_cmark::TagEnd::TableCell => {
                // Ensure space between cells
                quote!(#ui.label("  ");)
            }
            pulldown_cmark::TagEnd::Emphasis => {
                self.text_style.emphasis = false;
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::TagEnd::Strong => {
                self.text_style.strong = false;
                proc_macro2::TokenStream::new()
            }
            pulldown_cmark::TagEnd::Strikethrough => {
                self.text_style.strikethrough = false;
                proc_macro2::TokenStream::new()
            }
            //     pulldown_cmark::TagEnd::Link { .. } => {
            //         if let Some(link) = self.link.take() {
            //             link.end(ui, cache);
            //         }
            //     }
            //     pulldown_cmark::TagEnd::Image { .. } => {
            //         if let Some(image) = self.image.take() {
            //             image.end(ui, options);
            //         }
            //     }
            pulldown_cmark::TagEnd::HtmlBlock | pulldown_cmark::TagEnd::MetadataBlock(_) => {
                proc_macro2::TokenStream::new()
            }
            //  TODO: Remove
            _ => proc_macro2::TokenStream::new(),
        }
    }

    fn end_code_block(
        &mut self,
        ui: &Expr,
        cache: &Expr,
        options: &CommonMarkOptions,
        max_width: f32,
    ) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();
        if let Some(block) = self.fenced_code_block.take() {
            let lang = block.lang;
            let content = block.content;

            stream.extend(
                quote!(
                egui_commonmark::FencedCodeBlock {lang: #lang.to_owned(), content: #content.to_owned()}
                    .end(#ui, #cache, &options, max_width);
            ));

            self.text_style.code = false;
            if self.should_insert_newline {
                stream.extend(quote!(egui_commonmark::elements::newline(#ui);));
            }
        }

        stream
    }
}

fn to_richtext_tokenstream(s: &Style, ui: &Expr, text: &str) -> proc_macro2::TokenStream {
    // Try to write a compact stream

    let mut stream = proc_macro2::TokenStream::new();
    if let Some(level) = s.heading {
        // FIXME: Write at top to reduce code duplication
        stream.extend(quote!(
        let max_height = #ui
            .style()
            .text_styles
            .get(&TextStyle::Heading)
            .map_or(32.0, |d| d.size);
        let min_height = #ui
            .style()
            .text_styles
            .get(&TextStyle::Body)
            .map_or(14.0, |d| d.size);
        let diff = max_height - min_height;
        ));

        stream.extend(quote!(egui::RichText::new(#text)));

        match level {
            0 => {
                stream.extend(quote!(.strong().heading()));
            }
            1 => {
                stream.extend(quote!(.strong().size(min_height + diff * 0.835)));
            }
            2 => {
                stream.extend(quote!(.strong().size(min_height + diff * 0.668)));
            }
            3 => {
                stream.extend(quote!(.strong().size(min_height + diff * 0.501)));
            }
            4 => {
                stream.extend(quote!(.size(min_height + diff * 0.334)));
            }
            // We only support 6 levels
            5.. => {
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

    quote!({#stream})
}
