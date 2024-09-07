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
