use eframe::egui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
    curr_page: usize,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let p1 = r#"# Page 1
Check out the [next](#next) page."#;
        let p2 = r#"# Page 2
Check out the [previous](#prev) page."#;

        let p = [p1, p2];
        if self.cache.get_link_hook("#next").unwrap() {
            self.curr_page = 1;
        } else if self.cache.get_link_hook("#prev").unwrap() {
            self.curr_page = 0;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new("viewer").show(ui, &mut self.cache, p[self.curr_page]);
            });
        });
    }
}

fn main() {
    let mut cache = CommonMarkCache::default();
    cache.add_link_hook("#next");
    cache.add_link_hook("#prev");
    eframe::run_native(
        "Markdown link hooks",
        eframe::NativeOptions::default(),
        Box::new(|_| {
            Box::new(App {
                cache,
                curr_page: 0,
            })
        }),
    ).unwrap();
}
