use eframe::egui;

macro_rules! m {
    ($ui:expr, $cache:expr,$($a:expr),* $(,)? ) => {
        $(
        $ui.label("Label!");
        #[cfg(feature = "macros")]
        {
            egui_commonmark_macros::commonmark!("n1", $ui, &mut $cache, $a);
        }
        #[cfg(not(feature = "macros"))]
        {
            egui_commonmark::CommonMarkViewer::new("viewer").show($ui, &mut $cache, $a);
        }
        )*
    };
}

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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    m!(
                        ui,
                        cache,
                        "Markdown *a*",
                        "# Markdown (Deliberate space above)",
                        "--------------------",
                        r#"
- Simple list 1
- Simple list 2
                        "#,
                        r#"
1. aaa
2. aaa
    - abb
    - acc
3. bbb
   - baa
                        "#,
                        r#"
```rust
let x = 3;
```
                        "#,
                        r#"
A footnote [^F1]

[^F1]: The footnote"#,
                        r#"
>
> Test
>
                        "#,
                        r#"
> [!TIP]
>
> Test
                        "#,
                        r#"

Column A   | Column B
-----------|----------
`item` `a1` | item b1
item a2 | item b2
item a3 | item b3
item a4 | item b4

                        "#,
                        r#"
 ![Rust logo](egui_commonmark/examples/rust-logo-128x128.png)
                        "#,
                        r#"
[Link to repo](https://github.com/lampsitter/egui_commonmark)
                        "#,
                    );

                    ui.label("Label!");
                });
            });
        },
    )
}
