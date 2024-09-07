use egui::__run_test_ui;
use egui_commonmark_macros::commonmark_str;

// Check a simple case and ensure that it returns a reponse
fn main() {
    let mut cache = egui_commonmark_backend::CommonMarkCache::default();
    __run_test_ui(|ui| {
        let _response: egui::InnerResponse<()> = commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark_macros/tests/file.md"
        );
    });
}
