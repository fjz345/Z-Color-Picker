use std::ops::Rem;

use eframe::{
    egui::{self, Rect},
    glow::{self, HasContext},
};

#[repr(C)]
#[derive(Clone, Debug)]
pub struct Rgb {
    pub val: (u8, u8, u8),
}

#[derive(Debug)]
pub struct FramePixelRead {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Rgb>,
}

pub fn u8_to_u8u8u8(buf: &[u8]) -> Vec<Rgb> {
    assert!(buf.len().rem(3) == 0);
    let mut ret: Vec<Rgb> = Vec::with_capacity(buf.len() / 3);
    for i in (0..buf.len()).step_by(3) {
        let val = Rgb {
            val: (buf[i], buf[i + 1], buf[i + 2]),
        };
        ret.push(val);
    }

    ret
}

pub fn u8u8u8_to_u8(buf: &[Rgb]) -> Vec<u8> {
    let mut ret: Vec<u8> = Vec::new();
    for i in 0..buf.len() {
        ret.push(buf[i].val.0);
        ret.push(buf[i].val.1);
        ret.push(buf[i].val.2);
    }

    ret
}

pub fn u8u8u8u8_to_u8(buf: &[(u8, u8, u8, u8)]) -> Vec<u8> {
    let mut ret: Vec<u8> = Vec::new();
    for i in 0..buf.len() {
        ret.push(buf[i].0);
        ret.push(buf[i].1);
        ret.push(buf[i].2);
        ret.push(buf[i].3);
    }

    ret
}

pub fn u8u8u8_to_u8u8u8u8(buf: &[Rgb]) -> Vec<(u8, u8, u8, u8)> {
    let mut ret: Vec<(u8, u8, u8, u8)> = Vec::new();
    for i in 0..buf.len() {
        ret.push((buf[i].val.0, buf[i].val.1, buf[i].val.2, 255));
    }

    ret
}

/// Vertically flips the image pixels in memory
pub fn flip_v(image: FramePixelRead, bytes_per_pixel: usize) -> FramePixelRead {
    let w = image.width;
    let h = image.height;

    let mut bytes = u8u8u8_to_u8(&image.data);

    let rowsize = w * bytes_per_pixel; // each pixel is 4 bytes
    let mut tmp_a = vec![0; rowsize];
    // I believe this could be done safely with `as_chunks_mut`, but that's not stable yet
    for a_row_id in 0..(h / 2) {
        let b_row_id = h - a_row_id - 1;

        // swap rows `first_id` and `second_id`
        let a_byte_start = a_row_id * rowsize;
        let a_byte_end = a_byte_start + rowsize;
        let b_byte_start = b_row_id * rowsize;
        let b_byte_end = b_byte_start + rowsize;
        tmp_a.copy_from_slice(&bytes[a_byte_start..a_byte_end]);
        bytes.copy_within(b_byte_start..b_byte_end, a_byte_start);
        bytes[b_byte_start..b_byte_end].copy_from_slice(&tmp_a);
    }

    FramePixelRead {
        width: image.width,
        height: image.height,
        data: u8_to_u8u8u8(&bytes),
    }
}

pub fn gl_read_rect_pixels(
    rect: Rect,
    ctx: &egui::Context,
    frame: &eframe::Frame,
) -> Option<FramePixelRead> {
    // Convert egui points (rect) to native pixels coordinates
    let native_pixels_per_point = ctx.native_pixels_per_point().unwrap_or(1.0);

    // Screen size in native pixels
    let screen_size = ctx.screen_rect().size();
    let screen_width_px = (screen_size.x * native_pixels_per_point) as i32;
    let screen_height_px = (screen_size.y * native_pixels_per_point) as i32;

    // Convert rect coordinates from egui points to pixels and flip Y axis
    // OpenGL's origin (0,0) is bottom-left; egui's origin is top-left
    let x = (rect.min.x * native_pixels_per_point) as i32;
    let y_egui = (rect.min.y * native_pixels_per_point) as i32;
    let width = ((rect.width()) * native_pixels_per_point) as i32;
    let height = ((rect.height()) * native_pixels_per_point) as i32;

    // ensure 4*N
    let width_4 = (width + 3) & (-4);
    let height_4 = (height + 3) & (-4);

    // Flip Y because OpenGL origin is bottom-left, egui is top-left
    let y = screen_height_px - y_egui - height_4;

    if width_4 <= 0
        || height_4 <= 0
        || x < 0
        || y < 0
        || x + width_4 > screen_width_px
        || y + height_4 > screen_height_px
    {
        eprintln!("Rect out of screen bounds or empty");
        return None;
    }

    let mut pixels = unsafe {
        let buf_size = (3 * width_4 * height_4) as usize;
        let mut buf: Vec<u8> = vec![0u8; buf_size];
        let pixels = glow::PixelPackData::Slice(Some(&mut buf[..]));
        frame.gl().unwrap().read_pixels(
            x,
            y,
            width_4 as i32,
            height_4 as i32,
            glow::RGB,
            glow::UNSIGNED_BYTE,
            pixels,
        );

        u8_to_u8u8u8(&buf[0..buf_size])
    };

    if width == 1 && height == 1 {
        pixels = vec![pixels.first().unwrap().clone()];
    }

    Some(FramePixelRead {
        data: pixels,
        width: width as usize,
        height: height as usize,
    })
}
