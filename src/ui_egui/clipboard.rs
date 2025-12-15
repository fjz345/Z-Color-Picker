use std::{fs::File, io::Write, ops::Rem, time::Instant};

use arboard::{Clipboard, ImageData};
use ecolor::Color32;
use eframe::egui::{self, Pos2, Rect};

use crate::{
    common::ColorStringCopy,
    error::Result,
    image_processing::{FramePixelRead, Rgb},
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

fn write_color_ppm(ppm_string: &mut String, color: (u8, u8, u8)) {
    let ir = color.0;
    let ig = color.1;
    let ib = color.2;

    *ppm_string += &ir.to_string();
    *ppm_string += &' '.to_string();
    *ppm_string += &ig.to_string();
    *ppm_string += &' '.to_string();
    *ppm_string += &ib.to_string();
    *ppm_string += &'\n'.to_string();
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

pub fn write_pixels_to_test_ppm(image_data: &ImageData, test_vec: Vec<Rgb>) -> Result<()> {
    let copy = image_data.clone();

    let mut image_ppm: String = String::new();
    image_ppm += &format!("P3\n{} {}\n255\n", copy.width, copy.height).to_string();
    for col in test_vec {
        write_color_ppm(&mut image_ppm, col.val);
    }

    let render_file_path = "render.ppm";
    log::info!("Saving to file {}...", render_file_path);

    let mut render_file = File::create(render_file_path)?;
    render_file.write_all(image_ppm.as_bytes()).unwrap();

    log::info!("render.ppm written");

    Ok(())
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
