use eframe::egui;

// This is more of an test...
// Ensure that there are no newlines that should not be present when mixing markdown
// and egui widgets.
fn main() -> eframe::Result<()> {
    let mut cache = egui_commonmark::CommonMarkCache::default();

    eframe::run_simple_native(
        "Mixed egui and markdown",
        Default::default(),
        move |ctx, _frame| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let label = "Label!";
                ui.label(label);

                egui_commonmark::CommonMarkViewer::new("viewer").show(ui, &mut cache, "Markdown");

                ui.label(label);

                egui_commonmark::CommonMarkViewer::new("viewer").show(
                    ui,
                    &mut cache,
                    "# Markdown (Deliberate space above)",
                );

                ui.label(label);

                egui_commonmark::CommonMarkViewer::new("viewer").show(
                    ui,
                    &mut cache,
                    r#"1. aaa
2. aaa
3. bbb"#,
                );

                ui.label(label);

                egui_commonmark::CommonMarkViewer::new("viewer").show(
                    ui,
                    &mut cache,
                    r#"```rust
let x = 3;
````
                "#,
                );

                ui.label(label);

                egui_commonmark::CommonMarkViewer::new("viewer").show(
                    ui,
                    &mut cache,
                    r#"
A footnote [^F1]

[^F1]: The footnote
                "#,
                );

                ui.label(label);
                egui_commonmark::CommonMarkViewer::new("viewer").show(
                    ui,
                    &mut cache,
                    "---------------",
                );

                ui.label(label);
            });
        },
    )
}
