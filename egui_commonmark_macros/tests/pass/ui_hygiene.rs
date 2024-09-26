use egui::__run_test_ui;
use egui_commonmark_macros::commonmark;

// Check hygiene of the ui expression
fn main() {
    __run_test_ui(|ui| {
    let mut cache = egui_commonmark_backend::CommonMarkCache::default();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::none().show(ui, |not_named_ui| {
                commonmark!(not_named_ui, &mut cache, "# Hello, World");
            })
        });
    });
}
