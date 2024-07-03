use egui_commonmark_macros::commonmark;

// Ensure that the error message is sane
fn main() -> eframe::Result {
    let mut cache = egui_commonmark_backend::CommonMarkCache::default();
    let x = 3;
    commonmark!("a", x, &mut cache, "# Hello");
}
