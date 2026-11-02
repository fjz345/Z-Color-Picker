use crate::error::Result;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

pub fn write_string_to_file(in_string: &String, file_path: &Path) -> Result<()> {
    let mut file = File::create(file_path)?;
    file.write_all(in_string.as_bytes())?;

    Ok(())
}
