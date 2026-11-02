use ecolor::HsvaGamma;
use eframe::{
    egui::{self, Layout, PointerButton, Response, Sense, Ui, Vec2},
    epaint::Rect,
};
use serde::{Deserialize, Serialize};
use splines::Spline;

use crate::{
    common::{ColorStringCopy, SplineMode},
    datatypes::{
        control_point::{ControlPoint, ControlPointValue},
        hsv_key_value::HsvKeyValue,
    },
    ui_egui::{
        control_points::{
            control_points_to_spline, flatten_control_points, sample_spline_normalized,
        },
        gradient::color_function_gradient,
        ui_common::color_button,
    },
};

fn ui_previewer_colors(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPointValue],
    _color_copy_format: ColorStringCopy,
) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_control_points = ui.new_child(
        egui::UiBuilder::default()
            .max_rect(rect)
            .layout(Layout::left_to_right(egui::Align::Min)),
    );

    previewer_ui_control_points.spacing_mut().item_spacing = Vec2::ZERO;

    let ui_size: Vec2 = previewer_ui_control_points.available_size();

    let num_control_points = control_points.len();
    let size_per_color_x = ui_size.x / (num_control_points as f32);
    let size_per_color_y = ui_size.y;

    let mut points: Vec<Vec2> = Vec::with_capacity(num_control_points);
    for cp in control_points {
        points.push(Vec2::new(cp[0], cp[1]));
    }

    for i in 0..num_control_points {
        if points.len() <= i {
            break;
        }
        let color_data = &points[i];
        let color_data_hue = control_points[i][2];
        let color_at_point: HsvaGamma = HsvaGamma {
            h: color_data_hue,
            s: color_data.x,
            v: color_data.y,
            a: 1.0,
        };

        let size_weight: f32 = 1.0;
        color_button(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            color_at_point.into(),
            true,
        );
    }

    response
}

fn ui_previewer_control_points_with_drag(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPoint],
    previewer_data: &mut PreviewerData,
    _color_copy_format: ColorStringCopy,
) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_control_points = ui.new_child(
        egui::UiBuilder::default()
            .max_rect(rect)
            .layout(Layout::left_to_right(egui::Align::Min)),
    );

    previewer_ui_control_points.spacing_mut().item_spacing = Vec2::ZERO;

    let ui_size: Vec2 = previewer_ui_control_points.available_size();

    let num_control_points = control_points.len();
    let size_per_color_x = ui_size.x / (num_control_points as f32);
    let size_per_color_y = ui_size.y;
    let previewer_sizes_sum: f32 = previewer_data.points_preview_sizes.iter().sum();

    let mut points: Vec<Vec2> = Vec::with_capacity(num_control_points);
    for cp in control_points {
        points.push(Vec2::new(cp.val()[0], cp.val()[1]));
    }

    for i in 0..num_control_points {
        if points.len() <= i {
            break;
        }
        let color_data = &points[i];
        let color_data_hue = control_points[i].val().h();
        let color_at_point: HsvaGamma = HsvaGamma {
            h: color_data_hue,
            s: color_data.x,
            v: color_data.y,
            a: 1.0,
        };

        let size_weight: f32 = previewer_data.points_preview_sizes[i] * num_control_points as f32
            / previewer_sizes_sum;
        let response_button: Response = color_button(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            color_at_point.into(),
            true,
        );

        if response_button.dragged_by(PointerButton::Primary) {
            const PREVIEWER_DRAG_SENSITIVITY: f32 = 0.6;
            previewer_data.points_preview_sizes[i] +=
                response_button.drag_delta().x * PREVIEWER_DRAG_SENSITIVITY;
            previewer_data.points_preview_sizes[i] =
                previewer_data.points_preview_sizes[i].max(0.0);

            let min_percentage_x = 0.5 * (1.0 / num_control_points as f32);
            let min_preview_size: f32 = min_percentage_x * previewer_sizes_sum;

            // TODO: loop over all and set min_preview_size
            previewer_data.enforce_min_size(min_preview_size);
        }

        let _color_response_rect = response_button.ctx.available_rect();
    }

    response
}

fn modify_spline_t_to_preview_sizes(
    spline: Spline<f32, ControlPointValue>,
    spline_mode: SplineMode,
    previewer_data: &PreviewerData,
) -> Spline<f32, ControlPointValue> {
    let preview_sizes = &previewer_data.points_preview_sizes;

    let _hermite_index_offset = match spline_mode {
        SplineMode::HermiteBezier => {
            if spline.len() >= 2 {
                1
            } else {
                0
            }
        }
        _ => 0,
    };

    let mut spline_as_vec = spline.keys().to_vec();
    let mut accum_size = 0.0;
    for i in 0..preview_sizes.len() {
        let center_of_preview = accum_size + preview_sizes[i] * 0.5;
        spline_as_vec[i + 0].t = center_of_preview;

        accum_size += preview_sizes[i];
    }

    Spline::from_vec(spline_as_vec)
}

