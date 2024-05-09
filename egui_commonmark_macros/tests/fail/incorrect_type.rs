use egui_commonmark_macros::commonmark;

// Ensure that the error message is sane
fn main() {
    let mut cache = egui_commonmark_shared::CommonMarkCache::default();
    let x = 3;
    commonmark!("a", x, &mut cache, "# Hello");
}
