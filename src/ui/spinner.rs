use std::io::{stderr, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossterm::{cursor, execute, queue, style::Print, terminal};

const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const FRAME_INTERVAL_MS: u64 = 80;

/// Background spinner that writes to stderr so stdout stays usable for JSON / pipes.
///
/// Tracks an `Arc<AtomicUsize>` that workers increment as they finish — the spinner
/// renders the running count as `(done/total)` next to elapsed time.
pub struct Spinner {
    stop: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    pub fn start(message: &'static str, total: usize) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let stop_for_thread = stop.clone();
        let progress_for_thread = progress.clone();

        let _ = execute!(stderr(), cursor::Hide);

        let handle = thread::spawn(move || {
            let start = Instant::now();
            let mut frame_idx = 0usize;

            loop {
                if stop_for_thread.load(Ordering::Relaxed) {
                    break;
                }

                let done = progress_for_thread.load(Ordering::Relaxed);
                let secs = start.elapsed().as_secs_f64();
                let frame = FRAMES[frame_idx];
                frame_idx = (frame_idx + 1) % FRAMES.len();

                let line = format!(
                    "  {} {} ({}/{}) [{:.1}s]",
                    frame, message, done, total, secs
                );

                let _ = queue!(
                    stderr(),
                    cursor::MoveToColumn(0),
                    terminal::Clear(terminal::ClearType::CurrentLine),
                    Print(line),
                );
                let _ = stderr().flush();

                thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
            }
        });

        Self {
            stop,
            progress,
            handle: Some(handle),
        }
    }

    /// Hand out the shared counter so workers can `fetch_add(1, Relaxed)` on completion.
    pub fn progress_handle(&self) -> Arc<AtomicUsize> {
        self.progress.clone()
    }

    /// Stop the render thread and clear the spinner line.
    pub fn finish(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.stop.swap(true, Ordering::Relaxed) {
            return;
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let _ = execute!(
            stderr(),
            cursor::MoveToColumn(0),
            terminal::Clear(terminal::ClearType::CurrentLine),
            cursor::Show,
        );
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.shutdown();
    }
}
