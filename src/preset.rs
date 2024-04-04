use std::{
    fs::{self, remove_file, DirEntry, File, OpenOptions},
    io::{Read, Write},
};

use serde::{Deserialize, Serialize};

use crate::{
    color_picker::SplineMode, fs::write_string_to_file, hsv_key_value::HsvKeyValue,
    CONTROL_POINT_TYPE,
};

const PRESETS_PATH: &str = "./presets";

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
    pub control_points: Vec<CONTROL_POINT_TYPE>,
}

pub fn load_presets(presets: &mut Vec<Preset>) {
    let paths = fs::read_dir(PRESETS_PATH).unwrap();

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

                let maybe_loaded_preset = load_preset_from_disk(dir);

                presets.push(maybe_loaded_preset.expect("Failed to load preset from file"));
            }
            Err(_) => panic!("Path is invalid???"),
        }
    }
    if DEBUG_PRINT {
        println!("=====================");
    }
}

pub fn load_preset_from_disk(dir_entry: DirEntry) -> Option<Preset> {
    let file = File::open(dir_entry.path());

    match file {
        Ok(mut f) => {
            let mut buf: String = String::new();
            let read_ok = f.read_to_string(&mut buf);
            match read_ok {
                Ok(_) => {}
                Err(e) => println!("{}", e.kind()),
            }

            let preset_data = serde_json::from_str(&buf).expect("JSON format error");
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

            return Some(preset_from_file);
        }
        Err(_) => panic!("Failed To load preset file"),
    }

    None
}

pub fn save_preset_to_disk(preset: &Preset) {
    let preset_data_string = serde_json::to_string_pretty(&preset.data);
    let file_path = &get_preset_save_path(&preset);

    match preset_data_string {
        Ok(string) => {
            write_string_to_file(&string, file_path)
                .expect("Something went wrong with writing string to file");
            println!("SAVED TO PATH {}", file_path);
        }
        Err(_) => panic!("Failed To stringify preset"),
    }
}

pub fn delete_preset_from_disk(file_path: &str) {
    remove_file(file_path).expect("Failed????");
}

pub fn get_preset_save_path(preset: &Preset) -> String {
    format!("{PRESETS_PATH}\\{}.json", preset.name)
}
