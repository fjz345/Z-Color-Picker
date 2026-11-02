use std::{ops::Rem, time::Instant};

use arboard::{Clipboard, ImageData};
use eframe::egui::{self, pos2, vec2, ColorImage, Pos2, Rect, Vec2};

use crate::{error::Result, image_processing::FramePixelRead};

pub fn write_string_to_clipboard(text: String) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(&text)?;

    log::info!("Clipboard set to: {}", text);
    Ok(())
}

pub fn write_pixels_to_clipboard(image_data: ImageData) -> Result<()> {
    assert!(
        image_data.bytes.len().rem(4) == 0,
        "Needs to be 4 bytes per pixel"
    );
    let mut clipboard = Clipboard::new()?;
    let copy = image_data.clone();
    clipboard.set_image(image_data)?;

    log::info!(
        "Clipboard set to: W[{}],H[{}], NumBytes[{}]",
        copy.width,
        copy.height,
        copy.bytes.len()
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub enum ScreenshotTarget {
    Pixel(Pos2),
    Region(Rect),
}

pub fn crop_screenshot(
    image: &ColorImage,
    target: ScreenshotTarget,
    pixels_per_point: f32,
) -> ColorImage {
    let [width, height] = image.size;
    let to_pixel = |point: f32, limit: usize| {
        ((point * pixels_per_point).floor().max(0.0) as usize).min(limit)
    };
    let (min, max) = match target {
        ScreenshotTarget::Pixel(p) => {
            let x = to_pixel(p.x, width.saturating_sub(1));
            let y = to_pixel(p.y, height.saturating_sub(1));
            ([x, y], [(x + 1).min(width), (y + 1).min(height)])
        }
        ScreenshotTarget::Region(r) => {
            let min = [to_pixel(r.min.x, width), to_pixel(r.min.y, height)];
            let max = [
                to_pixel(r.max.x, width).max(min[0]),
                to_pixel(r.max.y, height).max(min[1]),
            ];
            (min, max)
        }
    };
    let pixel_rect = Rect::from_min_max(
        pos2(min[0] as f32, min[1] as f32),
        pos2(max[0] as f32, max[1] as f32),
    );
    image.region(&pixel_rect, None)
}

pub fn copy_status_text(result: &Result<()>, success_text: &str) -> String {
    match result {
        Ok(()) => success_text.to_string(),
        Err(e) => format!("Copy failed: {e}"),
    }
}

#[derive(Debug)]
pub struct ClipboardCopyEvent {
    pub frame_rect: Rect,
    pub frame_pixels: Option<FramePixelRead>,
}
#[derive(Debug)]
pub struct ClipboardPopup {
    pub open: bool,
    pub position: Pos2,
    pub open_timestamp: Instant,
    pub open_duration: f32,
    pub text: String,
}

impl Default for ClipboardPopup {
    fn default() -> Self {
        Self {
            open: false,
            position: Pos2::ZERO, // assuming Pos2::ZERO exists, else use Pos2::new(0.0, 0.0)
            open_timestamp: Instant::now(),
            open_duration: 2.0,
            text: "Copied to clipboard".to_string(),
        }
    }
}

impl ClipboardPopup {
    pub fn new(open: bool, position: Pos2, open_timestamp: Instant, open_duration: f32) -> Self {
        Self {
            open,
            position,
            open_timestamp,
            open_duration,
            ..Default::default()
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open(&mut self, position: Pos2, duration: f32) {
        self.open = true;
        self.position = position;
        self.open_timestamp = Instant::now();
        self.open_duration = duration;
    }

    pub fn update(&mut self) {
        let time_since = Instant::now()
            .duration_since(self.open_timestamp)
            .as_secs_f32();
        if time_since > self.open_duration {
            self.close();
        }
    }
    pub fn set_text(&mut self, new_text: &String) {
        self.text = new_text.clone();
    }
    pub fn draw(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("clipboard_popup"),
        ));

        let time_since = Instant::now()
            .duration_since(self.open_timestamp)
            .as_secs_f32();
        let alpha = popup_alpha(time_since, self.open_duration);
        let color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 200.0) as u8);

        let galley = painter.layout_no_wrap(
            self.text.clone(),
            egui::TextStyle::Heading.resolve(&ctx.style()),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8),
        );
        let rect = popup_rect(self.position, galley.size(), ctx.screen_rect());

        painter.rect_filled(rect, 4.0, color);
        painter.galley(
            rect.center() - galley.size() / 2.0,
            galley,
            egui::Color32::WHITE,
        );

        ctx.request_repaint();
    }
}

