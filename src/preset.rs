use std::{
    ffi::OsStr,
    fmt::Display,
    fs::{self, remove_file},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::{
    common::SplineMode,
    datatypes::control_point::ControlPoint,
    error::{Result, ZError},
    ui_egui::app::AppDataCtx,
};
use serde::{Deserialize, Serialize};

use crate::fs::write_string_to_file;

pub const PRESETS_FOLDER_NAME: &str = "presets";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetEntity {
    pub name: String,
    pub data: AppDataCtx,
}

impl PresetEntity {
    pub fn new(name: &str, data: AppDataCtx) -> Self {
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
    ) -> AppDataCtx {
        AppDataCtx {
            spline_mode: *spline_mode,
            control_points: control_points.clone(),
        }
    }
}

impl AppDataCtx {
    pub fn apply(self, control_points: &mut Vec<ControlPoint>, spline_mode: &mut SplineMode) {
        *control_points = self.control_points;
        *spline_mode = self.spline_mode;
    }
}

impl From<(Vec<ControlPoint>, SplineMode)> for AppDataCtx {
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

pub fn load_presets(dir: &Path) -> Vec<PresetEntity> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            log::warn!("Could not read presets folder {}: {e}", dir.display());
            return Vec::new();
        }
    };

    let mut presets = Vec::new();
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(e) => {
                log::warn!("Skipping unreadable entry in {}: {e}", dir.display());
                continue;
            }
        };
        match load_preset_from_disk(&path) {
            Ok(preset) => {
                log::info!("Loaded preset {}", path.display());
                presets.push(preset);
            }
            Err(e) => log::warn!("Skipping {}: {e}", path.display()),
        }
    }
    presets
}

fn load_preset_from_disk(path: &Path) -> Result<PresetEntity> {
    if path.extension() != Some(OsStr::new("json")) {
        return Err(ZError::Message("not a .json file".to_string()));
    }
    let name = path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ZError::Message("file name is not valid UTF-8".to_string()))?;
    let string = fs::read_to_string(path)?;
    let preset_data = serde_json::from_str(&string)?;
    Ok(PresetEntity::new(name, preset_data))
}

pub fn save_preset_to_disk(dir: &Path, preset: &PresetEntity) -> Result<()> {
    let preset_data_string = serde_json::to_string_pretty(&preset.data)?;
    fs::create_dir_all(dir)?;
    let file_path = &preset_file_path(dir, &preset.name);

    write_string_to_file(&preset_data_string, file_path)?;
    log::debug!("SAVED TO PATH {}", file_path.display());

    Ok(())
}

pub fn delete_preset_from_disk(dir: &Path, preset: &PresetEntity) -> Result<()> {
    let file_path = &preset_file_path(dir, &preset.name);

    remove_file(file_path)?;
    log::info!("DELETED {}", file_path.display());
    Ok(())
}

fn preset_file_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetNameError {
    Empty,
    InvalidChar(char),
    TrailingDotOrSpace,
    Reserved,
    TooLong,
    Duplicate,
}

impl Display for PresetNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresetNameError::Empty => write!(f, "Name is empty"),
            PresetNameError::InvalidChar(c) => write!(f, "Name contains invalid character {c:?}"),
            PresetNameError::TrailingDotOrSpace => write!(f, "Name ends with '.' or space"),
            PresetNameError::Reserved => write!(f, "Name is reserved by Windows"),
            PresetNameError::TooLong => {
                write!(f, "Name is longer than {MAX_PRESET_NAME_CHARS} characters")
            }
            PresetNameError::Duplicate => write!(f, "A preset with that name already exists"),
        }
    }
}

impl From<PresetNameError> for ZError {
    fn from(err: PresetNameError) -> ZError {
        ZError::Message(err.to_string())
    }
}

