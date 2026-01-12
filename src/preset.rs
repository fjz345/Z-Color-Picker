use std::{
    fs::{self, remove_file, DirEntry},
    path::{Path, PathBuf},
};

use crate::{
    common::SplineMode,
    datatypes::control_point::ControlPoint,
    error::{Result, ZError},
    preset,
};
use eframe::egui::load;
use serde::{Deserialize, Serialize};
use splines::Spline;

use crate::fs::write_string_to_file;

pub const PRESETS_FOLDER_NAME: &str = "presets";
pub const SAVED_FOLDER_NAME: &str = "saved";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetEntity {
    pub name: String,
    pub data: PresetData,
}

impl PresetEntity {
    pub fn new(name: &str, data: PresetData) -> Self {
        Self {
            name: name.to_string(),
            data,
        }
    }

    pub fn apply(&self, control_points: &mut Vec<ControlPoint>, spline_mode: &mut SplineMode) {
        self.data.clone().apply(control_points, spline_mode);
    }

    pub fn into(self) -> (Vec<ControlPoint>, SplineMode) {
        (self.data.control_points, self.data.spline_mode)
    }

    pub fn make_preset_data(
        control_points: &Vec<ControlPoint>,
        spline_mode: &SplineMode,
    ) -> PresetData {
        PresetData {
            spline_mode: *spline_mode,
            control_points: control_points.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetData {
    pub spline_mode: SplineMode,
    pub control_points: Vec<ControlPoint>,
}

impl PresetData {
    pub fn apply(self, control_points: &mut Vec<ControlPoint>, spline_mode: &mut SplineMode) {
        *control_points = self.control_points;
        *spline_mode = self.spline_mode;
    }
}

impl From<(Vec<ControlPoint>, SplineMode)> for PresetData {
    fn from(value: (Vec<ControlPoint>, SplineMode)) -> Self {
        Self {
            control_points: value.0,
            spline_mode: value.1,
        }
    }
}

pub fn get_presets_path() -> PathBuf {
    let cur_dir = std::env::current_dir().unwrap();
    cur_dir.join(PRESETS_FOLDER_NAME)
}

pub fn load_presets(path: &Path) -> Result<Vec<PresetEntity>> {
    let paths = fs::read_dir(path)?;

    let mut presets = Vec::new();
    const DEBUG_PRINT: bool = true;
    if DEBUG_PRINT {
        log::info!("PRINTING FOUND PRESETS ========");
    }
    for path in paths {
        match path {
            Ok(dir) => {
                if DEBUG_PRINT {
                    log::info!("Name: {}", dir.path().display());
                }

                let maybe_loaded_preset = load_preset_from_disk(&dir);
                match maybe_loaded_preset {
                    Ok(p) => presets.push(p),
                    Err(e) => {
                        log::info!(
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
        log::info!("=====================");
    }

    if presets.len() <= 0 {
        return Err(ZError::Message(
            "Did not manage to load any presets".to_string(),
        ));
    }
    Ok(presets)
}

pub fn load_preset_from_disk(dir_entry: &DirEntry) -> Result<PresetEntity> {
    let string = std::fs::read_to_string(dir_entry.path())?;
    let preset_data = serde_json::from_str(&string)?;
    let preset_from_file: PresetEntity = PresetEntity::new(
        dir_entry
            .file_name()
            .to_str()
            .unwrap()
            .to_string()
            .strip_suffix(".json")
            .unwrap(),
        preset_data,
    );

    Ok(preset_from_file)
}

pub fn save_preset_to_disk(preset: &PresetEntity) -> Result<()> {
    let preset_data_string = serde_json::to_string_pretty(&preset.data)?;
    let file_path = &get_preset_save_path(&preset);

    write_string_to_file(&preset_data_string, file_path)?;
    log::debug!("SAVED TO PATH {}", file_path);

    Ok(())
}

pub fn delete_preset_from_disk(preset: &PresetEntity) -> Result<()> {
    let file_path = &get_preset_save_path(&preset);

    remove_file(file_path)?;
    log::info!("DELETED {}", file_path);
    Ok(())
}

pub fn get_preset_save_path(preset: &PresetEntity) -> String {
    let curr_dir = std::env::current_dir().unwrap();
    let presets_path = curr_dir.join(PRESETS_FOLDER_NAME);
    let file_path = presets_path.join(format!("{}.json", preset.name));
    file_path.to_path_buf().to_str().unwrap().to_string()
}

pub fn save_all_presets_to_disk(presets: &[PresetEntity]) -> Result<()> {
    for preset in presets.iter() {
        save_preset_to_disk(preset)?;
    }
    Ok(())
}

pub fn delete_presets_from_disk(presets: &[PresetEntity]) -> Result<()> {
    for preset in presets.iter() {
        delete_preset_from_disk(preset)?;
    }
    Ok(())
}

pub fn delete_all_presets_from_disk() -> Result<()> {
    let presets: Vec<PresetEntity> = load_presets(get_presets_path().as_path())?;
    for preset in presets.iter() {
        delete_preset_from_disk(preset)?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PresetHandler {
    pub presets: Vec<PresetEntity>,
    pub preset_selected_index: Option<usize>,
    pub auto_save_presets: bool,
}

impl Default for PresetHandler {
    fn default() -> Self {
        let presets_result = load_presets(&get_presets_path());
        if let Err(e) = &presets_result {
            log::info!("{e}");
        }
        let presets = presets_result.unwrap_or(Vec::new());
        Self {
            presets: presets,
            preset_selected_index: None,
            auto_save_presets: false,
        }
    }
}

impl PresetHandler {
    pub fn presets(&self) -> &Vec<PresetEntity> {
        &self.presets
    }
    pub fn presets_mut(&mut self) -> &mut Vec<PresetEntity> {
        &mut self.presets
    }

    pub fn apply_selected_preset(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
    ) {
        if let Some(preset) = self.presets.get(self.preset_selected_index.unwrap()) {
            let (preset_control_points, preset_spline_mode) = preset.clone().into();
            *control_points = preset_control_points;
            *spline_mode = preset_spline_mode;
            log::info!("Preset Applied!");
        } else {
            log::info!("No preset selected");
        }
    }

    pub fn init_presets(&mut self) -> Result<()> {
        let loaded_presets = load_presets(&get_presets_path())?;
        self.presets = loaded_presets;
        Ok(())
    }

    pub fn save_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.preset_selected_index {
            let preset = &mut self.presets[s];
            save_preset_to_disk(&preset.clone())?;

            return Ok(());
        }

        Err(ZError::Message(
            "Preset Save failed, No preset selected".to_string(),
        ))
    }

    pub fn create_preset(
        &mut self,
        name: &String,
        control_points: &Vec<ControlPoint>,
        spline_mode: &SplineMode,
    ) -> Result<()> {
        for i in self.presets.iter() {
            if &i.name == name {
                return Err(ZError::Message(
                    "Preset already exists with that name".to_string(),
                ));
            }
        }

        let preset = PresetEntity::new(name, (control_points.clone(), *spline_mode).into());
        let index = self.presets.len();
        self.presets.push(preset);

        self.preset_selected_index = Some(index);
        self.save_selected_preset()?;

        Ok(())
    }

    pub fn delete_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.preset_selected_index {
            let preset_to_remove = self.presets.remove(s);
            delete_preset_from_disk(&preset_to_remove)?;
            self.preset_selected_index = None;

            return Ok(());
        }

        Err(ZError::Message(
            "Selected Preset Delete failed, No preset selected".to_string(),
        ))
    }
}
