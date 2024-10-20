use std::{
    fs::{self, remove_file, DirEntry},
    path::Path,
};

use crate::{
    control_point::ControlPoint,
    error::{Result, ZError},
};
use serde::{Deserialize, Serialize};

use crate::{color_picker::SplineMode, fs::write_string_to_file};

pub const PRESETS_FOLDER_NAME: &str = "presets";

#[derive(Clone, Debug)]
pub struct Preset {
    pub name: String,
    pub data: PresetData,
}

impl Preset {
    pub fn new(name: &str, data: PresetData) -> Self {
        Self {
            name: name.to_string(),
            data,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetData {
    pub spline_mode: SplineMode,
    pub control_points: Vec<ControlPoint>,
}

pub fn load_presets(path: &Path, presets: &mut Vec<Preset>) -> Result<()> {
    presets.clear();
    let paths = fs::read_dir(path)?;

    const DEBUG_PRINT: bool = true;
    if DEBUG_PRINT {
        println!("PRINTING FOUND PRESETS ========");
    }
    for path in paths {
        match path {
            Ok(dir) => {
                if DEBUG_PRINT {
                    println!("Name: {}", dir.path().display());
                }

                let maybe_loaded_preset = load_preset_from_disk(&dir);
                match maybe_loaded_preset {
                    Ok(p) => presets.push(p),
                    Err(e) => {
                        println!(
                            "Error: {:?}, Failed to load preset {:?} from file, maybe old version?",
                            e,
                            dir.file_name()
                        );
                    }
                }
            }
            Err(_) => panic!("Path is invalid???"),
        }
    }
    if DEBUG_PRINT {
        println!("=====================");
    }

    if presets.len() <= 0 {
        return Err(ZError::Message(
            "Did not manage to load any presets".to_string(),
        ));
    }
    Ok(())
}

pub fn load_preset_from_disk(dir_entry: &DirEntry) -> Result<Preset> {
    let string = std::fs::read_to_string(dir_entry.path())?;
    let preset_data = serde_json::from_str(&string)?;
    let preset_from_file: Preset = Preset::new(
        dir_entry
            .file_name()
            .to_str()
            .unwrap()
            .to_string()
            .strip_suffix(".json")
            .unwrap(),
        preset_data,
    );
    println!("{:?}", preset_from_file);

    Ok(preset_from_file)
}

pub fn save_preset_to_disk(preset: &Preset) -> Result<()> {
    let preset_data_string = serde_json::to_string_pretty(&preset.data)?;
    let file_path = &get_preset_save_path(&preset);

    write_string_to_file(&preset_data_string, file_path)?;
    println!("SAVED TO PATH {}", file_path);

    Ok(())
}

pub fn delete_preset_from_disk(file_path: &str) -> Result<()> {
    remove_file(file_path)?;
    Ok(())
}

pub fn get_preset_save_path(preset: &Preset) -> String {
    let curr_dir = std::env::current_dir().unwrap();
    let presets_path = curr_dir.join(PRESETS_FOLDER_NAME);
    let file_path = presets_path.join(format!("{}.json", preset.name));
    file_path.to_path_buf().to_str().unwrap().to_string()
}
