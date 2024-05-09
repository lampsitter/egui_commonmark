//! Shared code for egui_commonmark and egui_commonmark_macro. Breaking changes will happen and
//! should not be relied upon directly

#[doc(hidden)]
pub mod alerts;
#[doc(hidden)]
pub mod elements;
#[doc(hidden)]
pub mod misc;
#[cfg(feature = "pulldown-cmark")]
#[doc(hidden)]
pub mod pulldown;

// For ease of use in proc macros
#[doc(hidden)]
pub use {
    alerts::{alert_ui, Alert, AlertBundle},
    // Pretty much every single element in this module is used by the proc macros
    elements::*,
    misc::{prepare_show, CommonMarkCache, CommonMarkOptions, FencedCodeBlock, Image, Link},
};
