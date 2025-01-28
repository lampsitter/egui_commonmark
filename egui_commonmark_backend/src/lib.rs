//! Shared code for egui_commonmark and egui_commonmark_macro. Breaking changes will happen and
//! should ideally not be relied upon. Only items that can been seen in this documentation
//! can be safely used directly.

#[doc(hidden)]
pub mod alerts;
#[doc(hidden)]
pub mod elements;
#[doc(hidden)]
pub mod misc;
#[doc(hidden)]
pub mod pulldown;

#[cfg(feature = "embedded_image")]
mod data_url_loader;

// For ease of use in proc macros
#[doc(hidden)]
pub use {
    alerts::{alert_ui, Alert, AlertBundle},
    // Pretty much every single element in this module is used by the proc macros
    elements::*,
    misc::{prepare_show, CodeBlock, CommonMarkOptions, Image, Link},
};

// The only struct that is allowed to use directly. (If one does not need egui_commonmark)
pub use misc::CommonMarkCache;

/// Takes [`egui::Ui`], the math text to be rendered and whether it is inline
pub type RenderMathFn = dyn Fn(&mut egui::Ui, &str, bool);
/// Takes [`egui::Ui`] and the html text to be rendered/used
pub type RenderHtmlFn = dyn Fn(&mut egui::Ui, &str);
