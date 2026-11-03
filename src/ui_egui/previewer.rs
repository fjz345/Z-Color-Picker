use eframe::{
    egui::{self, lerp, CursorIcon, Layout, PointerButton, Response, Sense, TextStyle, Ui, Vec2},
    epaint::{Color32, Rect},
};
use serde::{Deserialize, Serialize};

use crate::{
    color_space::{BlendSpace, PickerSpace},
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::ControlPoint,
    ui_egui::{
        blend::Gradient,
        control_points::{drag_grab_offset_x, drag_t, key_ts, space_t_evenly, t_at_pointer},
        gradient::color_function_gradient,
        ui_common::{paint_swatch, swatch},
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
    colors: &[Color32],
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

    let num_control_points = colors.len();
    let size_per_color_x = ui_size.x / (num_control_points as f32);
    let size_per_color_y = ui_size.y;
    let hover_text =
        format!("{num_control_points} quantized colors; middle-click copies the strip as an image");

    for color in colors {
        let size_weight: f32 = 1.0;
        let response_button = swatch(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            *color,
        );
        on_hover_strip_hint(response_button, &hover_text);
    }

    response
}

const MIDDLE_KEY_BAND_HINT: &str =
    "Key color, nearest along the gradient. Drag: move its point's T; \
     middle-click copies the strip as an image";
const END_KEY_BAND_HINT: &str =
    "Key color of an end point, fixed at T 0 or 1; middle-click copies the strip as an image";

fn key_band_edges(control_points: &[ControlPoint]) -> Vec<f32> {
    let ts = key_ts(control_points);
    std::iter::once(0.0)
        .chain(ts.windows(2).map(|pair| 0.5 * (pair[0] + pair[1])))
        .chain(std::iter::once(1.0))
        .collect()
}

fn ui_previewer_control_points_with_drag(
    ui: &mut Ui,
    size: Vec2,
    control_points: &mut [ControlPoint],
    picker_space: PickerSpace,
) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let edges = key_band_edges(control_points);
    let count = control_points.len();

    for i in 0..count {
        let band = Rect::from_x_y_ranges(
            lerp(rect.x_range(), edges[i])..=lerp(rect.x_range(), edges[i + 1]),
            rect.y_range(),
        );
        paint_swatch(
            ui.painter(),
            band,
            control_points[i].val().color(picker_space),
        );

        let id = response.id.with(i);
        if i == 0 || i + 1 == count {
            on_hover_strip_hint(ui.interact(band, id, Sense::click()), END_KEY_BAND_HINT);
            continue;
        }
        let band_response = ui
            .interact(band, id, Sense::click_and_drag())
            .on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);
        let band_response = on_hover_strip_hint(band_response, MIDDLE_KEY_BAND_HINT);
        if band_response.dragged_by(PointerButton::Primary) {
            let key_x = lerp(rect.x_range(), *control_points[i].t());
            let grab_offset_x = drag_grab_offset_x(ui, &band_response, key_x);
            if let Some(pointer) = band_response.interact_pointer_pos() {
                let target = t_at_pointer(pointer.x, grab_offset_x, rect.left(), rect.width());
                drag_t(control_points, i, target);
            }
        }
    }

    response
}

fn ui_previewer_curve(ui: &mut Ui, size: Vec2, gradient: &Gradient) -> Response {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_curve = ui.new_child(
        egui::UiBuilder::default()
            .max_rect(rect)
            .layout(Layout::left_to_right(egui::Align::Min)),
    );
    previewer_ui_curve.spacing_mut().item_spacing = Vec2::ZERO;

    let response = color_function_gradient(&mut previewer_ui_curve, rect.size(), |x| {
        gradient.color_at(x).unwrap_or(Color32::TRANSPARENT)
    });

    on_hover_strip_hint(
        response,
        "Gradient along the curve; middle-click copies it as an image",
    )
}

fn ui_previewer_curve_quantized(
    ui: &mut Ui,
    size: Vec2,
    gradient: &Gradient,
    color_copy_format: ColorStringCopy,
    number_levels: usize,
) -> Response {
    let quantized_colors = quantized_colors(gradient, number_levels);

    let response = ui_previewer_colors(ui, size, &quantized_colors, color_copy_format);
    response
}

