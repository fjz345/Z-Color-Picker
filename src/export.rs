use std::path::{Path, PathBuf};

use ecolor::Color32;
use serde::{Deserialize, Serialize};

use crate::{
    color_space::linear_to_color32,
    error::{Result, ZError},
    fs::write_bytes_to_file,
    preset::PresetEntity,
    ui_egui::{app::AppDataCtx, blend::Gradient},
};

pub const DEFAULT_EXPORT_NAME: &str = "Gradient";
pub const CURVE_KEY_COUNT: usize = 64;
const DEFAULT_UNREAL_PORT: u16 = 30010;
const DEFAULT_UNREAL_ASSET_FOLDER: &str = "/Game/ZColorPicker";
const LOOKUP_STRIP_WIDTH: usize = 256;
const PHOTOSHOP_GRADIENT_STOP_COUNT: usize = 32;
const GRD_VERSION: u16 = 5;
const DESCRIPTOR_VERSION: u32 = 16;
const EMPTY_DESCRIPTOR_NAME: [u8; 6] = [0, 0, 0, 1, 0, 0];
const GRD_GRADIENT_CLASS_NAME: &str = "Gradient";
const GRD_LOCATION_END: i32 = 4096;
const GRD_FULL_SMOOTHNESS: f64 = 4096.0;
const GRD_MIDPOINT_PERCENT: i32 = 50;
const GRD_FULL_OPACITY_PERCENT: f64 = 100.0;
const ASE_VERSION_MAJOR: u16 = 1;
const ASE_VERSION_MINOR: u16 = 0;
const ASE_COLOR_ENTRY: u16 = 0x0001;
const ASE_NORMAL_COLOR: u16 = 2;

pub struct ExportFile {
    pub extension: &'static str,
    pub bytes: Vec<u8>,
}

pub type ExportOutcome = std::result::Result<Vec<PathBuf>, String>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportSettings {
    pub folder: String,
    pub on_save: bool,
    #[serde(skip)]
    pub last_result: Option<ExportOutcome>,
}

impl ExportSettings {
    pub fn folder(&self) -> Option<&Path> {
        let folder = self.folder.trim();
        (!folder.is_empty()).then(|| Path::new(folder))
    }

    pub fn export(&mut self, name: &str, data: &AppDataCtx) {
        let result = match self.folder() {
            Some(dir) => export_gradient(dir, name, data),
            None => Err(ZError::Message("No export folder set".to_string())),
        };
        self.last_result = Some(match result {
            Ok(paths) => {
                for path in &paths {
                    log::info!("Exported {}", path.display());
                }
                Ok(paths)
            }
            Err(e) => {
                let message = format!("Export failed: {e}");
                log::error!("{message}");
                Err(message)
            }
        });
    }

    pub fn export_saved_preset(&mut self, preset: &PresetEntity) {
        if self.on_save && self.folder().is_some() {
            self.export(&preset.name, &preset.data);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnrealLiveSettings {
    pub enabled: bool,
    pub port: u16,
    pub asset_folder: String,
}

impl Default for UnrealLiveSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_UNREAL_PORT,
            asset_folder: DEFAULT_UNREAL_ASSET_FOLDER.to_string(),
        }
    }
}

pub fn export_name(selected_preset: Option<&str>) -> &str {
    selected_preset.unwrap_or(DEFAULT_EXPORT_NAME)
}

pub fn export_gradient(dir: &Path, name: &str, data: &AppDataCtx) -> Result<Vec<PathBuf>> {
    let files = export_files(name, data)?;
    if !dir.is_dir() {
        return Err(ZError::Message(format!(
            "Export folder {} does not exist",
            dir.display()
        )));
    }
    let mut written: Vec<PathBuf> = Vec::new();
    for file in &files {
        let file_name = format!("{name}.{}", file.extension);
        let path = dir.join(&file_name);
        if let Err(e) = write_bytes_to_file(&file.bytes, &path) {
            let written_names: Vec<String> = written
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect();
            let kept = match written_names.as_slice() {
                [] => String::new(),
                names => format!("; {} already rewritten", names.join(", ")),
            };
            return Err(ZError::Message(format!("writing {file_name}: {e}{kept}")));
        }
        written.push(path);
    }
    Ok(written)
}

