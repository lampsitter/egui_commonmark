//! Demonstrates incremental markdown rendering in `egui_commonmark`.
//!
//! This example simulates a live markdown stream by appending characters from a
//! fixed source document every frame. It is useful for visually checking parser
//! behavior during partial/incomplete input and for observing render/cache cost
//! while content grows.
//!
//! Controls:
//! - `Start at offset`: start streaming from an initial character position.
//! - `Speed`: number of characters appended per frame.
//! - `Play/Pause`: toggle continuous appending.
//! - `Reset`: clear rendered content and restart from the selected offset.
//! - `frame`: jump to a synthetic frame index for deterministic reproduction.

use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use std::time::Instant;


// The maximum number of frames (char appends) to simulate streaming.
const MAX_FRAMES: usize = 10_000_000;

struct StreamingApp {
    state: StreamingState,
    frame_idx: usize,
    last_cache_regen_ms: f64,
    offset_start_chars: usize,
    chars_per_frame: usize,
    is_playing: bool,
}

impl eframe::App for StreamingApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.is_playing {
            self.state.append_chars(self.chars_per_frame);
        }

        self.render_controls(ui);
        let cache_regen_start = Instant::now();
        self.render_markdown(ui);
        self.last_cache_regen_ms = cache_regen_start.elapsed().as_secs_f64() * 1000.0;

        if self.is_playing {
            println!(
                "frame {}: chars: {}, cache regen: {:.3} ms",
                self.frame_idx, self.state.markdown.chars().count(), self.last_cache_regen_ms
            );

            self.frame_idx += 1;

            // To make sure the UI is repainted to see the streaming changes.
            ui.request_repaint();
        }
    }
}

impl StreamingApp {
    fn render_controls(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("streaming_top_panel").show_inside(ui, |ui| {
            egui::Grid::new("streaming_controls_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Start at offset:");
                    ui.add(
                        egui::Slider::new(
                            &mut self.offset_start_chars,
                            0..=self.state.source_char_len,
                        )
                        .show_value(true),
                    );
                    ui.end_row();

                    ui.label("Speed:");
                    ui.add(
                        egui::Slider::new(&mut self.chars_per_frame, 1..=1_000).text("chars/frame"),
                    );
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                if ui
                    .button(if self.is_playing { "Pause" } else { "Play" })
                    .clicked()
                {
                    self.is_playing = !self.is_playing;
                }
                if ui.button("Reset").clicked() {
                    self.state.reset(self.offset_start_chars);
                    self.frame_idx = 0;
                }

                let frame_input = ui.add(
                    egui::DragValue::new(&mut self.frame_idx)
                        .prefix("frame: ")
                        .speed(1.0)
                        .range(0..=MAX_FRAMES),
                );
                if frame_input.changed() {
                    self.state.seek_to_frame(
                        self.offset_start_chars,
                        self.chars_per_frame,
                        self.frame_idx,
                        MAX_FRAMES,
                    );
                }
                ui.label(format!("chars: {}", self.state.markdown.chars().count()));
                ui.label(format!("cache regen: {:.3} ms", self.last_cache_regen_ms));
            });
        });
    }

    fn render_markdown(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(egui::Id::new("streaming_scroll"))
                .auto_shrink([false, true])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    CommonMarkViewer::new().show(ui, &mut self.state.cache, &self.state.markdown);
                });
        });
    }
}

struct StreamingState {
    cache: CommonMarkCache,
    source: String,
    markdown: String,
    source_char_len: usize,
    consumed_chars: usize,
    consumed_bytes: usize,
}

impl StreamingState {
    fn new(source: String) -> Self {
        let source_char_len = source.chars().count();
        let mut markdown = String::with_capacity(source.len());
        markdown.push_str(&source);

        Self {
            cache: CommonMarkCache::default(),
            source,
            markdown,
            source_char_len,
            consumed_chars: 0,
            consumed_bytes: 0,
        }
    }

    fn reset(&mut self, initial_chars: usize) {
        let initial_chars = initial_chars.min(self.source_char_len);
        let initial_bytes = byte_index_for_char_count(&self.source, initial_chars);
        self.cache = CommonMarkCache::default();
        self.markdown.clear();
        self.markdown.push_str(&self.source[..initial_bytes]);
        self.consumed_chars = initial_chars;
        self.consumed_bytes = initial_bytes;
    }

    fn append_chars(&mut self, count: usize) {
        if self.source_char_len == 0 || count == 0 {
            return;
        }

        let mut remaining = count;
        while remaining > 0 {
            if self.consumed_chars >= self.source_char_len {
                self.consumed_chars = 0;
                self.consumed_bytes = 0;
            }

            let available = self.source_char_len - self.consumed_chars;
            let step = remaining.min(available);
            let target_chars = self.consumed_chars + step;
            let target_bytes = byte_index_for_char_count(&self.source, target_chars);

            self.markdown
                .push_str(&self.source[self.consumed_bytes..target_bytes]);
            self.consumed_chars = target_chars;
            self.consumed_bytes = target_bytes;
            remaining -= step;
        }
    }

    fn seek_to_frame(
        &mut self,
        start_chars: usize,
        chars_per_frame: usize,
        frame_idx: usize,
        max_chars: usize,
    ) {
        let start = start_chars.min(self.source_char_len);
        let appended = frame_idx.saturating_mul(chars_per_frame);
        let target_total = start.saturating_add(appended).min(max_chars);
        self.cache = CommonMarkCache::default();
        self.markdown.clear();

        if self.source_char_len == 0 {
            self.consumed_chars = 0;
            self.consumed_bytes = 0;
            return;
        }

        let full_repeats = target_total / self.source_char_len;
        let tail_chars = target_total % self.source_char_len;
        for _ in 0..full_repeats {
            self.markdown.push_str(&self.source);
        }
        let tail_bytes = byte_index_for_char_count(&self.source, tail_chars);
        self.markdown.push_str(&self.source[..tail_bytes]);

        self.consumed_chars = tail_chars;
        self.consumed_bytes = tail_bytes;
    }
}

/// Converts a character count into a valid UTF-8 byte index for `input`.
///
/// Streaming state is tracked in characters, but Rust string slicing uses byte
/// offsets. This helper keeps slices on char boundaries to avoid panics.
fn byte_index_for_char_count(input: &str, char_count: usize) -> usize {
    if char_count == 0 {
        return 0;
    }

    input
        .char_indices()
        .nth(char_count)
        .map_or(input.len(), |(byte_index, _)| byte_index)
}

fn main() {
    let source_markdown = include_str!("markdown/scroll_to_heading.md").to_owned();

    eframe::run_native(
        "egui_commonmark streaming example",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| {
            Ok(Box::new(StreamingApp {
                state: StreamingState::new(source_markdown),
                frame_idx: 0,
                last_cache_regen_ms: 0.0,
                offset_start_chars: 0,
                chars_per_frame: 100,
                is_playing: true,
            }))
        }),
    )
    .unwrap();
}