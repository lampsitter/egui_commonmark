use egui::__run_test_ui;
use egui_commonmark_macros::commonmark_str;

// Check that it fails to compile when it is not able to find a file
fn main() {
    let mut cache = egui_commonmark_shared::CommonMarkCache::default();
    __run_test_ui(|ui| {
        commonmark_str!("a", ui, &mut cache, "foo.md");
    });
}
