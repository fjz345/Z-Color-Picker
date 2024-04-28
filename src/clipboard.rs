use arboard::{Clipboard, ImageData};
use ecolor::Color32;

use crate::{
    color_picker::{format_color_as, ColorStringCopy},
    error::{Result, ZError},
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

pub fn write_pixels_to_clipboard(image_data: ImageData) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    let copy = image_data.clone();
    clipboard.set_image(image_data)?;

    println!(
        "Clipboard set to: W[{}],H[{}], NumBytes[{}]",
        copy.width,
        copy.height,
        copy.bytes.len()
    );
    Ok(())
}