fn quantized_colors(gradient: &Gradient, levels: usize) -> Vec<Color32> {
    if gradient.key_ts().is_empty() {
        return Vec::new();
    }
    (0..levels)
        .map(|i| {
            let x = i as f32 / (levels - 1).max(1) as f32;
            gradient
                .color_at(x)
                .expect("a gradient with keys has a color everywhere")
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

fn ui_previewer_options(ui: &mut Ui, previewer_data: &mut PreviewerData) {
    ui.horizontal(|ui| {
        ui.label("Levels");
        ui.add(egui::Slider::new(
            &mut previewer_data.quantize_num_levels,
            MIN_QUANTIZE_LEVELS..=MAX_QUANTIZE_LEVELS,
        ))
        .on_hover_text("Number of quantized colors");
        if ui
            .button("Space evenly")
            .on_hover_text("Spread the points evenly along the gradient (equal T steps)")
            .clicked()
        {
            space_t_evenly(&mut previewer_data.control_points);
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
        if previewer_data.control_points.is_empty() {
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

        let picker_space = previewer_data.picker_space;
        let response_control_points = ui_previewer_control_points_with_drag(
            ui,
            Vec2::new(width, control_points_height),
            &mut previewer_data.control_points,
            picker_space,
        );
        let gradient = Gradient::new(
            &previewer_data.control_points,
            spline_mode,
            picker_space,
            previewer_data.blend_space,
        );
        let response_curve = ui_previewer_curve(ui, Vec2::new(width, curve_height), &gradient);
        let response_curve_quantized = ui_previewer_curve_quantized(
            ui,
            Vec2::new(width, quantized_height),
            &gradient,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewerData {
    pub control_points: Vec<ControlPoint>,
    pub spline_mode: SplineMode,
    #[serde(default)]
    pub picker_space: PickerSpace,
    #[serde(default)]
    pub blend_space: BlendSpace,
    pub quantize_num_levels: usize,
}

impl PreviewerData {
    pub fn new(num: usize) -> Self {
        Self {
            control_points: vec![ControlPoint::default(); num],
            spline_mode: SplineMode::HermiteBezier,
            picker_space: PickerSpace::default(),
            blend_space: BlendSpace::default(),
            quantize_num_levels: 4,
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

    pub fn update(
        &mut self,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
        picker_space: PickerSpace,
        blend_space: BlendSpace,
    ) {
        self.data.spline_mode = spline_mode;
        self.data.picker_space = picker_space;
        self.data.blend_space = blend_space;
        self.data.control_points.clear();
        self.data.control_points.extend_from_slice(control_points);
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
    use crate::datatypes::control_point::ControlPointValue;
    use crate::ui_egui::control_points::MIN_T_GAP;
    use splines::Interpolate;

    fn quantized(points: &[ControlPoint], mode: SplineMode, levels: usize) -> Vec<Color32> {
        let gradient = Gradient::new(points, mode, PickerSpace::Hsv, BlendSpace::Picker);
        quantized_colors(&gradient, levels)
    }

    fn color_of(value: ControlPointValue) -> Color32 {
        value.color(PickerSpace::Hsv)
    }

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

    fn two_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.1, 0.2, 0.3), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.9, 0.8, 0.3), 1.0),
        ]
    }

    #[test]
    fn quantized_colors_two_points_exceed_point_count() {
        let cps = two_points();
        let colors = quantized(&cps, SplineMode::Linear, 8);
        assert_eq!(colors.len(), 8);
        for (level, color) in colors.iter().enumerate() {
            let x = level as f32 / 7.0;
            let expected = ControlPointValue::lerp(x, *cps[0].val(), *cps[1].val());
            assert_eq!(*color, color_of(expected), "level {level}");
        }
    }

    #[test]
    fn quantized_colors_one_level_is_first_point_and_zero_is_empty() {
        let cps = two_points();
        assert_eq!(
            quantized(&cps, SplineMode::Linear, 1),
            [color_of(*cps[0].val())]
        );
        assert!(quantized(&cps, SplineMode::Linear, 0).is_empty());
    }

    fn sv_ramp_points(ts: [f32; 3]) -> Vec<ControlPoint> {
        [0.0, 0.5, 1.0]
            .into_iter()
            .zip(ts)
            .map(|(sv, t)| ControlPoint::new_simple(ControlPointValue::new(sv, sv, 0.2), t))
            .collect()
    }

    #[test]
    fn quantized_colors_levels_equal_to_points_hit_every_key() {
        let cps = sv_ramp_points([0.0, 0.5, 1.0]);
        let colors = quantized(&cps, SplineMode::Linear, 3);
        let expected: Vec<Color32> = cps.iter().map(|cp| color_of(*cp.val())).collect();
        assert_eq!(colors, expected);
    }

    #[test]
    fn quantized_colors_follow_t() {
        let cps = sv_ramp_points([0.0, 0.25, 1.0]);
        let colors = quantized(&cps, SplineMode::Linear, 3);
        let expected = color_of(ControlPointValue::new(2.0 / 3.0, 2.0 / 3.0, 0.2));
        let channel_gaps: Vec<i16> = (0..3)
            .map(|c| i16::from(colors[1][c]) - i16::from(expected[c]))
            .collect();
        assert!(
            channel_gaps.iter().all(|gap| gap.abs() <= 1),
            "{colors:?} vs {expected:?}"
        );
    }

    #[test]
    fn quantized_colors_hit_every_key_at_its_t() {
        let cps = sv_ramp_points([0.0, 1.0 / 3.0, 1.0]);
        for mode in SplineMode::ALL {
            let colors = quantized(&cps, mode, 4);
            for (level, key) in [(0, 0), (1, 1), (3, 2)] {
                assert_eq!(colors[level], color_of(*cps[key].val()), "{mode:?}");
            }
        }
    }

    #[test]
    fn key_band_edges_split_halfway_between_neighbouring_t() {
        let cps = sv_ramp_points([0.0, 0.4, 1.0]);
        assert_eq!(key_band_edges(&cps), [0.0, 0.2, 0.7, 1.0]);
        assert_eq!(key_band_edges(&cps[..1]), [0.0, 1.0]);
    }

    #[test]
    fn key_band_edges_of_invalid_t_match_the_evenly_keyed_gradient() {
        let cps = sv_ramp_points([0.0, 0.0, 0.0]);
        assert_eq!(key_band_edges(&cps), [0.0, 0.25, 0.75, 1.0]);
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
                let colors = quantized(&[], spline_mode, levels);
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

    fn drag_key_band(press_x: f32, dx: f32) -> Vec<ControlPoint> {
        let ctx = egui::Context::default();
        let mut data = PreviewerData {
            control_points: sv_ramp_points([0.0, 0.4, 1.0]),
            ..PreviewerData::new(0)
        };
        let (rects, _) = draw_previewer_pass(&ctx, &mut data, Vec::new());
        let strip = rects[0];
        let start = strip.lerp_inside(Vec2::new(press_x, 0.5));
        let end = start + Vec2::new(dx * strip.width(), 0.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        let mut frames = vec![
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, true)],
        ];
        for step in 1..=3 {
            frames.push(vec![egui::Event::PointerMoved(
                start.lerp(end, step as f32 / 3.0),
            )]);
        }
        frames.push(vec![button(end, false)]);
        for events in frames {
            draw_previewer_pass(&ctx, &mut data, events);
        }
        data.control_points
    }

    fn ts_of(points: &[ControlPoint]) -> Vec<f32> {
        points.iter().map(|cp| *cp.t()).collect()
    }

    #[test]
    fn dragging_a_middle_key_band_moves_its_t_by_the_pointer_distance() {
        let points = drag_key_band(0.45, 0.2);
        assert!((points[1].t() - 0.6).abs() < 1e-4, "{:?}", ts_of(&points));
        assert_eq!((*points[0].t(), *points[2].t()), (0.0, 1.0));
    }

    #[test]
    fn dragging_a_middle_key_band_stops_at_the_min_gap_to_its_neighbour() {
        let points = drag_key_band(0.45, 2.0);
        assert_eq!(*points[1].t(), 1.0 - MIN_T_GAP);
    }

    #[test]
    fn end_key_bands_keep_their_t_when_dragged() {
        for (press_x, dx) in [(0.1, 0.3), (0.9, -0.3)] {
            assert_eq!(ts_of(&drag_key_band(press_x, dx)), [0.0, 0.4, 1.0]);
        }
    }

    #[test]
    fn previewer_data_json_with_removed_band_widths_still_loads() {
        let json = r#"{
            "control_points": [],
            "spline_mode": "Linear",
            "points_preview_sizes": [100.0, 250.0],
            "quantize_num_levels": 7
        }"#;

        let data: PreviewerData = serde_json::from_str(json).unwrap();

        assert_eq!(data.quantize_num_levels, 7);
        assert_eq!(data.spline_mode, SplineMode::Linear);
        assert_eq!(data.picker_space, PickerSpace::Hsv);
        assert_eq!(data.blend_space, BlendSpace::Picker);
    }

    #[test]
    fn quantized_colors_survive_count_transitions() {
        let pool: Vec<ControlPoint> = (0..5)
            .map(|i| {
                let f = i as f32;
                ControlPoint::new_simple(
                    ControlPointValue::new(f * 0.2, 1.0 - f * 0.2, 0.1 * f),
                    0.0,
                )
            })
            .collect();
        for spline_mode in SplineMode::ALL {
            for from in 0..=5 {
                for to in 0..=5 {
                    let mut previewer = ZPreviewer::default();
                    for points in [&pool[..from], &pool[..to]] {
                        previewer.update(points, spline_mode, PickerSpace::Hsv, BlendSpace::Picker);
                    }
                    let colors = quantized(&previewer.data.control_points, spline_mode, 8);
                    assert_eq!(
                        colors.len(),
                        if to == 0 { 0 } else { 8 },
                        "{spline_mode:?} {from} -> {to}"
                    );
                }
            }
        }
    }
}
