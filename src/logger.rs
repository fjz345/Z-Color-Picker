use eframe::egui::{
    self, Color32, Label, Rect, RichText, ScrollArea, Sense, TextWrapMode, Vec2, WidgetText,
};
use log::{Metadata, Record, SetLoggerError};
use std::sync::{Arc, Mutex, OnceLock};

static GLOBAL_LOG_BUFFER: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();
const MAX_LOG_LINES: usize = 2000;
const DEFAULT_LOG_FILTER: &str = "info,z_color_picker=debug";

fn default_env() -> env_logger::Env<'static> {
    env_logger::Env::default().default_filter_or(DEFAULT_LOG_FILTER)
}

pub struct LogCollector {
    pub buffer: Arc<Mutex<Vec<String>>>,
    delegate: Box<dyn log::Log>,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            delegate: Box::new(env_logger::Builder::from_env(default_env()).build()),
        }
    }
}

impl LogCollector {
    pub fn init() -> Result<Arc<Mutex<Vec<String>>>, SetLoggerError> {
        if let Some(buffer) = GLOBAL_LOG_BUFFER.get() {
            return Ok(buffer.clone());
        }

        let env_logger = env_logger::Builder::from_env(default_env()).build();

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
            let line = normalize_log_line(record.level(), record.args());
            push_capped(&mut self.buffer.lock().unwrap(), line);
        }
    }

    fn flush(&self) {
        self.delegate.flush();
    }
}

pub fn ui_log_window(ui: &mut egui::Ui, log_buffer: Arc<Mutex<Vec<String>>>) {
    let num_lines = log_buffer.lock().unwrap().len();
    let row_height = ui.text_style_height(&egui::TextStyle::Body);

    // ScrollArea with vertical scrollbar and full size
    ScrollArea::vertical()
        .auto_shrink([false; 2]) // Don't shrink smaller than contents
        .stick_to_bottom(true)
        .show_rows(ui, row_height, num_lines, |ui, row_range| {
            // Copy only the visible rows and release the lock before drawing: anything
            // that logs while the lock is held would deadlock in LogCollector::log.
            let visible: Vec<String> = {
                let buf = log_buffer.lock().unwrap();
                let end = row_range.end.min(buf.len());
                buf[row_range.start.min(end)..end].to_vec()
            };
            for line in visible {
                let mut text = RichText::new(&line);
                if let Some(color) = log_line_color(&line, ui.visuals()) {
                    text = text.color(color);
                }
                ui.add(Label::new(text).truncate());
            }
        });
}

pub fn ui_log_header(
    ui: &mut egui::Ui,
    is_expanded: bool,
    log_buffer: &Mutex<Vec<String>>,
) -> egui::Response {
    let height = ui.spacing().interact_size.y;
    let gap = ui.spacing().item_spacing.x;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());
    let color = ui.style().interact(&response).text_color();

    let chevron_rect = Rect::from_min_size(rect.min, Vec2::splat(height)).shrink(0.3 * height);
    ui.painter().add(chevron(chevron_rect, is_expanded, color));

    let title = WidgetText::from(RichText::new("Log").strong().color(color)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let title_pos = egui::pos2(
        chevron_rect.right() + gap,
        rect.center().y - 0.5 * title.size().y,
    );
    let last_line_left = title_pos.x + title.size().x + 2.0 * gap;
    ui.painter().galley(title_pos, title, color);

    let last_line = if is_expanded {
        None
    } else {
        log_buffer.lock().unwrap().last().cloned()
    };
    if let Some(line) = last_line {
        let mut text = RichText::new(&line);
        if let Some(line_color) = log_line_color(&line, ui.visuals()) {
            text = text.color(line_color);
        }
        let galley = WidgetText::from(text).into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            (rect.right() - last_line_left).max(0.0),
            egui::TextStyle::Body,
        );
        let pos = egui::pos2(last_line_left, rect.center().y - 0.5 * galley.size().y);
        ui.painter().galley(pos, galley, ui.visuals().text_color());
    }
    response
}

fn chevron(rect: Rect, is_expanded: bool, color: Color32) -> egui::Shape {
    let points = if is_expanded {
        vec![rect.left_top(), rect.right_top(), rect.center_bottom()]
    } else {
        vec![rect.left_bottom(), rect.center_top(), rect.right_bottom()]
    };
    egui::Shape::convex_polygon(points, color, egui::Stroke::NONE)
}

fn normalize_log_line(level: log::Level, message: impl std::fmt::Display) -> String {
    format!("[{level}] {message}").replace(['\r', '\n'], " ")
}

