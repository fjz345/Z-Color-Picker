use std::{fs::File, io::Write};

use arboard::{Clipboard, ImageData};
use ecolor::Color32;

use crate::{
    color_picker::{format_color_as, ColorStringCopy},
    error::Result,
    ui_common::Rgb,
};

pub fn write_string_to_clipboard(text: String) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(&text)?;

    println!("Clipboard set to: {}", text);
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
    let mut _clipboard = Clipboard::new()?;
    let copy = image_data.clone();
    // clipboard.set_image(image_data)?;

    println!(
        "Clipboard set to: W[{}],H[{}], NumBytes[{}]",
        copy.width,
        copy.height,
        copy.bytes.len()
    );
    Ok(())
}

pub fn write_pixels_to_clipboard_test_ppm(image_data: ImageData, test_vec: Vec<Rgb>) -> Result<()> {
    // let mut clipboard = Clipboard::new()?;
    let copy = image_data.clone();
    // clipboard.set_image(image_data)?;

    let mut image_ppm: String = String::new();
    image_ppm += &format!("P3\n{} {}\n255\n", image_data.width, image_data.height).to_string();
    for col in test_vec {
        write_color_ppm(&mut image_ppm, col.val);
    }

    let render_file_path = "render.ppm";
    println!("Saving to file {}...", render_file_path);

    let mut render_file = File::create(render_file_path)?;
    render_file.write_all(image_ppm.as_bytes()).unwrap();

    println!("render.ppm written");
    println!(
        "Clipboard set to: W[{}],H[{}], NumBytes[{}]",
        copy.width,
        copy.height,
        copy.bytes.len()
    );

    Ok(())
}