const POPUP_MIN_SIZE: Vec2 = vec2(200.0, 40.0);
const POPUP_PADDING: Vec2 = vec2(16.0, 8.0);

pub fn popup_rect(anchor: Pos2, text_size: Vec2, screen: Rect) -> Rect {
    let size = (text_size + 2.0 * POPUP_PADDING).max(POPUP_MIN_SIZE);
    let min = anchor.min(screen.max - size).max(screen.min);
    Rect::from_min_size(min, size)
}

const POPUP_OPAQUE_FRACTION: f32 = 0.6;

pub fn popup_alpha(elapsed: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return 0.0;
    }
    let fade_start = POPUP_OPAQUE_FRACTION * duration;
    (1.0 - (elapsed - fade_start) / (duration - fade_start)).clamp(0.0, 1.0)
}

pub fn copy_popup_seconds(result: &Result<()>) -> f32 {
    match result {
        Ok(()) => 1.5,
        Err(_) => 3.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ZError;
    use ecolor::Color32;
    use std::time::Duration;

    fn black_image(width: usize, height: usize) -> ColorImage {
        ColorImage::new([width, height], Color32::BLACK)
    }

    fn set_pixel(image: &mut ColorImage, x: usize, y: usize, color: Color32) {
        let width = image.width();
        image.pixels[y * width + x] = color;
    }

    fn region(min: f32, max: f32) -> ScreenshotTarget {
        ScreenshotTarget::Region(Rect::from_min_max(pos2(min, min), pos2(max, max)))
    }

    #[test]
    fn pixel_crop_is_one_pixel_at_fractional_scale() {
        let cursor_coords: Vec<f32> = (0..)
            .map(|i| i as f32 * 0.37)
            .take_while(|c| *c < 100.0)
            .collect();
        for ppp in [1.0_f32, 1.333, 1.5, 2.0] {
            let side = (100.0 * ppp).ceil() as usize;
            let image = black_image(side, side);
            for &y in &cursor_coords {
                for &x in &cursor_coords {
                    let crop = crop_screenshot(&image, ScreenshotTarget::Pixel(pos2(x, y)), ppp);
                    assert_eq!(crop.size, [1, 1], "ppp {ppp} cursor ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn pixel_crop_reads_floor_of_cursor_times_ppp() {
        let mut image = black_image(40, 40);
        set_pixel(&mut image, 13, 6, Color32::RED);
        let crop = crop_screenshot(&image, ScreenshotTarget::Pixel(pos2(10.0, 5.0)), 1.333);
        assert_eq!(crop.size, [1, 1]);
        assert_eq!(crop.pixels[0], Color32::RED);
    }

    #[test]
    fn pixel_crop_clamps_cursor_outside_image() {
        let mut image = black_image(40, 40);
        set_pixel(&mut image, 39, 39, Color32::RED);
        set_pixel(&mut image, 0, 0, Color32::BLUE);

        let past_end = crop_screenshot(&image, ScreenshotTarget::Pixel(pos2(1000.0, 1000.0)), 1.5);
        assert_eq!(past_end.size, [1, 1]);
        assert_eq!(past_end.pixels[0], Color32::RED);

        let before_start = crop_screenshot(&image, ScreenshotTarget::Pixel(pos2(-5.0, -5.0)), 1.5);
        assert_eq!(before_start.size, [1, 1]);
        assert_eq!(before_start.pixels[0], Color32::BLUE);
    }

    #[test]
    fn region_crop_is_clamped_to_image() {
        let image = black_image(50, 50);
        assert_eq!(
            crop_screenshot(&image, region(10.0, 100.0), 1.0).size,
            [40, 40]
        );
        assert_eq!(
            crop_screenshot(&image, region(5.0, 40.0), 2.0).size,
            [40, 40]
        );
    }

    #[test]
    fn region_crop_outside_image_is_empty() {
        let image = black_image(50, 50);
        let crop = crop_screenshot(&image, region(200.0, 300.0), 1.0);
        assert_eq!(crop.width() * crop.height(), 0);
    }

    #[test]
    fn copy_status_text_reports_clipboard_error() {
        let failed: Result<()> = Err(ZError::from(arboard::Error::ClipboardOccupied));
        assert_eq!(
            copy_status_text(&failed, "Copied"),
            format!("Copy failed: {}", arboard::Error::ClipboardOccupied)
        );
        assert_eq!(copy_status_text(&Ok(()), "Copied"), "Copied");
    }

    fn draw_after_warm_up(popup: &mut ClipboardPopup) -> egui::FullOutput {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| popup.draw(ctx));
        }
        ctx.run(egui::RawInput::default(), |ctx| popup.draw(ctx))
    }

    #[test]
    fn closed_popup_paints_nothing_and_requests_no_repaint() {
        let output = draw_after_warm_up(&mut ClipboardPopup::default());
        assert!(output.shapes.is_empty());
        assert_ne!(
            output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            Duration::ZERO
        );
    }

    #[test]
    fn open_popup_paints_and_requests_repaint() {
        let mut popup = ClipboardPopup::default();
        popup.open(Pos2::ZERO, 0.7);
        let output = draw_after_warm_up(&mut popup);
        assert!(!output.shapes.is_empty());
        assert_eq!(
            output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            Duration::ZERO
        );
    }

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))
    }

    #[test]
    fn popup_rect_short_text_is_minimum_size_at_anchor() {
        let rect = popup_rect(pos2(100.0, 50.0), vec2(40.0, 10.0), screen());
        assert_eq!(
            rect,
            Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 40.0))
        );
    }

    #[test]
    fn popup_rect_long_text_fits_inside() {
        let text_size = vec2(600.0, 30.0);
        let rect = popup_rect(pos2(100.0, 50.0), text_size, screen());
        assert!(rect.width() > text_size.x);
        assert!(rect.height() > text_size.y);
    }

    #[test]
    fn popup_rect_near_bottom_right_is_shifted_on_screen() {
        let rect = popup_rect(pos2(990.0, 790.0), vec2(300.0, 30.0), screen());
        assert!(screen().contains_rect(rect), "{rect:?}");
    }

    #[test]
    fn popup_rect_larger_than_screen_aligns_to_screen_min() {
        let rect = popup_rect(pos2(500.0, 500.0), vec2(2000.0, 1000.0), screen());
        assert_eq!(rect.min, screen().min);
    }

    #[test]
    fn failed_copy_popup_stays_longer() {
        let failed: Result<()> = Err(ZError::from(arboard::Error::ClipboardOccupied));
        assert_eq!(copy_popup_seconds(&failed), 3.0);
        assert_eq!(copy_popup_seconds(&Ok(())), 1.5);
        assert!(copy_popup_seconds(&failed) > copy_popup_seconds(&Ok(())));
    }

    #[test]
    fn popup_alpha_is_opaque_before_sixty_percent() {
        let duration = 1.5;
        assert_eq!(popup_alpha(0.0, duration), 1.0);
        assert_eq!(popup_alpha(0.59 * duration, duration), 1.0);
    }

    #[test]
    fn popup_alpha_fades_strictly_down_to_zero_at_duration() {
        let duration = 1.5;
        let fading: Vec<f32> = (0..=20)
            .map(|i| popup_alpha(duration * (0.6 + 0.4 * i as f32 / 20.0), duration))
            .collect();
        assert!(
            fading.windows(2).all(|pair| pair[1] < pair[0]),
            "{fading:?}"
        );
        assert_eq!(popup_alpha(duration, duration), 0.0);
        assert_eq!(popup_alpha(2.0 * duration, duration), 0.0);
    }

    #[test]
    fn popup_alpha_with_zero_duration_is_zero() {
        let alpha = popup_alpha(0.0, 0.0);
        assert!(alpha.is_finite());
        assert_eq!(alpha, 0.0);
        assert_eq!(popup_alpha(1.0, -1.0), 0.0);
    }
}
