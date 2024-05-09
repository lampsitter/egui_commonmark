//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --features better_syntax_highlighting,svg,fetch`
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --features better_syntax_highlighting,svg,fetch -- dark`

use eframe::egui;
use egui_commonmark::*;
use egui_commonmark_macro::*;

struct App {
    cache: CommonMarkCache,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Embed text directly
                commonmark!(
                    ui,
                    &mut self.cache,
                    r#"
# hello this is the `first` working text

## h2

### h3

__Very__ exciting _indeed_!

```rs
fn main() {
    println!("yay!");
}
```

> yeep
>
> jjj

> [!NOTE]
> note


1.
    1. Lorem ipsum dolor sit amet, consectetur __adipiscing elit, sed__ do
    eiusmod tempor incididunt _ut_ labore ~~et~~ dolore magna aliqua. Ut enim
    ad minim veniam, quis nostrud exercitation
    2. Lorem ipsum dolor sit amet, consectetur __adipiscing elit, sed__ do
    eiusmod tempor incididunt _ut_ labore ~~et~~ dolore magna aliqua. Ut enim
    ad minim veniam, quis nostrud exercitation
        - Lorem ipsum dolor sit amet, consectetur __adipiscing elit, sed__ do
        eiusmod tempor incididunt _ut_ labore ~~et~~ dolore magna aliqua. Ut enim
        ad minim veniam, quis nostrud exercitation



"#
                );
                // or from a file like include_str!
                commonmark_str!(ui, &mut self.cache, "markdown/hello_world.md");
            });
        });
    }
}

#[cfg(feature = "comrak")]
const BACKEND: &str = "comrak";
#[cfg(feature = "pulldown_cmark")]
const BACKEND: &str = "pulldown_cmark";

fn main() {
    let mut args = std::env::args();
    args.next();
    let use_dark_theme = if let Some(theme) = args.next() {
        if theme == "light" {
            false
        } else {
            theme == "dark"
        }
    } else {
        false
    };

    eframe::run_native(
        &format!("Markdown viewer (backend '{}')", BACKEND),
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(if use_dark_theme {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            Box::new(App {
                cache: CommonMarkCache::default(),
            })
        }),
    )
    .unwrap();
}
