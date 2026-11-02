use std::{ops::Rem, time::Instant};

use arboard::{Clipboard, ImageData};
use ecolor::Color32;
use eframe::egui::{self, pos2, ColorImage, Pos2, Rect};

use crate::{
    common::ColorStringCopy, error::Result, image_processing::FramePixelRead,
    ui_egui::color_picker::format_color_as,
};

pub fn write_string_to_clipboard(text: String) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(&text)?;

    log::info!("Clipboard set to: {}", text);
    Ok(())
}

pub fn write_color_to_clipboard(color: Color32, format: ColorStringCopy) -> Result<()> {
    let text = format_color_as(color.into(), format, None);
    write_string_to_clipboard(text)
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

    pub fn open(&mut self, position: Pos2) {
        self.open = true;
        self.position = position;
        self.open_timestamp = Instant::now();
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
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("clipboard_popup"),
        ));
        let rect = egui::Rect::from_min_size(self.position, egui::vec2(200.0, 40.0));

        let time_since = Instant::now()
            .duration_since(self.open_timestamp)
            .as_secs_f32();
        let alpha = (1.0 - (time_since / self.open_duration)).clamp(0.0, 1.0);
        let color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 200.0) as u8);

        painter.rect_filled(rect, 4.0, color);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &self.text,
            egui::TextStyle::Heading.resolve(&ctx.style()),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8),
        );

        ctx.request_repaint(); // keep redrawing until it fades out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ZError;

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
}
