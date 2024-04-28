use ecolor::HsvaGamma;
use eframe::{
    egui::{self, Layout, PointerButton, Response, Sense, Ui, Vec2},
    epaint::Rect,
};
use splines::Spline;

#[allow(unused_imports)]
use crate::error::Result;
use crate::{
    color_picker::{ColorStringCopy, ControlPoint, SplineMode},
    curves::{control_points_to_spline, find_spline_max_t, flatten_control_points},
    gradient::color_function_gradient,
    hsv_key_value::HsvKeyValue,
    ui_common::{color_button, response_copy_color_on_click},
    ControlPointType,
};

fn ui_previewer_colors(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPointType],
    color_copy_format: ColorStringCopy,
) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_control_points =
        ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));

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
        let response_button: Response = color_button(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            color_at_point.into(),
            true,
        );

        response_copy_color_on_click(
            ui,
            &response_button,
            color_at_point,
            color_copy_format,
            PointerButton::Middle,
        );
    }

    response
}

fn ui_previewer_control_points_with_drag(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPoint],
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_control_points =
        ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));

    previewer_ui_control_points.spacing_mut().item_spacing = Vec2::ZERO;

    let ui_size: Vec2 = previewer_ui_control_points.available_size();

    let num_control_points = control_points.len();
    let size_per_color_x = ui_size.x / (num_control_points as f32);
    let size_per_color_y = ui_size.y;
    let previewer_sizes_sum: f32 = previewer_data.points_preview_sizes.iter().sum();

    let mut points: Vec<Vec2> = Vec::with_capacity(num_control_points);
    for cp in control_points {
        points.push(Vec2::new(cp.val[0], cp.val[1]));
    }

    for i in 0..num_control_points {
        if points.len() <= i {
            break;
        }
        let color_data = &points[i];
        let color_data_hue = control_points[i].val.h();
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

        response_copy_color_on_click(
            ui,
            &response_button,
            color_at_point,
            color_copy_format,
            PointerButton::Middle,
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

        let _color_response_rect = response_button.ctx.screen_rect();
    }

    response
}

fn modify_spline_t_to_preview_sizes(
    spline: Spline<f32, ControlPointType>,
    spline_mode: SplineMode,
    previewer_data: &PreviewerData,
) -> Spline<f32, ControlPointType> {
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
    previewer_data: &PreviewerData,
) {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_curve = ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));
    previewer_ui_curve.spacing_mut().item_spacing = Vec2::ZERO;

    let flatten_control_points = flatten_control_points(control_points);
    let mut spline = control_points_to_spline(&flatten_control_points[..], spline_mode);

    match spline_mode {
        SplineMode::HermiteBezier => {}
        _ => spline = modify_spline_t_to_preview_sizes(spline, spline_mode, previewer_data),
    };

    let max_t = find_spline_max_t(&spline);

    let response = color_function_gradient(&mut previewer_ui_curve, rect.size(), |x| {
        if flatten_control_points.len() <= 0 {
            return HsvaGamma {
                h: 0.0,
                s: 0.0,
                v: 0.0,
                a: 0.0,
            }
            .into();
        } else if flatten_control_points.len() <= 1 {
            return flatten_control_points[0].val.color();
        }

        let sample_x = match spline_mode {
            SplineMode::HermiteBezier => 1.0 + x * (max_t - 2.0) as f32,
            _ => x * max_t,
        };

        let sample: HsvKeyValue = spline.clamped_sample(sample_x).unwrap_or_default();
        sample.color()
    });
}

fn ui_previewer_curve_quantized(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
    number_levels: usize,
) {
    let flatten_control_points = flatten_control_points(control_points);
    let mut spline = control_points_to_spline(&flatten_control_points[..], spline_mode);

    match spline_mode {
        SplineMode::HermiteBezier => {}
        _ => spline = modify_spline_t_to_preview_sizes(spline, spline_mode, previewer_data),
    };

    let max_t = find_spline_max_t(&spline);

    let mut quantized_colors: Vec<HsvKeyValue> = Vec::new();
    for i in 0..number_levels {
        let sample_x = match spline_mode {
            SplineMode::HermiteBezier => {
                1.0 + i as f32 / (number_levels) as f32 as f32 * (max_t - 2.0) as f32
            }
            _ => i as f32 / (number_levels - 1).max(1) as f32 as f32 * max_t,
        };

        let sample = spline.clamped_sample(sample_x).unwrap_or_default();
        quantized_colors.push(sample);
    }

    ui_previewer_colors(ui, size, &quantized_colors, color_copy_format);
}

fn ui_previewer_options(ui: &mut Ui, size: Vec2, previewer_data: &mut PreviewerData) {
    let slider = egui::Slider::new(
        &mut previewer_data.quantize_num_levels,
        1..=previewer_data.control_points.len(),
    );
    let slider_size: Vec2 = Vec2::new(300.0, 25.0);
    let mut slider_button_rect: Rect = Rect::from_min_size(
        egui::Pos2 {
            x: ui.max_rect().min.x,
            y: ui.max_rect().min.y,
        },
        slider_size,
    );
    slider_button_rect = slider_button_rect.translate(Vec2::new(0.0, 25.0));

    if ui.put(slider_button_rect, slider).clicked() {
        previewer_data.reset_preview_sizes();
    }
}

pub fn ui_previewer(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
) -> Response {
    let previewer_rect = ui.available_rect_before_wrap();

    let inner_response = ui.vertical(|ui| {
        let control_points_response = ui_previewer_control_points_with_drag(
            ui,
            previewer_rect.size() * Vec2::new(1.0, 0.16),
            control_points,
            previewer_data,
            color_copy_format,
        );
        ui_previewer_curve(
            ui,
            previewer_rect.size() * Vec2::new(1.0, 0.25),
            control_points,
            spline_mode,
            previewer_data,
        );
        ui_previewer_curve_quantized(
            ui,
            previewer_rect.size() * Vec2::new(1.0, 0.25),
            control_points,
            spline_mode,
            previewer_data,
            color_copy_format,
            previewer_data.quantize_num_levels,
        );
        ui_previewer_options(
            ui,
            previewer_rect.size() * Vec2::new(1.0, 1.0),
            previewer_data,
        );

        let reset_button = egui::Button::new("❌").small().wrap(true).frame(true);
        let reset_button_size: Vec2 = Vec2::new(25.0, 25.0);
        let reset_button_rect: Rect = Rect {
            min: previewer_rect.min,
            max: previewer_rect.min + reset_button_size,
        };

        if ui.put(reset_button_rect, reset_button).clicked() {
            previewer_data.reset_preview_sizes();
        }

        control_points_response
    });

    inner_response.inner
}

const PREVIEWER_DEFAULT_VALUE: f32 = 100.0;
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

    pub fn sum(&self) -> f32 {
        self.points_preview_sizes.iter().sum()
    }
}

pub struct ZPreviewer {
    pub data: PreviewerData,
}

impl ZPreviewer {
    pub fn new() -> Self {
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

    pub fn draw_ui(&mut self, ui: &mut Ui, color_copy_format: ColorStringCopy) -> Response {
        let previewer_response = ui_previewer(
            ui,
            &self.data.control_points.clone(),
            self.data.spline_mode,
            &mut self.data,
            color_copy_format,
        );

        previewer_response
    }
}

impl Default for ZPreviewer {
    fn default() -> Self {
        Self::new()
    }
}