fn ui_previewer_curve(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_curve = ui.new_child(
        egui::UiBuilder::default()
            .max_rect(rect)
            .layout(Layout::left_to_right(egui::Align::Min)),
    );
    previewer_ui_curve.spacing_mut().item_spacing = Vec2::ZERO;

    let flatten_control_points = flatten_control_points(control_points);
    let spline = control_points_to_spline(&flatten_control_points[..], spline_mode);

    // forgot what this does
    // match spline_mode {
    //     SplineMode::HermiteBezier => {}
    //     _ => spline = modify_spline_t_to_preview_sizes(spline, spline_mode, previewer_data),
    // };

    let response = color_function_gradient(&mut previewer_ui_curve, rect.size(), |x| {
        if flatten_control_points.is_empty() {
            return HsvaGamma::default().into();
        } else if flatten_control_points.len() == 1 {
            return flatten_control_points[0].val().color();
        }

        let sample: HsvKeyValue =
            sample_spline_normalized(&spline, spline_mode, x).unwrap_or(HsvKeyValue {
                val: [1.0, 1.0, 1.0],
            });
        sample.color()
    });

    response
}

fn ui_previewer_curve_quantized(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
    number_levels: usize,
) -> Response {
    let quantized_colors =
        quantized_colors(control_points, spline_mode, previewer_data, number_levels);

    let response = ui_previewer_colors(ui, size, &quantized_colors, color_copy_format);
    response
}

fn quantized_colors(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    previewer_data: &PreviewerData,
    levels: usize,
) -> Vec<HsvKeyValue> {
    let flatten_control_points = flatten_control_points(control_points);
    let mut spline = control_points_to_spline(&flatten_control_points[..], spline_mode);

    match spline_mode {
        SplineMode::HermiteBezier => {}
        _ => spline = modify_spline_t_to_preview_sizes(spline, spline_mode, previewer_data),
    };

    (0..levels)
        .map(|i| {
            let x = i as f32 / (levels - 1).max(1) as f32;
            sample_spline_normalized(&spline, spline_mode, x).unwrap_or_default()
        })
        .collect()
}

fn previewer_strip_heights(total_height: f32, controls_height: f32) -> [f32; 3] {
    let strips_height = (total_height - controls_height).max(0.0);
    [0.2, 0.4, 0.4].map(|share| share * strips_height)
}

const MAX_QUANTIZE_LEVELS: usize = 32;

fn ui_previewer_options(ui: &mut Ui, previewer_data: &mut PreviewerData) {
    ui.horizontal(|ui| {
        ui.label("Levels");
        ui.add(egui::Slider::new(
            &mut previewer_data.quantize_num_levels,
            1..=MAX_QUANTIZE_LEVELS,
        ))
        .on_hover_text("Number of quantized colors");
        if ui
            .button("Reset sizes")
            .on_hover_text("Reset the color band widths")
            .clicked()
        {
            previewer_data.reset_preview_sizes();
        }
    });
}

pub fn ui_previewer(
    ui: &mut Ui,
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
) -> PreviewerUiResponses {
    let previewer_rect = ui.available_rect_before_wrap();
    let gaps_after_strips = 3.0 * ui.spacing().item_spacing.y;
    let controls_height = ui.spacing().interact_size.y + gaps_after_strips;
    let [control_points_height, curve_height, quantized_height] =
        previewer_strip_heights(previewer_rect.height(), controls_height);
    let width = previewer_rect.width();

    let inner_response = ui.vertical(|ui| {
        let control_points = &previewer_data.control_points.clone();

        let response_control_points = ui_previewer_control_points_with_drag(
            ui,
            Vec2::new(width, control_points_height),
            control_points,
            previewer_data,
            color_copy_format,
        );
        let response_curve = ui_previewer_curve(
            ui,
            Vec2::new(width, curve_height),
            control_points,
            spline_mode,
        );
        let response_curve_quantized = ui_previewer_curve_quantized(
            ui,
            Vec2::new(width, quantized_height),
            control_points,
            spline_mode,
            previewer_data,
            color_copy_format,
            previewer_data.quantize_num_levels,
        );
        ui_previewer_options(ui, previewer_data);

        PreviewerUiResponses {
            controlpoints: Some(response_control_points.clone()),
            curve: Some(response_curve.clone()),
            quantized_curve: Some(response_curve_quantized.clone()),
        }
    });

    inner_response.inner
}

#[derive(Debug)]
pub struct PreviewerUiResponses {
    pub controlpoints: Option<Response>,
    pub curve: Option<Response>,
    pub quantized_curve: Option<Response>,
}

impl Default for PreviewerUiResponses {
    fn default() -> Self {
        Self {
            controlpoints: None,
            curve: None,
            quantized_curve: None,
        }
    }
}

