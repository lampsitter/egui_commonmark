use egui::__run_test_ui;
use egui_commonmark_macros::commonmark_str;

// Testing all the different examples should give fairly good coverage
fn main() {
    let mut cache = egui_commonmark_backend::CommonMarkCache::default();
    __run_test_ui(|ui| {
        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/hello_world.md"
        );

        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/headers.md"
        );

        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/lists.md"
        );

        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/code-blocks.md"
        );

        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/blockquotes.md"
        );

        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/tables.md"
        );
        commonmark_str!(
            ui,
            &mut cache,
            "../../../../egui_commonmark/examples/markdown/definition_list.md"
        );
    });
}
