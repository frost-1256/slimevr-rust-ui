//! `--smoke-test` harness: walks every page so a headless CI run lays out all
//! widgets, then closes the window with a success marker on stdout.

use std::time::{Duration, Instant};

use eframe::egui;

use crate::page::Page;

pub struct SmokeState {
    enabled: bool,
    deadline: Option<Instant>,
    page_timer: Instant,
}

impl SmokeState {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            deadline: None,
            page_timer: Instant::now(),
        }
    }

    /// Close the window after `after`.
    pub fn arm(&mut self, after: Duration) {
        self.deadline = Some(Instant::now() + after);
    }

    /// Advance the page tour and schedule the next repaint. Runs once per
    /// frame, after the UI has been laid out.
    pub fn tick(&mut self, ctx: &egui::Context, page: &mut Page) {
        if !self.enabled {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if self.page_timer.elapsed() >= Duration::from_millis(280) {
            self.page_timer = Instant::now();
            let index = Page::ALL
                .iter()
                .position(|(candidate, _)| candidate == page)
                .unwrap_or(0);
            *page = Page::ALL[(index + 1) % Page::ALL.len()].0;
            log::info!("smoke page: {}", page.label());
        }
        // Give the socket a moment, then exit successfully.
        ctx.request_repaint_after(Duration::from_millis(50));
        if let Some(deadline) = self.deadline {
            if Instant::now() >= deadline {
                self.deadline = None;
                println!("smoke-ok");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
}