pub fn gradient(data: &AppDataCtx) -> Gradient {
    Gradient::new(
        &data.control_points,
        data.spline_mode,
        data.picker_space,
        data.blend_space,
    )
}

pub fn export_files(name: &str, data: &AppDataCtx) -> Result<Vec<ExportFile>> {
    let gradient = gradient(data);
    let gradient_stops: Vec<Color32> = bake_linear(&gradient, PHOTOSHOP_GRADIENT_STOP_COUNT)?
        .into_iter()
        .map(linear_to_color32)
        .collect();
    let point_colors: Vec<Color32> = data
        .control_points
        .iter()
        .map(|point| point.val().color(data.picker_space))
        .collect();
    Ok(vec![
        ExportFile {
            extension: "csv",
            bytes: curve_linear_color_csv(&bake_linear(&gradient, CURVE_KEY_COUNT)?).into_bytes(),
        },
        ExportFile {
            extension: "png",
            bytes: lookup_strip_png(&bake_linear(&gradient, LOOKUP_STRIP_WIDTH)?)?,
        },
        ExportFile {
            extension: "grd",
            bytes: photoshop_gradient(name, &gradient_stops)?,
        },
        ExportFile {
            extension: "ase",
            bytes: swatch_exchange(name, &point_colors)?,
        },
    ])
}

fn sample_x(index: usize, count: usize) -> f32 {
    index as f32 / (count - 1) as f32
}

pub fn bake_linear(gradient: &Gradient, count: usize) -> Result<Vec<[f32; 3]>> {
    assert!(count >= 2, "a bake needs both gradient ends");
    (0..count)
        .map(|i| {
            let x = sample_x(i, count);
            gradient
                .linear_at(x)
                .ok_or_else(|| ZError::Message(format!("The gradient has no color at {x}")))
        })
        .collect()
}

fn curve_linear_color_csv(samples: &[[f32; 3]]) -> String {
    samples
        .iter()
        .enumerate()
        .map(|(i, [r, g, b])| {
            let time = sample_x(i, samples.len());
            format!("{time:.6},{r:.6},{g:.6},{b:.6},1.000000\n")
        })
        .collect()
}

pub fn unreal_gradient_array(data: &AppDataCtx) -> Result<String> {
    let samples = bake_linear(&gradient(data), CURVE_KEY_COUNT)?;
    Ok(unreal_linear_color_array(&samples))
}

fn unreal_linear_color_array(samples: &[[f32; 3]]) -> String {
    let colors: Vec<String> = samples
        .iter()
        .map(|&[r, g, b]| unreal_linear_color([r, g, b, 1.0]))
        .collect();
    format!("({})", colors.join(","))
}

pub fn unreal_linear_color([r, g, b, a]: [f32; 4]) -> String {
    format!("(R={r:.6},G={g:.6},B={b:.6},A={a:.6})")
}

fn lookup_strip_png(samples: &[[f32; 3]]) -> Result<Vec<u8>> {
    let pixels: Vec<u8> = samples
        .iter()
        .flat_map(|&linear| {
            let color = linear_to_color32(linear);
            [color.r(), color.g(), color.b()]
        })
        .collect();
    let png_error = |e: png::EncodingError| ZError::Message(format!("PNG encoding failed: {e}"));
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, samples.len() as u32, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(png_error)?;
    writer.write_image_data(&pixels).map_err(png_error)?;
    writer.finish().map_err(png_error)?;
    Ok(bytes)
}

