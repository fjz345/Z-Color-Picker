use eframe::egui::{self, ScrollArea};
use log::{Level, Metadata, Record, SetLoggerError};
use std::sync::{Arc, Mutex, OnceLock};

static GLOBAL_LOG_BUFFER: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
pub struct LogCollector {
    pub buffer: Arc<Mutex<Vec<String>>>,
    delegate: Box<dyn log::Log>,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            delegate: Box::new(env_logger::Builder::from_env(env_logger::Env::default()).build()),
        }
    }
}

impl LogCollector {
    pub fn init() -> Result<Arc<Mutex<Vec<String>>>, SetLoggerError> {
        if let Some(buffer) = GLOBAL_LOG_BUFFER.get() {
            return Ok(buffer.clone());
        }

        let env_logger = env_logger::Builder::from_env(env_logger::Env::default()).build();

        let buffer = Arc::new(Mutex::new(Vec::new()));

        let collector = LogCollector {
            buffer: buffer.clone(),
            delegate: Box::new(env_logger),
        };

        // Set our collector as the logger
        log::set_boxed_logger(Box::new(collector))?;
        log::set_max_level(log::LevelFilter::Trace);

        GLOBAL_LOG_BUFFER.set(buffer.clone()).unwrap();

        Ok(buffer)
    }
}

impl log::Log for LogCollector {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.delegate.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // Forward to env_logger
            self.delegate.log(record);

            // Capture in our buffer
            let mut buf = self.buffer.lock().unwrap();
            buf.push(format!("[{}] {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {
        self.delegate.flush();
    }
}

pub fn ui_log_window(
    ui: &mut egui::Ui,
    log_buffer: Arc<Mutex<Vec<String>>>,
    scroll_to_bottom: &mut bool,
) {
    // Lock and clone logs for UI rendering
    let logs = {
        let buf = log_buffer.lock().unwrap();
        buf.clone()
    };

    // ScrollArea with vertical scrollbar and full size
    let scroll_response = ScrollArea::vertical()
        .auto_shrink([false; 2]) // Don't shrink smaller than contents
        .stick_to_bottom(*scroll_to_bottom)
        .show(ui, |ui| {
            // Fill available width & stretch height as needed
            ui.vertical(|ui| {
                for line in logs {
                    ui.label(line);
                }
            });
        });

    let has_scroll = scroll_response.content_size.y > scroll_response.inner_rect.size().y;

    if has_scroll {
        let scroll_pos = ui.ctx().input(|input| input.raw_scroll_delta.y);
        if scroll_pos > 0.0 {
            *scroll_to_bottom = false;
        } else if scroll_pos < 0.0 {
            *scroll_to_bottom = true;
        }
    } else {
        *scroll_to_bottom = true;
    }
}
