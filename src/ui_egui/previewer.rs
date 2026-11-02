use ecolor::HsvaGamma;
use eframe::{
    egui::{self, CursorIcon, Layout, PointerButton, Response, Sense, TextStyle, Ui, Vec2},
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

fn on_hover_strip_hint(response: Response, text: &str) -> Response {
    // A middle press screenshots this pass, so a tooltip drawn now would be in the copied image.
    if response
        .ctx
        .input(|i| i.pointer.button_down(PointerButton::Middle))
    {
        return response;
    }
    response.on_hover_text(text)
}

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
    let hover_text =
        format!("{num_control_points} quantized colors; middle-click copies the strip as an image");

    for cp in control_points {
        let size_weight: f32 = 1.0;
        let response_button = color_button(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            cp.color(),
            true,
        );
        on_hover_strip_hint(response_button, &hover_text);
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

    for (i, cp) in control_points.iter().enumerate() {
        let size_weight: f32 = previewer_data.points_preview_sizes[i] * num_control_points as f32
            / previewer_sizes_sum;
        let response_button: Response = color_button(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            cp.val().color(),
            true,
        )
        .on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);
        let response_button = on_hover_strip_hint(
            response_button,
            "Key colors. Drag a band to resize it; middle-click copies the strip as an image",
        );

        if response_button.dragged_by(PointerButton::Primary) {
            resize_band(
                &mut previewer_data.points_preview_sizes,
                i,
                response_button.drag_delta().x,
                ui_size.x,
            );
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

    on_hover_strip_hint(
        response,
        "Gradient along the curve; middle-click copies it as an image",
    )
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
    if control_points.is_empty() {
        return Vec::new();
    }
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

const MIN_QUANTIZE_LEVELS: usize = 2;
const MAX_QUANTIZE_LEVELS: usize = 32;

fn quantize_levels(stored: usize) -> usize {
    stored.clamp(MIN_QUANTIZE_LEVELS, MAX_QUANTIZE_LEVELS)
}

fn resize_band(sizes: &mut [f32], index: usize, dx_px: f32, width_px: f32) {
    let n = sizes.len();
    if n <= 1 || width_px <= 0.0 {
        return;
    }
    let sum: f32 = sizes.iter().sum();
    let others_sum = sum - sizes[index];
    if sum <= 0.0 || others_sum <= 0.0 {
        return;
    }

    let to_px = |size: f32| size / sum * width_px;
    let px = to_px(sizes[index]);
    let min_px = width_px / n as f32 / 2.0;
    let narrowest_other_px = sizes
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != index)
        .map(|(_, &size)| to_px(size))
        .fold(f32::INFINITY, f32::min);
    let max_px = width_px - min_px * (width_px - px) / narrowest_other_px;

    let grow_limit = max_px.max(px);
    let shrink_limit = min_px.min(px);
    let new_px = (px + dx_px).min(grow_limit).max(shrink_limit);
    sizes[index] = new_px * others_sum / (width_px - new_px);
}

fn ui_previewer_options(ui: &mut Ui, previewer_data: &mut PreviewerData) {
    ui.horizontal(|ui| {
        ui.label("Levels");
        ui.add(egui::Slider::new(
            &mut previewer_data.quantize_num_levels,
            MIN_QUANTIZE_LEVELS..=MAX_QUANTIZE_LEVELS,
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

const EMPTY_PREVIEWER_HINT: &str = "Add control points to preview the gradient";

fn paint_empty_previewer_hint(ui: &Ui, rect: Rect) {
    let padding = 6.0;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    let text_color = ui.visuals().text_color();
    let galley = painter.layout(
        EMPTY_PREVIEWER_HINT.to_owned(),
        TextStyle::Body.resolve(ui.style()),
        text_color,
        rect.width() - 4.0 * padding,
    );
    let text_rect = Rect::from_center_size(rect.center(), galley.size());
    painter.galley(text_rect.min, galley, text_color);
}

pub fn ui_previewer(
    ui: &mut Ui,
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
) -> PreviewerUiResponses {
    previewer_data.quantize_num_levels = quantize_levels(previewer_data.quantize_num_levels);
    let previewer_rect = ui.available_rect_before_wrap();
    let gaps_after_strips = 3.0 * ui.spacing().item_spacing.y;
    let controls_height = ui.spacing().interact_size.y + gaps_after_strips;
    let [control_points_height, curve_height, quantized_height] =
        previewer_strip_heights(previewer_rect.height(), controls_height);
    let width = previewer_rect.width();

    let inner_response = ui.vertical(|ui| {
        let control_points = &previewer_data.control_points.clone();

        if control_points.is_empty() {
            let gaps_between_strips = 2.0 * ui.spacing().item_spacing.y;
            let strips_height =
                control_points_height + curve_height + quantized_height + gaps_between_strips;
            let rect = Rect::from_min_size(
                ui.available_rect_before_wrap().min,
                Vec2::new(width, strips_height),
            );
            ui.allocate_rect(rect, Sense::hover());
            paint_empty_previewer_hint(ui, rect);
            ui_previewer_options(ui, previewer_data);
            return PreviewerUiResponses::default();
        }

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

    fn band_widths(sizes: &[f32], width_px: f32) -> Vec<f32> {
        let sum: f32 = sizes.iter().sum();
        sizes.iter().map(|size| size / sum * width_px).collect()
    }

    const BAND_LAYOUTS: [(f32, usize); 4] = [(800.0, 4), (2000.0, 4), (600.0, 6), (1500.0, 2)];

    #[test]
    fn band_width_tracks_pointer() {
        for (width_px, n) in BAND_LAYOUTS {
            for index in 0..n {
                for dx in [10.0, -10.0] {
                    let mut sizes = vec![PREVIEWER_DEFAULT_VALUE; n];
                    let before = band_widths(&sizes, width_px)[index];
                    resize_band(&mut sizes, index, dx, width_px);
                    let after = band_widths(&sizes, width_px)[index];
                    assert!(
                        (after - before - dx).abs() < 1e-3,
                        "W {width_px} n {n} band {index} dx {dx}: {before} -> {after}"
                    );
                }
            }
        }
    }

    #[test]
    fn band_never_narrower_than_half_share() {
        for (width_px, n) in BAND_LAYOUTS {
            let min_px = width_px / n as f32 / 2.0;
            for dx in [1e4, -1e4] {
                let mut sizes = vec![PREVIEWER_DEFAULT_VALUE; n];
                for index in 0..n {
                    resize_band(&mut sizes, index, dx, width_px);
                    let widths = band_widths(&sizes, width_px);
                    assert!(
                        widths.iter().all(|&w| w >= min_px - 1e-3),
                        "W {width_px} n {n} band {index} dx {dx}: {widths:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn single_band_zero_sum_zero_width_are_noops() {
        let mut single = vec![PREVIEWER_DEFAULT_VALUE];
        resize_band(&mut single, 0, 10.0, 800.0);
        assert_eq!(single, [PREVIEWER_DEFAULT_VALUE]);

        let mut zero_sum = vec![0.0; 3];
        resize_band(&mut zero_sum, 1, 10.0, 800.0);
        assert_eq!(zero_sum, [0.0; 3]);

        let mut zero_width = vec![PREVIEWER_DEFAULT_VALUE; 3];
        resize_band(&mut zero_width, 1, 10.0, 0.0);
        assert_eq!(zero_width, [PREVIEWER_DEFAULT_VALUE; 3]);
    }

    #[test]
    fn other_bands_keep_their_ratio() {
        let width_px = 1000.0;
        let mut sizes = vec![120.0, 80.0, 200.0, 100.0];
        let before = band_widths(&sizes, width_px);
        resize_band(&mut sizes, 1, 15.0, width_px);
        let after = band_widths(&sizes, width_px);
        let scale = (width_px - after[1]) / (width_px - before[1]);
        for j in [0, 2, 3] {
            assert!(
                (after[j] - before[j] * scale).abs() < 1e-3,
                "{before:?} -> {after:?}"
            );
        }
    }

    #[test]
    fn bands_already_past_the_bounds_do_not_jump() {
        let width_px = 1000.0;
        let mut sizes = vec![300.0, 150.0, 40.0];
        let before = band_widths(&sizes, width_px);
        let assert_unchanged = |sizes: &[f32]| {
            let after = band_widths(sizes, width_px);
            assert!(
                after.iter().zip(&before).all(|(a, b)| (a - b).abs() < 1e-3),
                "{before:?} -> {after:?}"
            );
        };
        resize_band(&mut sizes, 0, 1.0, width_px);
        assert_unchanged(&sizes);

        resize_band(&mut sizes, 2, -1.0, width_px);
        assert_unchanged(&sizes);

        resize_band(&mut sizes, 2, 1.0, width_px);
        let grown = band_widths(&sizes, width_px)[2];
        assert!((grown - before[2] - 1.0).abs() < 1e-3, "{grown}");
    }

    #[test]
    fn quantize_levels_clamps_to_slider_range() {
        assert_eq!(quantize_levels(0), MIN_QUANTIZE_LEVELS);
        assert_eq!(quantize_levels(1), 2);
        assert_eq!(quantize_levels(5), 5);
        assert_eq!(quantize_levels(99), MAX_QUANTIZE_LEVELS);
        assert_eq!(MAX_QUANTIZE_LEVELS, 32);
    }

    #[test]
    fn quantized_colors_without_points_is_empty() {
        for spline_mode in [
            SplineMode::Linear,
            SplineMode::Bezier,
            SplineMode::HermiteBezier,
        ] {
            for levels in [2, 32] {
                let colors = quantized_colors(&[], spline_mode, &PreviewerData::new(0), levels);
                assert!(colors.is_empty(), "{spline_mode:?} {levels}: {colors:?}");
            }
        }
    }

    fn draw_previewer_pass(
        ctx: &egui::Context,
        previewer_data: &mut PreviewerData,
        events: Vec<egui::Event>,
    ) -> (Vec<Rect>, Vec<String>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let mut rects = Vec::new();
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rects = ui_previewer(
                    ui,
                    SplineMode::Linear,
                    previewer_data,
                    ColorStringCopy::HEXNOA,
                )
                .get_rects();
            });
        });
        let painted_texts = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        (rects, painted_texts)
    }

    fn draw_previewer(previewer_data: &mut PreviewerData) -> (bool, usize) {
        let (rects, painted_texts) =
            draw_previewer_pass(&egui::Context::default(), previewer_data, Vec::new());
        let paints_hint = painted_texts
            .iter()
            .any(|text| text == EMPTY_PREVIEWER_HINT);
        (paints_hint, rects.len())
    }

    #[test]
    fn empty_previewer_paints_hint_and_no_copy_rects() {
        assert_eq!(draw_previewer(&mut PreviewerData::new(0)), (true, 0));
        assert_eq!(draw_previewer(&mut PreviewerData::new(2)), (false, 3));
    }

    fn mentions_middle_click(painted_texts: &[String]) -> bool {
        painted_texts
            .iter()
            .any(|text| text.contains("middle-click"))
    }

    #[test]
    fn middle_press_hides_strip_copy_tooltips() {
        for strip in 0..3 {
            let ctx = egui::Context::default();
            ctx.style_mut(|style| {
                style.interaction.tooltip_delay = 0.0;
                style.interaction.show_tooltips_only_when_still = false;
            });
            let mut previewer_data = PreviewerData::new(2);
            let (rects, _) = draw_previewer_pass(&ctx, &mut previewer_data, Vec::new());
            let pos = rects[strip].lerp_inside(Vec2::new(0.3, 0.5));
            draw_previewer_pass(
                &ctx,
                &mut previewer_data,
                vec![egui::Event::PointerMoved(pos)],
            );
            let mut hovered_texts = Vec::new();
            for _ in 0..3 {
                (_, hovered_texts) = draw_previewer_pass(&ctx, &mut previewer_data, Vec::new());
            }
            assert!(
                mentions_middle_click(&hovered_texts),
                "strip {strip}: {hovered_texts:?}"
            );

            let middle_press = egui::Event::PointerButton {
                pos,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            };
            let (_, pressed_texts) =
                draw_previewer_pass(&ctx, &mut previewer_data, vec![middle_press]);
            assert!(
                !mentions_middle_click(&pressed_texts),
                "strip {strip}: {pressed_texts:?}"
            );
        }
    }
}