fn photoshop_gradient(name: &str, stops: &[Color32]) -> Result<Vec<u8>> {
    let name_units: u32 = format_count(utf16_units_with_null(name), "Gradient name length")?;
    let stop_count: u32 = format_count(stops.len(), "Gradient stop count")?;
    let mut grd = BigEndianBytes::default();
    grd.bytes(b"8BGR").u16(GRD_VERSION).u32(DESCRIPTOR_VERSION);
    grd.descriptor_object(b"null", 1)
        .descriptor_key(b"GrdL", b"VlLs")
        .u32(1);
    grd.bytes(b"Objc").descriptor_object(b"Grdn", 1);
    grd.descriptor_key(b"Grad", b"Objc")
        .named_descriptor_object(GRD_GRADIENT_CLASS_NAME, b"Grdn", 5);
    grd.descriptor_key(b"Nm  ", b"TEXT")
        .u32(name_units)
        .utf16_with_null(name);
    grd.descriptor_key(b"GrdF", b"enum")
        .descriptor_id(b"GrdF")
        .descriptor_id(b"CstS");
    grd.descriptor_key(b"Intr", b"doub")
        .f64(GRD_FULL_SMOOTHNESS);
    grd.descriptor_key(b"Clrs", b"VlLs").u32(stop_count);
    for (i, color) in stops.iter().enumerate() {
        grd.bytes(b"Objc").descriptor_object(b"Clrt", 4);
        grd.descriptor_key(b"Clr ", b"Objc")
            .descriptor_object(b"RGBC", 3);
        for (key, channel) in [
            (b"Rd  ", color.r()),
            (b"Grn ", color.g()),
            (b"Bl  ", color.b()),
        ] {
            grd.descriptor_key(key, b"doub").f64(channel.into());
        }
        grd.descriptor_key(b"Type", b"enum")
            .descriptor_id(b"Clry")
            .descriptor_id(b"UsrS");
        grd.descriptor_key(b"Lctn", b"long")
            .i32(grd_location(sample_x(i, stops.len())));
        grd.descriptor_key(b"Mdpn", b"long")
            .i32(GRD_MIDPOINT_PERCENT);
    }
    grd.descriptor_key(b"Trns", b"VlLs").u32(2);
    for location in [0, GRD_LOCATION_END] {
        grd.bytes(b"Objc").descriptor_object(b"TrnS", 3);
        grd.descriptor_key(b"Opct", b"UntF")
            .bytes(b"#Prc")
            .f64(GRD_FULL_OPACITY_PERCENT);
        grd.descriptor_key(b"Lctn", b"long").i32(location);
        grd.descriptor_key(b"Mdpn", b"long")
            .i32(GRD_MIDPOINT_PERCENT);
    }
    Ok(grd.0)
}

fn grd_location(t: f32) -> i32 {
    (t * GRD_LOCATION_END as f32).round() as i32
}

fn swatch_exchange(name: &str, colors: &[Color32]) -> Result<Vec<u8>> {
    let mut ase = BigEndianBytes::default();
    ase.bytes(b"ASEF")
        .u16(ASE_VERSION_MAJOR)
        .u16(ASE_VERSION_MINOR)
        .i32(format_count(colors.len(), "Swatch count")?);
    for (i, color) in colors.iter().enumerate() {
        let swatch_name = format!("{name} {}", i + 1);
        let name_units: u16 =
            format_count(utf16_units_with_null(&swatch_name), "Swatch name length")?;
        let mut entry = BigEndianBytes::default();
        entry
            .u16(name_units)
            .utf16_with_null(&swatch_name)
            .bytes(b"RGB ");
        for channel in [color.r(), color.g(), color.b()] {
            entry.f32(f32::from(channel) / 255.0);
        }
        entry.u16(ASE_NORMAL_COLOR);
        ase.u16(ASE_COLOR_ENTRY)
            .i32(format_count(entry.0.len(), "Swatch entry length")?)
            .bytes(&entry.0);
    }
    Ok(ase.0)
}

fn utf16_units_with_null(text: &str) -> usize {
    text.encode_utf16().count() + 1
}

fn format_count<T: TryFrom<usize>>(count: usize, what: &str) -> Result<T> {
    T::try_from(count)
        .map_err(|_| ZError::Message(format!("{what} {count} does not fit the file format")))
}

#[derive(Default)]
struct BigEndianBytes(Vec<u8>);

