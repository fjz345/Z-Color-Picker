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

    pub fn into(self) -> (Vec<ControlPoint>, SplineMode) {
        (self.data.control_points, self.data.spline_mode)
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

    match remove_file(file_path) {
        Ok(()) => log::info!("DELETED {}", file_path.display()),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            log::warn!("{} was already deleted: {e}", file_path.display())
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn preset_file_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

fn ensure_no_preset_file(dir: &Path, name: &str) -> Result<()> {
    if preset_file_path(dir, name).try_exists()? {
        return Err(ZError::Message(format!(
            "A file named {name}.json already exists in the presets folder"
        )));
    }
    Ok(())
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
    #[serde(skip)]
    failed_auto_save: Option<AppDataCtx>,
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
            failed_auto_save: None,
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

    pub fn apply_selected_preset(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
    ) {
        if let Some(preset) = self.preset_selected_index.and_then(|i| self.presets.get(i)) {
            let (preset_control_points, preset_spline_mode) = preset.clone().into();
            *control_points = preset_control_points;
            *spline_mode = preset_spline_mode;
            self.failed_auto_save = None;
            log::info!("Preset Applied!");
        } else {
            log::info!("No preset selected");
        }
    }

    pub fn set_selected_data_in_memory(
        &mut self,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
    ) {
        let preset = self
            .preset_selected_index
            .and_then(|s| self.presets.get_mut(s))
            .expect("an applied preset is selected");
        preset.data = (control_points.to_vec(), spline_mode).into();
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

    pub fn save_selected_preset(
        &mut self,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
    ) -> Result<()> {
        let Some(preset) = self
            .preset_selected_index
            .and_then(|s| self.presets.get_mut(s))
        else {
            return Err(ZError::Message(
                "Preset Save failed, No preset selected".to_string(),
            ));
        };
        let attempt =
            PresetEntity::new(&preset.name, (control_points.to_vec(), spline_mode).into());
        save_preset_to_disk(&self.presets_dir, &attempt)?;
        preset.data = attempt.data;
        self.failed_auto_save = None;
        Ok(())
    }

    pub fn create_preset(
        &mut self,
        name: &String,
        control_points: &Vec<ControlPoint>,
        spline_mode: &SplineMode,
    ) -> Result<()> {
        self.check_name(name, None)?;
        ensure_no_preset_file(&self.presets_dir, name)?;

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
        if new_name.to_lowercase() != self.presets[index].name.to_lowercase() {
            ensure_no_preset_file(&self.presets_dir, new_name)?;
        }
        fs::rename(
            preset_file_path(&self.presets_dir, &self.presets[index].name),
            preset_file_path(&self.presets_dir, new_name),
        )?;
        self.presets[index].name = new_name.to_string();
        save_preset_to_disk(&self.presets_dir, &self.presets[index])
    }

    pub fn is_selected_dirty(
        &self,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
    ) -> bool {
        self.preset_selected_index
            .and_then(|s| self.presets.get(s))
            .is_some_and(|preset| !data_matches(&preset.data, control_points, spline_mode))
    }

    pub fn auto_save(
        &mut self,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
    ) -> Result<bool> {
        let already_failed = self
            .failed_auto_save
            .as_ref()
            .is_some_and(|failed| data_matches(failed, control_points, spline_mode));
        if !self.auto_save_presets
            || already_failed
            || !self.is_selected_dirty(control_points, spline_mode)
        {
            return Ok(false);
        }
        let preset = self
            .preset_selected_index
            .and_then(|s| self.presets.get_mut(s))
            .expect("is_selected_dirty implies a valid selection");
        let attempt =
            PresetEntity::new(&preset.name, (control_points.to_vec(), spline_mode).into());
        match save_preset_to_disk(&self.presets_dir, &attempt) {
            Ok(()) => {
                preset.data = attempt.data;
                self.failed_auto_save = None;
                Ok(true)
            }
            Err(e) => {
                self.failed_auto_save = Some(attempt.data);
                Err(e)
            }
        }
    }

    pub fn has_failed_auto_save(&self) -> bool {
        self.failed_auto_save.is_some()
    }
}

fn data_matches(
    data: &AppDataCtx,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> bool {
    data.spline_mode == spline_mode && data.control_points == control_points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::{ControlPointStorage, ControlPointValue};
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

    fn sample_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::ControlPointSimple(ControlPointStorage {
                val: ControlPointValue::new(0.25, 0.5, 0.75),
                t: 0.0,
                tangents: [None, Some(ControlPointValue::new(0.125, -0.25, 0.0))],
            }),
            ControlPoint::ControlPointLeftRightTangent(ControlPointStorage {
                val: ControlPointValue::new(0.5, 0.25, 0.125),
                t: 1.0,
                tangents: [
                    Some(ControlPointValue::new(-0.5, 0.0, 0.0)),
                    Some(ControlPointValue::new(0.5, 0.0, 0.0)),
                ],
            }),
        ]
    }

    const SAMPLE_PRESET_JSON: &str = r#"{
  "spline_mode": "Bezier",
  "control_points": [
    {
      "ControlPointSimple": {
        "val": { "val": [0.25, 0.5, 0.75] },
        "t": 0.0,
        "tangents": [null, { "val": [0.125, -0.25, 0.0] }]
      }
    },
    {
      "ControlPointLeftRightTangent": {
        "val": { "val": [0.5, 0.25, 0.125] },
        "t": 1.0,
        "tangents": [{ "val": [-0.5, 0.0, 0.0] }, { "val": [0.5, 0.0, 0.0] }]
      }
    }
  ]
}"#;

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
        fs::create_dir_all(handler.presets_dir.join("A.json")).unwrap();
        handler.presets = vec![PresetEntity::new("A", data(SplineMode::Linear))];
        handler.preset_selected_index = Some(0);
        assert!(handler.delete_selected_preset().is_err());
        assert_eq!(handler.presets.len(), 1);
        assert_eq!(handler.preset_selected_index, Some(0));
    }

    #[test]
    fn delete_selected_preset_succeeds_when_file_already_gone() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "A", SplineMode::Linear).unwrap();
        fs::remove_file(handler.presets_dir.join("A.json")).unwrap();
        handler.delete_selected_preset().unwrap();
        assert!(handler.presets.is_empty());
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn apply_selected_preset_without_selection_keeps_points_and_mode() {
        let (_tmp, mut handler) = temp_handler();
        handler.presets = vec![PresetEntity::new("Pink", data(SplineMode::Linear))];
        let mut points = sample_points();
        let mut spline_mode = SplineMode::Bezier;
        handler.apply_selected_preset(&mut points, &mut spline_mode);
        assert_eq!(points, sample_points());
        assert_eq!(spline_mode, SplineMode::Bezier);
    }

    #[test]
    fn create_preset_refuses_unlisted_existing_file() {
        let (_tmp, mut handler) = temp_handler();
        fs::create_dir_all(&handler.presets_dir).unwrap();
        let old_path = handler.presets_dir.join("old.json");
        fs::write(&old_path, "{ garbage").unwrap();
        handler.init_presets();
        assert!(create(&mut handler, "old", SplineMode::Linear).is_err());
        assert_eq!(fs::read_to_string(&old_path).unwrap(), "{ garbage");
        assert!(handler.presets.is_empty());
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
    fn rename_preset_refuses_unlisted_existing_destination() {
        let (_tmp, mut handler) = temp_handler();
        create(&mut handler, "Src", SplineMode::Linear).unwrap();
        let src_path = handler.presets_dir.join("Src.json");
        let src_content = fs::read_to_string(&src_path).unwrap();
        let dest_path = handler.presets_dir.join("Dest.json");
        fs::write(&dest_path, "{ garbage").unwrap();
        assert!(handler.rename_preset(0, "Dest").is_err());
        assert_eq!(fs::read_to_string(&src_path).unwrap(), src_content);
        assert_eq!(fs::read_to_string(&dest_path).unwrap(), "{ garbage");
        assert_eq!(names(&handler), ["Src"]);
    }

    #[test]
    fn init_presets_loads_pinned_json_format() {
        let (_tmp, mut handler) = temp_handler();
        fs::create_dir_all(&handler.presets_dir).unwrap();
        fs::write(handler.presets_dir.join("Golden.json"), SAMPLE_PRESET_JSON).unwrap();
        handler.init_presets();
        assert_eq!(names(&handler), ["Golden"]);
        let expected = AppDataCtx {
            spline_mode: SplineMode::Bezier,
            control_points: sample_points(),
        };
        assert_eq!(handler.presets[0].data, expected);
    }

    #[test]
    fn created_preset_round_trips_through_disk() {
        let (_tmp, mut handler) = temp_handler();
        handler
            .create_preset(&"Trip".to_string(), &sample_points(), &SplineMode::Bezier)
            .unwrap();
        let mut reloaded = PresetHandler::new(handler.presets_dir.clone());
        reloaded.init_presets();
        assert_eq!(names(&reloaded), ["Trip"]);
        assert_eq!(reloaded.presets[0].data, handler.presets[0].data);
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

    fn create_sample(handler: &mut PresetHandler) {
        handler
            .create_preset(&"A".to_string(), &sample_points(), &SplineMode::Bezier)
            .unwrap();
    }

    fn moved_points(s: f32) -> Vec<ControlPoint> {
        let mut points = sample_points();
        *points[0].val_mut() = ControlPointValue::new(s, 0.5, 0.75);
        points
    }

    fn read_preset_file(handler: &PresetHandler) -> String {
        fs::read_to_string(handler.presets_dir.join("A.json")).unwrap()
    }

    #[test]
    fn is_selected_dirty_false_right_after_create() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        assert!(!handler.is_selected_dirty(&sample_points(), SplineMode::Bezier));
    }

    #[test]
    fn is_selected_dirty_true_after_point_or_mode_change() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        assert!(handler.is_selected_dirty(&moved_points(0.9), SplineMode::Bezier));
        assert!(handler.is_selected_dirty(&sample_points(), SplineMode::Linear));
    }

    #[test]
    fn is_selected_dirty_false_without_valid_selection() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        handler.preset_selected_index = None;
        assert!(!handler.is_selected_dirty(&moved_points(0.9), SplineMode::Linear));
        handler.preset_selected_index = Some(5);
        assert!(!handler.is_selected_dirty(&moved_points(0.9), SplineMode::Linear));
    }

    #[test]
    fn auto_save_off_does_not_write() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        let before = read_preset_file(&handler);
        let saved = handler.auto_save(&moved_points(0.9), SplineMode::Linear);
        assert!(!saved.unwrap());
        assert_eq!(read_preset_file(&handler), before);
    }

    #[test]
    fn auto_save_clean_preset_does_not_write() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        handler.auto_save_presets = true;
        assert!(!handler
            .auto_save(&sample_points(), SplineMode::Bezier)
            .unwrap());
    }

    #[test]
    fn auto_save_dirty_preset_writes_and_marks_clean() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        handler.auto_save_presets = true;
        assert!(handler
            .auto_save(&moved_points(0.9), SplineMode::Linear)
            .unwrap());
        let on_disk: AppDataCtx = serde_json::from_str(&read_preset_file(&handler)).unwrap();
        assert_eq!(on_disk, (moved_points(0.9), SplineMode::Linear).into());
        assert!(!handler.is_selected_dirty(&moved_points(0.9), SplineMode::Linear));
    }

    #[test]
    fn auto_save_with_stale_index_does_nothing() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        handler.auto_save_presets = true;
        handler.preset_selected_index = Some(5);
        assert!(!handler
            .auto_save(&moved_points(0.9), SplineMode::Linear)
            .unwrap());
    }

    fn unwritable_handler_with_a(tmp: &TempDir) -> PresetHandler {
        let regular_file = tmp.path().join(PRESETS_FOLDER_NAME);
        fs::write(&regular_file, "").unwrap();
        let mut handler = PresetHandler::new(regular_file);
        handler.presets = vec![PresetEntity::new("A", data(SplineMode::Linear))];
        handler.preset_selected_index = Some(0);
        handler
    }

    #[test]
    fn auto_save_failure_keeps_dirty_and_retries_only_new_data() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a(&tmp);
        handler.auto_save_presets = true;

        assert!(handler
            .auto_save(&moved_points(0.9), SplineMode::Linear)
            .is_err());
        assert!(handler.is_selected_dirty(&moved_points(0.9), SplineMode::Linear));
        assert!(!handler
            .auto_save(&moved_points(0.9), SplineMode::Linear)
            .unwrap());
        assert!(handler
            .auto_save(&moved_points(0.8), SplineMode::Linear)
            .is_err());
    }

    #[test]
    fn save_selected_preset_writes_the_points_and_updates_data() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);

        handler
            .save_selected_preset(&moved_points(0.9), SplineMode::Linear)
            .unwrap();

        let expected: AppDataCtx = (moved_points(0.9), SplineMode::Linear).into();
        let on_disk: AppDataCtx = serde_json::from_str(&read_preset_file(&handler)).unwrap();
        assert_eq!(on_disk, expected);
        assert_eq!(handler.presets[0].data, expected);
    }

    #[test]
    fn save_selected_preset_failure_keeps_data() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a(&tmp);

        assert!(handler
            .save_selected_preset(&moved_points(0.9), SplineMode::Bezier)
            .is_err());
        assert_eq!(handler.presets[0].data, data(SplineMode::Linear));
    }

    #[test]
    fn save_selected_preset_with_stale_index_errs() {
        let (_tmp, mut handler) = temp_handler();
        create_sample(&mut handler);
        handler.preset_selected_index = Some(5);

        assert!(handler
            .save_selected_preset(&moved_points(0.9), SplineMode::Linear)
            .is_err());
    }

    #[test]
    fn save_selected_preset_success_clears_failed_auto_save() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a(&tmp);
        handler.auto_save_presets = true;
        assert!(handler
            .auto_save(&moved_points(0.9), SplineMode::Linear)
            .is_err());
        assert!(handler.has_failed_auto_save());
        handler.presets_dir = tmp.path().join("writable");

        handler
            .save_selected_preset(&moved_points(0.9), SplineMode::Linear)
            .unwrap();

        assert!(!handler.has_failed_auto_save());
    }

    #[test]
    fn apply_selected_preset_clears_failed_auto_save() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a(&tmp);
        handler.auto_save_presets = true;
        let mut points = moved_points(0.9);
        let mut mode = SplineMode::Linear;
        assert!(handler.auto_save(&points, mode).is_err());

        handler.apply_selected_preset(&mut points, &mut mode);

        assert!(!handler.has_failed_auto_save());
    }
}
