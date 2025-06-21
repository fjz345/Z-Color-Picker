use std::{fs::File, io::Write, ops::Rem, time::Instant};

use arboard::{Clipboard, ImageData};
use ecolor::Color32;
use eframe::egui::{InnerResponse, Pos2, Rect, Ui, Window};

use crate::{
    color_picker::format_color_as, common::ColorStringCopy, error::Result, image_processing::Rgb,
    ui_common::FramePixelRead,
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
}

impl Default for ClipboardPopup {
    fn default() -> Self {
        Self {
            open: false,
            position: Pos2::ZERO, // assuming Pos2::ZERO exists, else use Pos2::new(0.0, 0.0)
            open_timestamp: Instant::now(),
            open_duration: 0.0,
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

    pub fn draw_ui(&mut self, ui: &mut Ui) -> Option<InnerResponse<Option<()>>> {
        let time_since_open = Instant::now()
            .duration_since(self.open_timestamp)
            .as_secs_f32();
        let alpha = (1.0 - (time_since_open / self.open_duration)).clamp(0.0, 1.0);
        self.draw_ui_clipboard_copy(ui, alpha)
    }

    fn draw_ui_clipboard_copy(
        &mut self,
        ui: &mut Ui,
        opacity: f32,
    ) -> Option<InnerResponse<Option<()>>> {
        let prev_visuals = ui.visuals_mut().clone();

        let alpha_u8 = (opacity * 255.0) as u8;
        let mut color_bg = prev_visuals.window_fill;
        color_bg[3] = alpha_u8;
        let mut color_text = prev_visuals.text_color();
        color_text[3] = alpha_u8;
        ui.visuals_mut().window_fill = color_bg;
        ui.visuals_mut().window_stroke.color = color_bg;
        ui.visuals_mut().window_stroke.width = 0.0;
        ui.visuals_mut().widgets.active.fg_stroke.color = color_text;
        // ui.visuals_mut().window_shadow.extrusion = 0.0;
        ui.ctx().set_visuals(ui.visuals().clone());

        let mut should_open: bool = self.open;
        let response = Window::new("")
            .fixed_pos(&[self.position.x, self.position.y])
            .resizable(false)
            .title_bar(false)
            .open(&mut should_open)
            .auto_sized()
            .show(ui.ctx(), |ui| {
                ui.label("Copied to clipboard");

                ui.ctx().request_repaint();
            });
        self.open = should_open;

        ui.ctx().set_visuals(prev_visuals);

        response
    }
}