impl BigEndianBytes {
    fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        self.0.extend_from_slice(bytes);
        self
    }

    fn u16(&mut self, value: u16) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn u32(&mut self, value: u32) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn i32(&mut self, value: i32) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn f32(&mut self, value: f32) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn f64(&mut self, value: f64) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn utf16_with_null(&mut self, text: &str) -> &mut Self {
        for unit in text.encode_utf16().chain([0]) {
            self.u16(unit);
        }
        self
    }

    fn descriptor_id(&mut self, id: &[u8; 4]) -> &mut Self {
        self.u32(0).bytes(id)
    }

    fn descriptor_key(&mut self, key: &[u8; 4], os_type: &[u8; 4]) -> &mut Self {
        self.descriptor_id(key).bytes(os_type)
    }

    fn descriptor_object(&mut self, class_id: &[u8; 4], item_count: u32) -> &mut Self {
        self.bytes(&EMPTY_DESCRIPTOR_NAME)
            .descriptor_id(class_id)
            .u32(item_count)
    }

    fn named_descriptor_object(
        &mut self,
        class_name: &str,
        class_id: &[u8; 4],
        item_count: u32,
    ) -> &mut Self {
        let name_units = utf16_units_with_null(class_name) as u32;
        self.u32(name_units)
            .utf16_with_null(class_name)
            .descriptor_id(class_id)
            .u32(item_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        color_space::{BlendSpace, PickerSpace},
        common::SplineMode,
        datatypes::control_point::{ControlPoint, ControlPointValue},
    };
    use std::fs;
    use tempfile::TempDir;

    fn red_to_blue() -> AppDataCtx {
        AppDataCtx {
            spline_mode: SplineMode::Linear,
            control_points: vec![
                ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.0), 0.0),
                ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 2.0 / 3.0), 1.0),
            ],
            picker_space: PickerSpace::Hsv,
            blend_space: BlendSpace::Picker,
        }
    }

    fn gradient_of(data: &AppDataCtx) -> Gradient {
        Gradient::new(
            &data.control_points,
            data.spline_mode,
            data.picker_space,
            data.blend_space,
        )
    }

    fn file_bytes<'a>(files: &'a [ExportFile], extension: &str) -> &'a [u8] {
        &files
            .iter()
            .find(|file| file.extension == extension)
            .unwrap()
            .bytes
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn settings_for(dir: &Path) -> ExportSettings {
        ExportSettings {
            folder: dir.to_str().unwrap().to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn csv_holds_64_linear_keys_from_0_to_1_with_opaque_alpha() {
        let data = red_to_blue();
        let gradient = gradient_of(&data);
        let files = export_files("Warm", &data).unwrap();
        let csv = std::str::from_utf8(file_bytes(&files, "csv")).unwrap();

        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 64);
        assert_eq!(lines[0], "0.000000,1.000000,0.000000,0.000000,1.000000");
        assert!(lines[63].starts_with("1.000000,"), "{}", lines[63]);
        for (i, line) in lines.iter().enumerate() {
            let cells: Vec<f32> = line.split(',').map(|cell| cell.parse().unwrap()).collect();
            let x = i as f32 / 63.0;
            let linear = gradient.linear_at(x).unwrap();
            assert_eq!(cells.len(), 5, "{line}");
            assert!((cells[0] - x).abs() <= 6e-7, "{line}");
            for (cell, expected) in cells[1..4].iter().zip(linear) {
                assert!((cell - expected).abs() <= 6e-7, "{line} vs {linear:?}");
            }
            assert_eq!(cells[4], 1.0);
        }
    }

    #[test]
    fn png_is_a_256x1_rgb8_strip_with_the_gradient_end_colors_at_its_ends() {
        let data = red_to_blue();
        let gradient = gradient_of(&data);
        let files = export_files("Warm", &data).unwrap();

        let decoder = png::Decoder::new(file_bytes(&files, "png"));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).unwrap();

        assert_eq!((info.width, info.height), (256, 1));
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgb, png::BitDepth::Eight)
        );
        let rgb = |color: Color32| [color.r(), color.g(), color.b()];
        assert_eq!(pixels[..3], rgb(gradient.color_at(0.0).unwrap()));
        assert_eq!(pixels[255 * 3..], rgb(gradient.color_at(1.0).unwrap()));
        assert_eq!(pixels[..3], [255, 0, 0]);
        assert_eq!(pixels[255 * 3..], [0, 0, 255]);
    }

    #[test]
    fn every_spline_mode_picker_space_and_blend_space_bakes_every_file() {
        let control_points = vec![
            ControlPoint::new_simple(ControlPointValue::new(0.2, 0.3, 0.1), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.6, 0.5, 0.4), 0.3),
            ControlPoint::new_simple(ControlPointValue::new(0.8, 0.9, 0.7), 1.0),
        ];
        for spline_mode in SplineMode::ALL {
            for picker_space in PickerSpace::ALL {
                for blend_space in BlendSpace::ALL {
                    let data = AppDataCtx {
                        spline_mode,
                        control_points: control_points.clone(),
                        picker_space,
                        blend_space,
                    };
                    let files = export_files("Warm", &data);
                    assert!(
                        files.as_ref().is_ok_and(|files| files.len() == 4),
                        "{spline_mode:?} {picker_space:?} {blend_space:?}"
                    );
                    let single = AppDataCtx {
                        control_points: control_points[..1].to_vec(),
                        ..data
                    };
                    assert!(
                        export_files("Warm", &single).is_ok(),
                        "one point, {spline_mode:?} {picker_space:?} {blend_space:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn export_name_is_the_selected_preset_else_gradient() {
        assert_eq!(export_name(Some("Warm")), "Warm");
        assert_eq!(export_name(None), "Gradient");
    }

    #[test]
    fn export_writes_every_file_named_after_the_preset() {
        let tmp = TempDir::new().unwrap();
        let data = red_to_blue();

        let paths = export_gradient(tmp.path(), "Warm", &data).unwrap();

        assert_eq!(
            paths,
            ["Warm.csv", "Warm.png", "Warm.grd", "Warm.ase"].map(|name| tmp.path().join(name))
        );
        assert_eq!(
            file_names(tmp.path()),
            ["Warm.ase", "Warm.csv", "Warm.grd", "Warm.png"]
        );
        let files = export_files("Warm", &data).unwrap();
        for (path, extension) in paths.iter().zip(["csv", "png", "grd", "ase"]) {
            assert_eq!(fs::read(path).unwrap(), file_bytes(&files, extension));
        }
    }

    #[test]
    fn a_gradient_without_points_is_an_error_and_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let data = AppDataCtx {
            control_points: Vec::new(),
            ..red_to_blue()
        };

        assert!(export_gradient(tmp.path(), "Empty", &data).is_err());
        assert!(file_names(tmp.path()).is_empty());
    }

    #[test]
    fn failed_write_names_the_failed_file_and_the_ones_already_written() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("Warm.grd")).unwrap();

        let error = export_gradient(tmp.path(), "Warm", &red_to_blue())
            .unwrap_err()
            .to_string();

        assert!(error.contains("Warm.grd"), "{error}");
        assert!(error.contains("Warm.csv, Warm.png"), "{error}");
        assert!(!error.contains("Warm.ase"), "{error}");
    }

    #[test]
    fn export_into_a_missing_folder_reports_the_error_and_creates_nothing() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("missing");
        let mut settings = settings_for(&missing);

        settings.export("Warm", &red_to_blue());

        assert!(
            matches!(&settings.last_result, Some(Err(e)) if e.contains("does not exist")),
            "{:?}",
            settings.last_result
        );
        assert!(!missing.exists());
    }

    #[test]
    fn export_on_save_writes_only_when_on_and_a_folder_is_set() {
        let tmp = TempDir::new().unwrap();
        let preset = PresetEntity::new("Warm", red_to_blue());
        let mut settings = settings_for(tmp.path());

        settings.export_saved_preset(&preset);
        assert!(file_names(tmp.path()).is_empty());
        assert!(settings.last_result.is_none());

        settings.on_save = true;
        settings.folder = " ".to_string();
        settings.export_saved_preset(&preset);
        assert!(settings.last_result.is_none());

        settings.folder = tmp.path().to_str().unwrap().to_string();
        settings.export_saved_preset(&preset);
        assert_eq!(
            file_names(tmp.path()),
            ["Warm.ase", "Warm.csv", "Warm.grd", "Warm.png"]
        );
        assert!(
            matches!(&settings.last_result, Some(Ok(paths)) if paths.len() == 4),
            "{:?}",
            settings.last_result
        );
    }

    fn hex(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        digits
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn assert_fields(actual: &[u8], fields: &[(&str, &str)]) {
        let mut offset = 0;
        for (field, field_hex) in fields {
            let expected = hex(field_hex);
            let end = offset + expected.len();
            assert_eq!(
                actual.get(offset..end),
                Some(&expected[..]),
                "{field} at byte {offset}"
            );
            offset = end;
        }
        assert_eq!(actual.len(), offset, "bytes after the last field");
    }

    fn be_u32(bytes: &[u8], at: usize) -> u32 {
        u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn be_f64(bytes: &[u8], at: usize) -> f64 {
        f64::from_be_bytes(bytes[at..at + 8].try_into().unwrap())
    }

    fn grd_color_stop(
        red: &'static str,
        green: &'static str,
        blue: &'static str,
        location: &'static str,
    ) -> [(&'static str, &'static str); 28] {
        [
            ("stop type", "4F626A63"),
            ("stop class name", "00000001 0000"),
            ("stop class", "00000000 436C7274"),
            ("stop item count", "00000004"),
            ("Clr key", "00000000 436C7220"),
            ("Clr type", "4F626A63"),
            ("Clr class name", "00000001 0000"),
            ("Clr class", "00000000 52474243"),
            ("Clr item count", "00000003"),
            ("Rd key", "00000000 52642020"),
            ("Rd type", "646F7562"),
            ("Rd", red),
            ("Grn key", "00000000 47726E20"),
            ("Grn type", "646F7562"),
            ("Grn", green),
            ("Bl key", "00000000 426C2020"),
            ("Bl type", "646F7562"),
            ("Bl", blue),
            ("Type key", "00000000 54797065"),
            ("Type type", "656E756D"),
            ("Type enum type", "00000000 436C7279"),
            ("Type value", "00000000 55737253"),
            ("Lctn key", "00000000 4C63746E"),
            ("Lctn type", "6C6F6E67"),
            ("Lctn", location),
            ("Mdpn key", "00000000 4D64706E"),
            ("Mdpn type", "6C6F6E67"),
            ("Mdpn", "00000032"),
        ]
    }

    fn grd_transparency_stop(location: &'static str) -> [(&'static str, &'static str); 14] {
        [
            ("stop type", "4F626A63"),
            ("stop class name", "00000001 0000"),
            ("stop class", "00000000 54726E53"),
            ("stop item count", "00000003"),
            ("Opct key", "00000000 4F706374"),
            ("Opct type", "556E7446"),
            ("Opct unit", "23507263"),
            ("Opct", "40590000 00000000"),
            ("Lctn key", "00000000 4C63746E"),
            ("Lctn type", "6C6F6E67"),
            ("Lctn", location),
            ("Mdpn key", "00000000 4D64706E"),
            ("Mdpn type", "6C6F6E67"),
            ("Mdpn", "00000032"),
        ]
    }

    #[test]
    fn grd_of_two_stops_is_the_v5_descriptor_byte_for_byte() {
        let stops = [
            Color32::from_rgb(255, 128, 0),
            Color32::from_rgb(0, 64, 255),
        ];

        let grd = photoshop_gradient("Hi", &stops).unwrap();

        let head = [
            ("signature", "38424752"),
            ("version", "0005"),
            ("descriptor version", "00000010"),
            ("root class name", "00000001 0000"),
            ("root class", "00000000 6E756C6C"),
            ("root item count", "00000001"),
            ("GrdL key", "00000000 4772644C"),
            ("GrdL type", "566C4C73"),
            ("GrdL count", "00000001"),
            ("wrapper type", "4F626A63"),
            ("wrapper class name", "00000001 0000"),
            ("wrapper class", "00000000 4772646E"),
            ("wrapper item count", "00000001"),
            ("Grad key", "00000000 47726164"),
            ("Grad type", "4F626A63"),
            (
                "Grad class name",
                "00000009 0047 0072 0061 0064 0069 0065 006E 0074 0000",
            ),
            ("Grad class", "00000000 4772646E"),
            ("Grad item count", "00000005"),
            ("Nm key", "00000000 4E6D2020"),
            ("Nm type", "54455854"),
            ("Nm", "00000003 0048 0069 0000"),
            ("GrdF key", "00000000 47726446"),
            ("GrdF type", "656E756D"),
            ("GrdF enum type", "00000000 47726446"),
            ("GrdF value", "00000000 43737453"),
            ("Intr key", "00000000 496E7472"),
            ("Intr type", "646F7562"),
            ("Intr", "40B00000 00000000"),
            ("Clrs key", "00000000 436C7273"),
            ("Clrs type", "566C4C73"),
            ("Clrs count", "00000002"),
        ];
        let transparency_list = [
            ("Trns key", "00000000 54726E73"),
            ("Trns type", "566C4C73"),
            ("Trns count", "00000002"),
        ];
        let fields: Vec<(&str, &str)> = head
            .into_iter()
            .chain(grd_color_stop(
                "406FE000 00000000",
                "40600000 00000000",
                "00000000 00000000",
                "00000000",
            ))
            .chain(grd_color_stop(
                "00000000 00000000",
                "40500000 00000000",
                "406FE000 00000000",
                "00001000",
            ))
            .chain(transparency_list)
            .chain(grd_transparency_stop("00000000"))
            .chain(grd_transparency_stop("00001000"))
            .collect();
        assert_fields(&grd, &fields);
        assert_eq!(grd.len(), 714);
    }

    #[test]
    fn grd_export_has_32_ascending_stops_of_the_gradient_named_after_the_export() {
        let data = red_to_blue();
        let gradient = gradient_of(&data);
        let files = export_files("Warm", &data).unwrap();
        let grd = file_bytes(&files, "grd");
        let name_end = 128 + 2 * 5;
        let stops_start = name_end + 64;
        let stop_size = 172;
        let transparency_start = stops_start + 32 * stop_size + 16;

        assert_eq!(grd.len(), 5878);
        assert_eq!(grd[..10], hex("38424752 0005 00000010"));
        assert_eq!(be_u32(grd, 124), 5);
        assert_eq!(grd[128..name_end], hex("0057 0061 0072 006D 0000"));
        assert_eq!(be_u32(grd, stops_start - 4), 32);
        let mut previous_location = None;
        for i in 0..32 {
            let stop = stops_start + i * stop_size;
            let location = be_u32(grd, stop + 152);
            let color = gradient.color_at(i as f32 / 31.0).unwrap();
            assert_eq!(grd[stop..stop + 4], *b"Objc", "stop {i}");
            assert_eq!(location, (i as u32 * 8192 + 31) / 62, "stop {i}");
            assert!(previous_location < Some(location), "stop {i}");
            assert_eq!(
                [64, 84, 104].map(|at| be_f64(grd, stop + at)),
                [color.r(), color.g(), color.b()].map(f64::from),
                "stop {i}"
            );
            previous_location = Some(location);
        }
        assert_eq!(previous_location, Some(4096));
        assert_eq!(be_u32(grd, stops_start + 152), 0);
        assert_eq!(be_u32(grd, transparency_start - 4), 2);
        assert_eq!(be_u32(grd, transparency_start + 58), 0);
        assert_eq!(be_u32(grd, transparency_start + 78 + 58), 4096);
        assert_eq!(transparency_start + 2 * 78, grd.len());
    }

    #[test]
    fn ase_of_one_swatch_is_the_block_layout_byte_for_byte() {
        let ase = swatch_exchange("Hi", &[Color32::from_rgb(255, 102, 51)]).unwrap();

        assert_fields(
            &ase,
            &[
                ("signature", "41534546"),
                ("version", "0001 0000"),
                ("block count", "00000001"),
                ("block type", "0001"),
                ("block length", "0000001E"),
                ("name length", "0005"),
                ("name", "0048 0069 0020 0031 0000"),
                ("color model", "52474220"),
                ("red", "3F800000"),
                ("green", "3ECCCCCD"),
                ("blue", "3E4CCCCD"),
                ("color type", "0002"),
            ],
        );
        assert_eq!(ase.len(), 48);
    }

    #[test]
    fn ase_export_has_one_numbered_swatch_per_control_point_in_its_color() {
        let data = AppDataCtx {
            control_points: vec![
                ControlPoint::new_simple(ControlPointValue::new(0.2, 0.3, 0.1), 0.0),
                ControlPoint::new_simple(ControlPointValue::new(0.6, 0.5, 0.4), 0.3),
                ControlPoint::new_simple(ControlPointValue::new(0.8, 0.9, 0.7), 1.0),
            ],
            picker_space: PickerSpace::Okhsl,
            ..red_to_blue()
        };

        let files = export_files("Warm", &data).unwrap();

        let ase = file_bytes(&files, "ase");
        assert_eq!(ase[..12], hex("41534546 0001 0000 00000003"));
        let block_size = 6 + 2 + 2 * 7 + 4 + 12 + 2;
        assert_eq!(ase.len(), 12 + 3 * block_size);
        for (i, point) in data.control_points.iter().enumerate() {
            let block = &ase[12 + i * block_size..12 + (i + 1) * block_size];
            let color = point.val().color(PickerSpace::Okhsl);
            let name: Vec<u8> = format!("Warm {}", i + 1)
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_be_bytes)
                .collect();
            let rgb: Vec<u8> = [color.r(), color.g(), color.b()]
                .into_iter()
                .flat_map(|channel| (f32::from(channel) / 255.0).to_be_bytes())
                .collect();
            assert_eq!(block[..8], hex("0001 00000022 0007"), "swatch {i}");
            assert_eq!(block[8..22], name, "swatch {i}");
            assert_eq!(block[22..26], *b"RGB ", "swatch {i}");
            assert_eq!(block[26..38], rgb, "swatch {i}");
            assert_eq!(block[38..], hex("0002"), "swatch {i}");
        }
    }

    #[test]
    fn unreal_array_of_two_samples_is_opaque_linear_colors_without_spaces() {
        assert_eq!(
            unreal_linear_color_array(&[[1.0, 0.5, 0.0], [0.25, 0.0, 1.0]]),
            "((R=1.000000,G=0.500000,B=0.000000,A=1.000000),\
             (R=0.250000,G=0.000000,B=1.000000,A=1.000000))"
        );
    }

    #[test]
    fn unreal_gradient_array_holds_the_64_baked_linear_colors() {
        let data = red_to_blue();
        let samples = bake_linear(&gradient_of(&data), 64).unwrap();

        let text = unreal_gradient_array(&data).unwrap();

        assert_eq!(text, unreal_linear_color_array(&samples));
        assert_eq!(text.matches("),(").count(), 63);
        assert!(
            text.starts_with("((R=1.000000,G=0.000000,B=0.000000,A=1.000000),(R="),
            "{text}"
        );
        assert!(
            text.ends_with("),(R=0.000000,G=0.000000,B=1.000000,A=1.000000))"),
            "{text}"
        );
    }

    #[test]
    fn unreal_gradient_array_without_points_is_an_error() {
        let data = AppDataCtx {
            control_points: Vec::new(),
            ..red_to_blue()
        };

        assert!(unreal_gradient_array(&data).is_err());
    }

    #[test]
    fn settings_load_from_state_saved_before_they_existed() {
        let settings: ExportSettings = serde_json::from_str("{}").unwrap();

        assert_eq!(settings.folder, "");
        assert!(!settings.on_save);
    }

    #[test]
    fn unreal_live_settings_load_from_state_saved_before_they_existed() {
        let settings: UnrealLiveSettings = serde_json::from_str("{}").unwrap();

        assert_eq!(
            settings,
            UnrealLiveSettings {
                enabled: false,
                port: 30010,
                asset_folder: "/Game/ZColorPicker".to_string(),
            }
        );
    }
}
