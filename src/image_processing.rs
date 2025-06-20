use std::ops::Rem;

use crate::ui_common::FramePixelRead;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct Rgb {
    pub val: (u8, u8, u8),
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
