use egui_commonmark_macros::commonmark;

// Ensure that the error message is sane
fn main() {
    let mut cache = egui_commonmark_backend::CommonMarkCache::default();
    egui::__run_test_ui(|ui| {
        commonmark!(ui, &cache, "# Hello");
    });
}
