//! A commonmark viewer for egui
//!
//! # Example
//!
//! ```
//! # use egui_commonmark::*;
//! # use egui::__run_test_ui;
//! let markdown =
//! r"# Hello world
//!
//! * A list
//! * [ ] Checkbox
//! ";
//!
//! # __run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! CommonMarkViewer::new().show(ui, &mut cache, markdown);
//! # });
//!
//! ```
//!
//! Remember to opt into the image formats you want to use!
//!
//! ```toml
//! image = { version = "0.25", default-features = false, features = ["png"] }
//! ```
//! # FAQ
//!
//! ## URL is not displayed when hovering over a link
//!
//! By default egui does not show urls when you hover hyperlinks. To enable it,
//! you can do the following before calling any ui related functions:
//!
//! ```
//! # use egui::__run_test_ui;
//! # __run_test_ui(|ui| {
//! ui.style_mut().url_in_tooltip = true;
//! # });
//! ```
//!
//!
//! # Compile time evaluation of markdown
//!
//! If you want to embed markdown directly the binary then you can enable the `macros` feature.
//! This will do the parsing of the markdown at compile time and output egui widgets.
//!
//! ## Example
//!
//! ```
//! use egui_commonmark::{CommonMarkCache, commonmark};
//! # egui::__run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! let _response = commonmark!(ui, &mut cache, "# ATX Heading Level 1");
//! # });
//! ```
//!
//! Alternatively you can embed a file
//!
//!
//! ## Example
//!
//! ```rust,ignore
//! use egui_commonmark::{CommonMarkCache, commonmark_str};
//! # egui::__run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! commonmark_str!(ui, &mut cache, "content.md");
//! # });
//! ```
//!
//! For more information check out the documentation for
//! [egui_commonmark_macros](https://docs.rs/crate/egui_commonmark_macros/latest)
#![cfg_attr(feature = "document-features", doc = "# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use egui::{self, Id};

mod parsers;

pub use egui_commonmark_backend::RenderHtmlFn;
pub use egui_commonmark_backend::RenderMathFn;
pub use egui_commonmark_backend::alerts::{Alert, AlertBundle};
pub use egui_commonmark_backend::misc::CommonMarkCache;

#[cfg(feature = "macros")]
pub use egui_commonmark_macros::*;

#[cfg(feature = "macros")]
// Do not rely on this directly!
#[doc(hidden)]
pub use egui_commonmark_backend;

use egui_commonmark_backend::*;

#[derive(Debug, Default)]
pub struct CommonMarkViewer<'f> {
    options: CommonMarkOptions<'f>,
}