fn push_capped(buf: &mut Vec<String>, line: String) {
    buf.push(line);
    // Drop the oldest half at once so trimming stays amortized O(1) per line.
    if buf.len() > MAX_LOG_LINES {
        let excess = buf.len() - MAX_LOG_LINES / 2;
        buf.drain(..excess);
    }
}

fn log_line_color(line: &str, visuals: &egui::Visuals) -> Option<Color32> {
    if line.starts_with("[ERROR]") {
        Some(visuals.error_fg_color)
    } else if line.starts_with("[WARN]") {
        Some(visuals.warn_fg_color)
    } else if line.starts_with("[DEBUG]") || line.starts_with("[TRACE]") {
        Some(visuals.weak_text_color())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered_buffer(count: usize) -> Vec<String> {
        let mut buf = Vec::new();
        for i in 0..count {
            push_capped(&mut buf, i.to_string());
        }
        buf
    }

    #[test]
    fn push_capped_never_exceeds_max() {
        let mut buf = Vec::new();
        for i in 0..5000 {
            push_capped(&mut buf, i.to_string());
            assert!(buf.len() <= MAX_LOG_LINES);
        }
    }

    #[test]
    fn push_capped_over_cap_keeps_newest_half() {
        let buf = numbered_buffer(MAX_LOG_LINES + 1);
        let expected: Vec<String> = (MAX_LOG_LINES / 2 + 1..=MAX_LOG_LINES)
            .map(|i| i.to_string())
            .collect();
        assert_eq!(buf.len(), MAX_LOG_LINES / 2);
        assert_eq!(buf.last(), Some(&MAX_LOG_LINES.to_string()));
        assert_eq!(buf, expected);
    }

    #[test]
    fn push_capped_below_cap_keeps_all_in_order() {
        let buf = numbered_buffer(10);
        let expected: Vec<String> = (0..10).map(|i| i.to_string()).collect();
        assert_eq!(buf, expected);
    }

    #[test]
    fn normalize_log_line_flattens_newlines() {
        assert_eq!(normalize_log_line(log::Level::Debug, "a\nb"), "[DEBUG] a b");
        assert_eq!(
            normalize_log_line(log::Level::Info, "a\r\nb\n"),
            "[INFO] a  b "
        );
    }

    #[test]
    fn log_line_color_by_level_prefix() {
        let visuals = egui::Visuals::dark();
        assert_eq!(
            log_line_color("[ERROR] x", &visuals),
            Some(visuals.error_fg_color)
        );
        assert_eq!(
            log_line_color("[WARN] x", &visuals),
            Some(visuals.warn_fg_color)
        );
        assert_eq!(
            log_line_color("[DEBUG] x", &visuals),
            Some(visuals.weak_text_color())
        );
        assert_eq!(
            log_line_color("[TRACE] x", &visuals),
            Some(visuals.weak_text_color())
        );
        assert_eq!(log_line_color("[INFO] x", &visuals), None);
    }

    fn header_texts(is_expanded: bool, log_buffer: &Mutex<Vec<String>>) -> Vec<(String, Color32)> {
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default()
                .show(ctx, |ui| ui_log_header(ui, is_expanded, log_buffer));
        });
        output
            .shapes
            .into_iter()
            .filter_map(|clipped| match clipped.shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.galley.job.sections[0].format.color,
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn collapsed_log_header_shows_only_the_last_line_in_its_level_color() {
        let log_buffer = Mutex::new(vec!["[INFO] first".to_owned(), "[WARN] last".to_owned()]);
        let warn = egui::Context::default().style().visuals.warn_fg_color;

        let collapsed = header_texts(false, &log_buffer);
        let expanded = header_texts(true, &log_buffer);

        let texts = |shown: &[(String, Color32)]| -> Vec<String> {
            shown.iter().map(|(text, _)| text.clone()).collect()
        };
        assert_eq!(texts(&collapsed), ["Log", "[WARN] last"]);
        assert_eq!(collapsed[1].1, warn);
        assert_eq!(texts(&expanded), ["Log"]);
        assert_eq!(texts(&header_texts(false, &Mutex::default())), ["Log"]);
    }

    #[test]
    fn default_filter_keeps_app_debug_and_drops_dependency_debug() {
        let logger = env_logger::Builder::new()
            .parse_filters(DEFAULT_LOG_FILTER)
            .build();
        let enabled = |target: &str, level: log::Level| {
            log::Log::enabled(
                &logger,
                &Metadata::builder().target(target).level(level).build(),
            )
        };
        assert!(enabled("z_color_picker::preset", log::Level::Debug));
        assert!(!enabled("glutin_winit", log::Level::Debug));
        assert!(enabled("glutin_winit", log::Level::Warn));
        assert!(enabled("eframe", log::Level::Info));
    }
}