impl PreviewerUiResponses {
    pub fn get_rects(&self) -> Vec<Rect> {
        let mut res_vec = Vec::new();

        if let Some(response) = &self.controlpoints {
            res_vec.push(response.rect.clone());
        }
        if let Some(response) = &self.curve {
            res_vec.push(response.rect.clone());
        }
        if let Some(response) = &self.quantized_curve {
            res_vec.push(response.rect.clone());
        }

        res_vec
    }
}

const PREVIEWER_DEFAULT_VALUE: f32 = 100.0;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewerData {
    pub control_points: Vec<ControlPoint>,
    pub spline_mode: SplineMode,
    pub points_preview_sizes: Vec<f32>,
    pub quantize_num_levels: usize,
}

impl PreviewerData {
    pub fn new(num: usize) -> Self {
        Self {
            points_preview_sizes: vec![PREVIEWER_DEFAULT_VALUE; num],
            control_points: vec![ControlPoint::default(); num],
            spline_mode: SplineMode::HermiteBezier,
            quantize_num_levels: 4,
        }
    }
    pub fn reset_preview_sizes(&mut self) {
        for val in self.points_preview_sizes.iter_mut() {
            *val = PREVIEWER_DEFAULT_VALUE;
        }
    }

    pub fn enforce_min_size(&mut self, min_size: f32) {
        for point_ref in &mut self.points_preview_sizes {
            *point_ref = point_ref.max(min_size);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZPreviewer {
    pub data: PreviewerData,
}

impl ZPreviewer {
    pub fn default() -> Self {
        Self {
            data: PreviewerData::new(0),
        }
    }

    pub fn update(&mut self, control_points: &[ControlPoint], spline_mode: SplineMode) {
        self.data.spline_mode = spline_mode;

        let old_size = self.data.control_points.len();
        let new_size = control_points.len();
        self.data.control_points.clear();
        self.data.control_points.extend_from_slice(control_points);

        if old_size != new_size {
            self.data.points_preview_sizes = vec![PREVIEWER_DEFAULT_VALUE; new_size];
        }
    }

    pub fn draw_ui(
        &mut self,
        ui: &mut Ui,
        color_copy_format: ColorStringCopy,
    ) -> PreviewerUiResponses {
        let previewer_response =
            ui_previewer(ui, self.data.spline_mode, &mut self.data, color_copy_format);

        previewer_response
    }
}

impl Default for ZPreviewer {
    fn default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_heights_plus_controls_fill_total() {
        let heights = previewer_strip_heights(600.0, 27.0);
        let sum: f32 = heights.iter().sum();
        assert!((sum + 27.0 - 600.0).abs() < 1e-3, "{heights:?}");
    }

    #[test]
    fn strip_heights_ratio_is_1_2_2() {
        let [swatches, curve, quantized] = previewer_strip_heights(527.0, 27.0);
        assert!((swatches - 100.0).abs() < 1e-3, "{swatches}");
        assert!((curve - 2.0 * swatches).abs() < 1e-3);
        assert!((quantized - 2.0 * swatches).abs() < 1e-3);
    }

    #[test]
    fn strip_heights_are_zero_when_controls_do_not_fit() {
        let heights = previewer_strip_heights(10.0, 27.0);
        assert!(heights.iter().all(|&height| height >= 0.0), "{heights:?}");
        assert_eq!(heights.iter().sum::<f32>(), 0.0);
    }

    fn assert_close(actual: HsvKeyValue, expected: HsvKeyValue) {
        for c in 0..3 {
            assert!(
                (actual[c] - expected[c]).abs() < 1e-5,
                "{actual:?} != {expected:?}"
            );
        }
    }

    fn two_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.1, 0.2, 0.3), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.9, 0.8, 0.3), 1.0),
        ]
    }

    #[test]
    fn quantized_colors_two_points_exceed_point_count() {
        let cps = two_points();
        let colors = quantized_colors(&cps, SplineMode::Linear, &PreviewerData::new(2), 8);
        assert_eq!(colors.len(), 8);
        assert_close(colors[0], *cps[0].val());
        assert_close(colors[7], *cps[1].val());
        assert!(colors.windows(2).all(|w| w[0].s() < w[1].s()), "{colors:?}");
    }

    #[test]
    fn quantized_colors_one_level_is_first_point_and_zero_is_empty() {
        let cps = two_points();
        let data = PreviewerData::new(2);
        let one = quantized_colors(&cps, SplineMode::Linear, &data, 1);
        assert_eq!(one.len(), 1);
        assert_close(one[0], *cps[0].val());
        assert!(quantized_colors(&cps, SplineMode::Linear, &data, 0).is_empty());
    }

    #[test]
    fn quantized_colors_levels_equal_to_points_hit_every_key() {
        let cps = vec![
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.2), 0.5),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.2), 1.0),
        ];
        let colors = quantized_colors(&cps, SplineMode::Linear, &PreviewerData::new(3), 3);
        assert_eq!(colors.len(), 3);
        for (color, cp) in colors.iter().zip(&cps) {
            assert_close(*color, *cp.val());
        }
    }
}