impl<'f> CommonMarkViewer<'f> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The amount of spaces a bullet point is indented. By default this is 4
    /// spaces.
    pub fn indentation_spaces(mut self, spaces: usize) -> Self {
        self.options.indentation_spaces = spaces;
        self
    }

    /// The maximum size images are allowed to be. They will be scaled down if
    /// they are larger
    pub fn max_image_width(mut self, width: Option<usize>) -> Self {
        self.options.max_image_width = width;
        self
    }

    /// The default width of the ui. This is only respected if this is larger than
    /// the [`max_image_width`](Self::max_image_width)
    pub fn default_width(mut self, width: Option<usize>) -> Self {
        self.options.default_width = width;
        self
    }

    /// Show alt text when hovering over images. By default this is enabled.
    pub fn show_alt_text_on_hover(mut self, show: bool) -> Self {
        self.options.show_alt_text_on_hover = show;
        self
    }

    /// Allows changing the default implicit `file://` uri scheme.
    /// This does nothing if [`explicit_image_uri_scheme`](`Self::explicit_image_uri_scheme`) is enabled
    ///
    /// # Example
    /// ```
    /// # use egui_commonmark::CommonMarkViewer;
    /// CommonMarkViewer::new().default_implicit_uri_scheme("https://example.org/");
    /// ```
    pub fn default_implicit_uri_scheme<S: Into<String>>(mut self, scheme: S) -> Self {
        self.options.default_implicit_uri_scheme = scheme.into();
        self
    }

    /// By default any image without a uri scheme such as `foo://` is assumed to
    /// be of the type `file://`. This assumption can sometimes be wrong or be done
    /// incorrectly, so if you want to always be explicit with the scheme then set
    /// this to `true`
    pub fn explicit_image_uri_scheme(mut self, use_explicit: bool) -> Self {
        self.options.use_explicit_uri_scheme = use_explicit;
        self
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Set the syntax theme to be used inside code blocks in light mode
    pub fn syntax_theme_light<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_light = theme.into();
        self
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Set the syntax theme to be used inside code blocks in dark mode
    pub fn syntax_theme_dark<S: Into<String>>(mut self, theme: S) -> Self {
        self.options.theme_dark = theme.into();
        self
    }

    /// Specify what kind of alerts are supported. This can also be used to localize alerts.
    ///
    /// By default [github flavoured markdown style alerts](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts)
    /// are used
    pub fn alerts(mut self, alerts: AlertBundle) -> Self {
        self.options.alerts = alerts;
        self
    }

    /// Allows rendering math. This has to be done manually as you might want a different
    /// implementation for the web and native.
    ///
    /// The example is template code for rendering a svg image. Make sure to enable the
    /// `egui_extras/svg` feature for the result to show up.
    ///
    /// ## Example
    ///
    /// ```
    /// # use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
    /// # use egui_commonmark::CommonMarkViewer;
    /// let mut math_images = Rc::new(RefCell::new(HashMap::new()));
    /// CommonMarkViewer::new()
    ///     .render_math_fn(Some(&move |ui, math, inline| {
    ///         let mut map = math_images.borrow_mut();
    ///         let svg = map
    ///             .entry(math.to_string())
    ///             .or_insert_with(|| {
    ///                 if inline {
    ///                     // render as inline
    ///                     // dummy data for the example
    ///                     Arc::new([0])
    ///                 } else {
    ///                     Arc::new([0])
    ///                 }
    ///             });
    ///
    ///     let uri = format!("{}.svg", egui::Id::from(math.to_string()).value());
    ///     ui.add(
    ///          egui::Image::new(egui::ImageSource::Bytes {
    ///             uri: uri.into(),
    ///             bytes: egui::load::Bytes::Shared(svg.clone()),
    ///          })
    ///          .fit_to_original_size(1.0),
    ///     );
    ///     }));
    /// ```
    pub fn render_math_fn(mut self, func: Option<&'f RenderMathFn>) -> Self {
        self.options.math_fn = func;
        self
    }

    /// Allows custom handling of html. Enabling this will disable plain text rendering
    /// of html blocks. Nodes are included in the provided text
    pub fn render_html_fn(mut self, func: Option<&'f RenderHtmlFn>) -> Self {
        self.options.html_fn = func;
        self
    }

    /// Shows rendered markdown
    pub fn show(
        self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &str,
    ) -> egui::InnerResponse<()> {
        egui_commonmark_backend::prepare_show(cache, ui.ctx());

        let (response, _) = parsers::pulldown::CommonMarkViewerInternal::new().show(
            ui,
            cache,
            &self.options,
            text,
            None,
        );

        response
    }

    /// Shows rendered markdown, and allows the rendered ui to mutate the source text.
    ///
    /// The only currently implemented mutation is allowing checkboxes to be toggled through the ui.
    pub fn show_mut(
        mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &mut String,
    ) -> egui::InnerResponse<()> {
        self.options.mutable = true;
        egui_commonmark_backend::prepare_show(cache, ui.ctx());

        let (response, checkmark_events) = parsers::pulldown::CommonMarkViewerInternal::new().show(
            ui,
            cache,
            &self.options,
            text,
            None,
        );

        // Update source text for checkmarks that were clicked
        for ev in checkmark_events {
            if ev.checked {
                text.replace_range(ev.span, "[x]")
            } else {
                text.replace_range(ev.span, "[ ]")
            }
        }

        response
    }

    /// Shows markdown inside a [`ScrollArea`].
    /// This function is much more performant than just calling [`show`] inside a [`ScrollArea`],
    /// because it only renders elements that are visible.
    ///
    /// # Caveat
    ///
    /// This assumes that the markdown is static. If it does change, you have to clear the cache
    /// by using [`clear_scrollable_with_id`](CommonMarkCache::clear_scrollable_with_id) or
    /// [`clear_scrollable`](CommonMarkCache::clear_scrollable). If the content changes every frame,
    /// it's faster to call [`show`] directly.
    ///
    /// [`ScrollArea`]: egui::ScrollArea
    /// [`show`]: crate::CommonMarkViewer::show
    #[doc(hidden)] // Buggy in scenarios more complex than the example application
    #[cfg(feature = "pulldown_cmark")]
    pub fn show_scrollable(
        self,
        source_id: impl std::hash::Hash,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &str,
    ) {
        egui_commonmark_backend::prepare_show(cache, ui.ctx());
        parsers::pulldown::CommonMarkViewerInternal::new().show_scrollable(
            Id::new(source_id),
            ui,
            cache,
            &self.options,
            text,
        );
    }
}

pub(crate) struct ListLevel {
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

    pub fn start_item(&mut self, ui: &mut egui::Ui, options: &CommonMarkOptions) {
        // To ensure that newlines are only inserted within the list and not before it
        if self.has_list_begun {
            newline(ui);
        } else {
            self.has_list_begun = true;
        }

        let len = self.items.len();
        if let Some(item) = self.items.last_mut() {
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

        ui.add_space(4.0);
    }

    pub fn end_level(&mut self, ui: &mut egui::Ui, insert_newline: bool) {
        self.items.pop();

        if self.items.is_empty() && insert_newline {
            newline(ui);
        }
    }
}