const MAX_PRESET_NAME_CHARS: usize = 64;
const FORBIDDEN_NAME_CHARS: &str = "<>:\"/\\|?*";
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn validate_preset_name<'a>(
    name: &str,
    existing_names: impl IntoIterator<Item = &'a str>,
) -> std::result::Result<(), PresetNameError> {
    if name.trim().is_empty() {
        return Err(PresetNameError::Empty);
    }
    if let Some(c) = name
        .chars()
        .find(|&c| FORBIDDEN_NAME_CHARS.contains(c) || c.is_control())
    {
        return Err(PresetNameError::InvalidChar(c));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(PresetNameError::TrailingDotOrSpace);
    }
    let device_name = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED_DEVICE_NAMES.contains(&device_name.to_uppercase().as_str()) {
        return Err(PresetNameError::Reserved);
    }
    if name.chars().count() > MAX_PRESET_NAME_CHARS {
        return Err(PresetNameError::TooLong);
    }
    let lowercase_name = name.to_lowercase();
    if existing_names
        .into_iter()
        .any(|existing| existing.to_lowercase() == lowercase_name)
    {
        return Err(PresetNameError::Duplicate);
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresetHandler {
    #[serde(skip)]
    pub presets: Vec<PresetEntity>,
    #[serde(skip)]
    pub preset_selected_index: Option<usize>,
    pub auto_save_presets: bool,
    #[serde(skip, default = "get_presets_path")]
    presets_dir: PathBuf,
}

impl Default for PresetHandler {
    fn default() -> Self {
        Self::new(get_presets_path())
    }
}

impl PresetHandler {
    pub fn new(presets_dir: PathBuf) -> Self {
        Self {
            presets: Vec::new(),
            preset_selected_index: None,
            auto_save_presets: false,
            presets_dir,
        }
    }

    pub fn check_name(
        &self,
        name: &str,
        renaming: Option<usize>,
    ) -> std::result::Result<(), PresetNameError> {
        let other_names = self
            .presets
            .iter()
            .enumerate()
            .filter(|&(i, _)| Some(i) != renaming)
            .map(|(_, preset)| preset.name.as_str());
        validate_preset_name(name, other_names)
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

    pub fn init_presets(&mut self) {
        self.presets = load_presets(&self.presets_dir);
        if self
            .preset_selected_index
            .is_some_and(|s| s >= self.presets.len())
        {
            self.preset_selected_index = None;
        }
    }

    pub fn save_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.preset_selected_index {
            let preset = &mut self.presets[s];
            save_preset_to_disk(&self.presets_dir, &preset.clone())?;

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
        self.check_name(name, None)?;

        let preset = PresetEntity::new(name, (control_points.clone(), *spline_mode).into());
        save_preset_to_disk(&self.presets_dir, &preset)?;
        let index = self.presets.len();
        self.presets.push(preset);

        self.preset_selected_index = Some(index);

        Ok(())
    }

    pub fn delete_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.preset_selected_index {
            delete_preset_from_disk(&self.presets_dir, &self.presets[s])?;
            self.presets.remove(s);
            self.preset_selected_index = None;

            return Ok(());
        }

        Err(ZError::Message(
            "Selected Preset Delete failed, No preset selected".to_string(),
        ))
    }

    pub fn rename_preset(&mut self, index: usize, new_name: &str) -> Result<()> {
        self.check_name(new_name, Some(index))?;
        fs::rename(
            preset_file_path(&self.presets_dir, &self.presets[index].name),
            preset_file_path(&self.presets_dir, new_name),
        )?;
        self.presets[index].name = new_name.to_string();
        save_preset_to_disk(&self.presets_dir, &self.presets[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_handler() -> (TempDir, PresetHandler) {
        let tmp = TempDir::new().unwrap();
        let handler = PresetHandler::new(tmp.path().join(PRESETS_FOLDER_NAME));
        (tmp, handler)
    }

    fn data(spline_mode: SplineMode) -> AppDataCtx {
        AppDataCtx {
            spline_mode,
            control_points: Vec::new(),
        }
    }

    fn create(handler: &mut PresetHandler, name: &str, spline_mode: SplineMode) -> Result<()> {
        handler.create_preset(&name.to_string(), &Vec::new(), &spline_mode)
    }

    fn names(handler: &PresetHandler) -> Vec<&str> {
        handler.presets.iter().map(|p| p.name.as_str()).collect()
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn validate(name: &str) -> std::result::Result<(), PresetNameError> {
        validate_preset_name(name, [])
    }

    #[test]
    fn name_rejects_empty_and_whitespace_only() {
        assert_eq!(validate(""), Err(PresetNameError::Empty));
        assert_eq!(validate("   "), Err(PresetNameError::Empty));
    }

    #[test]
    fn name_rejects_forbidden_characters() {
        for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
            assert_eq!(
                validate(&format!("a{c}b")),
                Err(PresetNameError::InvalidChar(c))
            );
        }
    }

    #[test]
    fn name_rejects_control_characters() {
        for name in ["a\tb", "a\u{1}b", "a\u{7f}b"] {
            assert!(
                matches!(validate(name), Err(PresetNameError::InvalidChar(_))),
                "{name:?}"
            );
        }
    }

    #[test]
    fn name_rejects_trailing_dot_or_space() {
        assert_eq!(validate("abc."), Err(PresetNameError::TrailingDotOrSpace));
        assert_eq!(validate("abc "), Err(PresetNameError::TrailingDotOrSpace));
        assert_eq!(validate("a.b"), Ok(()));
    }

    #[test]
    fn name_rejects_reserved_device_names() {
        for name in [
            "CON", "prn", "Aux", "nul", "COM1", "com9", "LPT1", "lpt9", "CON.foo", "con.txt",
        ] {
            assert_eq!(validate(name), Err(PresetNameError::Reserved), "{name}");
        }
        for name in ["COM0", "COM10", "CONSOLE", "Icon"] {
            assert_eq!(validate(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn name_rejects_more_than_64_chars() {
        assert_eq!(validate(&"a".repeat(65)), Err(PresetNameError::TooLong));
        assert_eq!(validate(&"a".repeat(64)), Ok(()));
        assert_eq!(validate(&"\u{e9}".repeat(64)), Ok(()));
    }

    #[test]
    fn name_rejects_existing_name_ignoring_case() {
        assert_eq!(
            validate_preset_name("pink", ["Pink"]),
            Err(PresetNameError::Duplicate)
        );
        assert_eq!(validate_preset_name("Pink2", ["Pink"]), Ok(()));
    }

    #[test]
    fn name_accepts_ordinary_names() {
        for name in [
            "Pink 2",
            "my-preset_v1 (warm)",
            "F\u{e4}rger \u{6f22}\u{5b57}",
        ] {
            assert_eq!(validate(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn check_name_ignores_the_preset_being_renamed() {
        let (_tmp, mut handler) = temp_handler();
        handler.presets = vec![
            PresetEntity::new("Pink", data(SplineMode::Linear)),
            PresetEntity::new("Cold", data(SplineMode::Bezier)),
        ];
        assert_eq!(handler.check_name("pink", Some(0)), Ok(()));
        assert_eq!(
            handler.check_name("pink", None),
            Err(PresetNameError::Duplicate)
        );
        assert_eq!(
            handler.check_name("cold", Some(0)),
            Err(PresetNameError::Duplicate)
        );
    }

    #[test]
    fn delete_selected_preset_keeps_entry_when_disk_delete_fails() {
        let (_tmp, mut handler) = temp_handler();
        handler.presets = vec![PresetEntity::new("A", data(SplineMode::Linear))];
        handler.preset_selected_index = Some(0);
        assert!(handler.delete_selected_preset().is_err());
        assert_eq!(handler.presets.len(), 1);
        assert_eq!(handler.preset_selected_index, Some(0));
    }

    #[test]
    fn delete_selected_preset_removes_file_then_entry() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "A", SplineMode::Linear).unwrap();
        handler.delete_selected_preset().unwrap();
        assert!(file_names(&handler.presets_dir).is_empty());
        assert!(handler.presets.is_empty());
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn create_preset_with_path_separator_adds_no_ghost_entry() {
        let (_tmp, mut handler) = temp_handler();
        assert!(create(&mut handler, "a/b", SplineMode::Linear).is_err());
        assert!(handler.presets.is_empty());
        assert!(!handler.presets_dir.exists() || file_names(&handler.presets_dir).is_empty());
    }

    #[test]
    fn create_preset_cannot_escape_presets_dir() {
        let (tmp, mut handler) = temp_handler();
        assert!(create(&mut handler, "..\\x", SplineMode::Linear).is_err());
        assert!(!tmp.path().join("x.json").exists());
    }

    #[test]
    fn create_preset_rejects_existing_name_ignoring_case() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "Pink", SplineMode::Linear).unwrap();
        let pink_path = handler.presets_dir.join("Pink.json");
        let pink_content = fs::read_to_string(&pink_path).unwrap();
        assert!(create(&mut handler, "pink", SplineMode::Bezier).is_err());
        assert_eq!(handler.presets.len(), 1);
        assert_eq!(fs::read_to_string(&pink_path).unwrap(), pink_content);
    }

    #[test]
    fn create_preset_write_failure_adds_no_entry() {
        let tmp = TempDir::new().unwrap();
        let regular_file = tmp.path().join(PRESETS_FOLDER_NAME);
        fs::write(&regular_file, "").unwrap();
        let mut handler = PresetHandler::new(regular_file);
        assert!(create(&mut handler, "Valid", SplineMode::Linear).is_err());
        assert!(handler.presets.is_empty());
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn rename_preset_moves_file_and_entry() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "Old", SplineMode::Linear).unwrap();
        handler.rename_preset(0, "New").unwrap();
        assert_eq!(file_names(&handler.presets_dir), ["New.json"]);
        assert_eq!(names(&handler), ["New"]);
    }

    #[test]
    fn rename_preset_allows_case_only_change() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "pink", SplineMode::Linear).unwrap();
        handler.rename_preset(0, "Pink").unwrap();
        assert_eq!(file_names(&handler.presets_dir), ["Pink.json"]);
        assert_eq!(names(&handler), ["Pink"]);
    }

    #[test]
    fn rename_preset_failure_keeps_name() {
        let (_tmp, mut handler) = temp_handler();
        handler.presets = vec![PresetEntity::new("Ghost", data(SplineMode::Linear))];
        handler.preset_selected_index = Some(0);
        assert!(handler.rename_preset(0, "New").is_err());
        assert_eq!(names(&handler), ["Ghost"]);
        assert!(!handler.presets_dir.join("New.json").exists());
    }

    #[test]
    fn rename_preset_rejects_other_preset_name_ignoring_case() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "Pink", SplineMode::Linear).unwrap();
        create(&mut handler, "Cold", SplineMode::Bezier).unwrap();
        let pink_path = handler.presets_dir.join("Pink.json");
        let pink_content = fs::read_to_string(&pink_path).unwrap();
        assert!(handler.rename_preset(1, "pink").is_err());
        assert_eq!(fs::read_to_string(&pink_path).unwrap(), pink_content);
        assert_eq!(file_names(&handler.presets_dir), ["Cold.json", "Pink.json"]);
        assert_eq!(names(&handler), ["Pink", "Cold"]);
    }

    #[test]
    fn init_presets_skips_non_json_malformed_and_directories() {
        let (_tmp, mut handler) = temp_handler();
        let dir = handler.presets_dir.clone();
        fs::create_dir_all(dir.join("Sub.json")).unwrap();
        let preset_json = serde_json::to_string_pretty(&data(SplineMode::Linear)).unwrap();
        fs::write(dir.join("Good.json"), &preset_json).unwrap();
        fs::write(dir.join("X.json.bak"), &preset_json).unwrap();
        fs::write(dir.join("notes.txt"), "notes").unwrap();
        fs::write(dir.join("Bad.json"), "{ not json").unwrap();
        handler.init_presets();
        assert_eq!(names(&handler), ["Good"]);
    }

    #[test]
    fn init_presets_with_empty_dir_clears_ghost_presets() {
        let (_tmp, mut handler) = temp_handler();
        fs::create_dir_all(&handler.presets_dir).unwrap();
        handler.presets = vec![PresetEntity::new("Ghost", data(SplineMode::Linear))];
        handler.preset_selected_index = Some(0);
        handler.init_presets();
        assert!(handler.presets.is_empty());
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn init_presets_with_missing_dir_clears_ghost_presets() {
        let (_tmp, mut handler) = temp_handler();
        handler.presets = vec![PresetEntity::new("Ghost", data(SplineMode::Linear))];
        handler.preset_selected_index = Some(0);
        handler.init_presets();
        assert!(handler.presets.is_empty());
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn init_presets_drops_out_of_range_selection() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "Only", SplineMode::Linear).unwrap();
        handler.preset_selected_index = Some(3);
        handler.init_presets();
        assert_eq!(handler.preset_selected_index, None);
        handler.preset_selected_index = Some(0);
        handler.init_presets();
        assert_eq!(handler.preset_selected_index, Some(0));
    }

    #[test]
    fn preset_handler_serde_skips_preset_list() {
        let (_tmp, mut handler) = temp_handler();
        handler.presets = vec![PresetEntity::new("Pink", data(SplineMode::Linear))];
        let serialized = serde_json::to_value(&handler).unwrap();
        assert!(serialized.get("presets").is_none());
        let saved = serde_json::json!({
            "presets": [PresetEntity::new("Pink", data(SplineMode::Linear))],
            "auto_save_presets": true,
        });
        let loaded: PresetHandler = serde_json::from_value(saved).unwrap();
        assert!(loaded.presets.is_empty());
        assert!(loaded.auto_save_presets);
    }
}
