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
//! // Stores image handles between each frame
//! # __run_test_ui(|ui| {
//! let mut cache = CommonMarkCache::default();
//! CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
//! # });
//!
//! ```
//!
//! Remember to opt into the image formats you want to use!
//!
//! ```toml
//! image = { version = "0.24", default-features = false, features = ["png"] }
//! ```
//!
#![cfg_attr(feature = "document-features", doc = "# Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use egui::{self, Id};

mod parsers;

pub use egui_commonmark_shared::alerts::{Alert, AlertBundle};
pub use egui_commonmark_shared::misc::CommonMarkCache;

#[cfg(all(feature = "comrak", feature = "pulldown_cmark"))]
compile_error!("Cannot have multiple different parsing backends enabled at the same time");

#[cfg(not(any(feature = "comrak", feature = "pulldown_cmark")))]
compile_error!("Either the pulldown_cmark or comrak backend must be enabled");

#[cfg(feature = "macro")]
pub use egui_commonmark_macros::*;

#[cfg(feature = "macro")]
// Do not rely on this directly!
#[doc(hidden)]
pub use egui_commonmark_shared;

use egui_commonmark_shared::*;

#[derive(Debug)]
pub struct CommonMarkViewer {
    source_id: Id,
    options: CommonMarkOptions,
}

impl CommonMarkViewer {
    pub fn new(source_id: impl std::hash::Hash) -> Self {
        Self {
            source_id: Id::new(source_id),
            options: CommonMarkOptions::default(),
        }
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
    /// CommonMarkViewer::new("viewer").default_implicit_uri_scheme("https://example.org/");
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
    #[deprecated(note = "use `syntax_theme_light` or `syntax_theme_dark` instead")]
    pub fn syntax_theme(mut self, theme: String) -> Self {
        self.options.theme_light = theme.clone();
        self.options.theme_dark = theme;
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

    #[cfg(not(feature = "comrak"))] // not supported by the backend atm.
    /// Specify what kind of alerts are supported. This can also be used to localize alerts.
    ///
    /// By default [github flavoured markdown style alerts](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts)
    /// are used
    pub fn alerts(mut self, alerts: AlertBundle) -> Self {
        self.options.alerts = alerts;
        self
    }

    /// Shows rendered markdown
    pub fn show(
        self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &str,
    ) -> egui::InnerResponse<()> {
        egui_commonmark_shared::prepare_show(cache, ui.ctx());

        #[cfg(feature = "pulldown_cmark")]
        let (response, _) = parsers::pulldown::CommonMarkViewerInternal::new(self.source_id).show(
            ui,
            cache,
            &self.options,
            text,
            false,
        );

        #[cfg(feature = "comrak")]
        let response = parsers::comrak::CommonMarkViewerInternal::new(self.source_id).show(
            ui,
            cache,
            &self.options,
            text,
        );

        response
    }

    /// Shows rendered markdown, and allows the rendered ui to mutate the source text.
    ///
    /// The only currently implemented mutation is allowing checkboxes to be toggled through the ui.
    #[cfg(feature = "pulldown_cmark")]
    pub fn show_mut(
        mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        text: &mut String,
    ) -> egui::InnerResponse<()> {
        self.options.mutable = true;
        egui_commonmark_shared::prepare_show(cache, ui.ctx());

        let (response, checkmark_events) = parsers::pulldown::CommonMarkViewerInternal::new(
            self.source_id,
        )
        .show(ui, cache, &self.options, text, false);

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
    pub fn show_scrollable(self, ui: &mut egui::Ui, cache: &mut CommonMarkCache, text: &str) {
        egui_commonmark_shared::prepare_show(cache, ui.ctx());
        parsers::pulldown::CommonMarkViewerInternal::new(self.source_id).show_scrollable(
            ui,
            cache,
            &self.options,
            text,
        );
    }
}
