use ecolor::{Color32, HsvaGamma};
use eframe::{
    egui::{self, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    emath::RectTransform,
    epaint::PathShape,
};
use splines::{Interpolation, Key, Spline};

use crate::{
    common::SplineMode,
    datatypes::control_point::{ControlPoint, ControlPointValue},
    ui_egui::tangents::ui_control_point_tangents,
};

const CONTROL_POINT_RADIUS: f32 = 8.0;
const FILL_RADIUS_SCALE: f32 = 1.8;
pub const CONTROL_POINT_FILL_RADIUS: f32 = FILL_RADIUS_SCALE * CONTROL_POINT_RADIUS;

#[derive(Default)]
pub struct ControlPointUiResult {
    pub dragged_point: Option<egui::Response>,
    pub selected_index: Option<usize>,
    pub hovering_control_point: Option<(egui::Response, usize)>,
    pub selected_tangent: Option<usize>,
    pub dragged_tangent: Option<egui::Response>,
    pub dragged_tangent_offset: Option<Vec2>,
}

pub fn control_point_pos(cp: &ControlPoint) -> Pos2 {
    Pos2::new(
        cp.val()[0].clamp(0.0, 1.0),
        1.0 - cp.val()[1].clamp(0.0, 1.0),
    )
}

fn to_screen_pos(to_screen: &RectTransform, cp: &ControlPoint) -> Pos2 {
    to_screen.transform_pos(control_point_pos(cp))
}

pub fn ui_ordered_control_points(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    marked_control_point_index: Option<usize>,
    _is_middle_interpolated: bool,
    parent_response: &egui::Response,
    show_bezier_tangents: bool,
) -> ControlPointUiResult {
    use egui::PointerButton::Primary;

    const SHOW_LINEAR_LINE: bool = false;

    if control_points.is_empty() {
        return ControlPointUiResult::default();
    }

    let to_screen = RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0)),
        parent_response.rect,
    );

    let control_point_radius = CONTROL_POINT_RADIUS;
    let control_point_draw_size = Vec2::splat(2.0 * control_point_radius);

    let inactive_stroke = ui.style().noninteractive().fg_stroke;
    let active_stroke = ui.style().interact(parent_response).fg_stroke;

    let mut selected_index = marked_control_point_index;
    let mut tangent_selected_index = None;
    let mut hovering_control_point = None;
    let mut dragged_point_response = None;
    let mut dragged_tangent_response = None;
    let mut dragged_tangent_offset = None;

    let mut tangent_shapes = Vec::new();
    let mut tangent_paths = Vec::new();
    if show_bezier_tangents {
        for (i, cp) in control_points.iter().enumerate() {
            let result = ui_control_point_tangents(
                ui,
                i,
                cp,
                i == 0,
                i == control_points.len() - 1,
                marked_control_point_index == Some(i),
                &to_screen,
                parent_response,
                control_point_draw_size,
                control_point_radius,
                inactive_stroke,
                &mut tangent_shapes,
                &mut tangent_paths,
            );

            if dragged_tangent_response.is_none() {
                dragged_tangent_response = result.dragged_tangent;
                tangent_selected_index = result.selected_tangent;
                dragged_tangent_offset = result.dragged_tangent_offset;
            }

            if result.selected_by_tangent {
                selected_index = Some(i);
            }
        }
    }

    let control_point_shapes_fill: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let point_in_screen = to_screen_pos(&to_screen, cp);

            let rect = Rect::from_center_size(point_in_screen, control_point_draw_size);
            let response = ui.interact(rect, parent_response.id.with(i), Sense::click_and_drag());

            if dragged_point_response.is_none()
                && (response.dragged_by(Primary) || response.clicked_by(Primary))
            {
                selected_index = Some(i);
                dragged_point_response = Some(response.clone());
            }

            if hovering_control_point.is_none() && response.hovered() {
                hovering_control_point = Some((response, i));
            }

            let color = HsvaGamma {
                h: cp.val()[2],
                s: cp.val()[0],
                v: cp.val()[1],
                a: 1.0,
            };

            Shape::circle_filled(
                point_in_screen,
                FILL_RADIUS_SCALE * control_point_radius,
                color,
            )
        })
        .collect();

    let control_point_shapes: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let point = to_screen_pos(&to_screen, cp);

            if i == 0 || i == control_points.len() - 1 {
                Shape::rect_stroke(
                    Rect::from_center_size(point, Vec2::splat(control_point_radius)),
                    0.0,
                    active_stroke,
                    egui::StrokeKind::Middle,
                )
            } else {
                Shape::circle_stroke(point, control_point_radius, active_stroke)
            }
        })
        .collect();

    if SHOW_LINEAR_LINE {
        let points: Vec<Pos2> = control_points
            .iter()
            .map(|cp| to_screen_pos(&to_screen, cp))
            .collect();

        ui.painter().add(PathShape::line(
            points,
            Stroke::new(1.0, Color32::RED.linear_multiply(0.25)),
        ));
    }

    ui.painter().extend(control_point_shapes_fill);
    ui.painter().extend(control_point_shapes);
    ui.painter().extend(tangent_shapes);
    ui.painter()
        .extend(tangent_paths.into_iter().map(Into::into));

    if let Some(marked) = marked_control_point_index {
        let point = to_screen_pos(&to_screen, &control_points[marked]);
        ui.painter().add(Shape::rect_stroke(
            Rect::from_center_size(point, Vec2::splat(control_point_radius * 0.5)),
            0.0,
            active_stroke,
            egui::StrokeKind::Middle,
        ));
    }

    ControlPointUiResult {
        dragged_point: dragged_point_response,
        selected_index,
        hovering_control_point,
        selected_tangent: tangent_selected_index,
        dragged_tangent: dragged_tangent_response,
        dragged_tangent_offset,
    }
}

pub fn flatten_control_points(control_points: &[ControlPoint]) -> Vec<ControlPoint> {
    let mut flattened: Vec<ControlPoint> = Vec::with_capacity(control_points.len());

    for (i, cp) in control_points.iter().enumerate() {
        if i == 0 {
            flattened.push(cp.clone());
            continue;
        }

        let prev_hue = flattened[i - 1].val()[2];
        let hue_diff = cp.val()[2] - prev_hue;

        let cp_clone = cp.clone();

        if hue_diff.abs() > 0.5 {
            // Adjust all previous hues by ±1 to smooth wraparound
            let adjustment = if hue_diff > 0.0 { 1.0 } else { -1.0 };
            for prev_cp in &mut flattened {
                prev_cp.val_mut()[2] += adjustment;
            }
        }

        flattened.push(cp_clone);
    }

    flattened
}

pub fn find_spline_max_t(spline: &Spline<f32, ControlPointValue>) -> f32 {
    // Spline::from_vec sorts keys by t.
    spline.keys().last().map_or(0.0, |k| k.t)
}

/// Samples `spline` (built by `control_points_to_spline`) at `x` in [0, 1] across the real control points.
pub fn sample_spline_normalized(
    spline: &Spline<f32, ControlPointValue>,
    spline_mode: SplineMode,
    x: f32,
) -> Option<ControlPointValue> {
    match spline_mode {
        SplineMode::HermiteBezier => {
            // Real keys are at 1..=n, with phantom endpoint duplicates at 0 and n + 1.
            let n = spline.len().checked_sub(2)?;
            if n == 0 {
                return None;
            }
            let t = 1.0 + x * (n - 1) as f32;
            // CatmullRom needs keys[i + 2], so the sampler returns None exactly at the last real key.
            spline
                .clamped_sample(t)
                .or_else(|| (t >= n as f32).then(|| spline.keys()[n].value))
        }
        _ => {
            let first = spline.keys().first()?.t;
            let last = spline.keys().last()?.t;
            spline.clamped_sample(first + x * (last - first))
        }
    }
}

pub fn control_points_to_spline(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> Spline<f32, ControlPointValue> {
    match spline_mode {
        SplineMode::Linear => Spline::from_vec(
            control_points
                .iter()
                .enumerate()
                .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::Linear))
                .collect(),
        ),
        SplineMode::Bezier => Spline::from_vec(
            control_points
                .iter()
                .enumerate()
                .map(|(index, e)| {
                    Key::new(
                        index as f32,
                        *e.val(),
                        Interpolation::StrokeBezier(
                            *control_points[index].val()
                                + control_points[index].tangents()[0].unwrap_or_default(),
                            *control_points[index].val()
                                + control_points[index].tangents()[1].unwrap_or_default(),
                        ),
                    )
                })
                .collect(),
        ),
        SplineMode::HermiteBezier => {
            let mut catmul_rom_spline_vec = control_points.to_vec();
            if control_points.len() >= 1 {
                catmul_rom_spline_vec.insert(0, control_points.first().unwrap().clone());
            }
            if control_points.len() >= 1 {
                catmul_rom_spline_vec.push(control_points.last().unwrap().clone());
            }

            let new_spline = Spline::from_vec(
                catmul_rom_spline_vec
                    .iter()
                    .enumerate()
                    .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::CatmullRom))
                    .collect(),
            );

            new_spline
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.2), 0.5),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.2), 1.0),
        ]
    }

    #[test]
    fn hermite_sample_at_end_returns_last_point() {
        let cps = three_points();
        let spline = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(
            sample_spline_normalized(&spline, SplineMode::HermiteBezier, 1.0),
            Some(*cps[2].val())
        );
    }

    #[test]
    fn hermite_sample_at_start_returns_first_point() {
        let cps = three_points();
        let spline = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(
            sample_spline_normalized(&spline, SplineMode::HermiteBezier, 0.0),
            Some(*cps[0].val())
        );
    }

    #[test]
    fn linear_sample_covers_first_segment() {
        let cps = three_points();
        let spline = control_points_to_spline(&cps, SplineMode::Linear);
        assert_eq!(
            sample_spline_normalized(&spline, SplineMode::Linear, 0.0),
            Some(*cps[0].val())
        );
        let mid = sample_spline_normalized(&spline, SplineMode::Linear, 0.25).unwrap();
        assert!((mid[0] - 0.25).abs() < 1e-6 && (mid[1] - 0.25).abs() < 1e-6);
    }

    const SAMPLED_MODES: [SplineMode; 3] = [
        SplineMode::Linear,
        SplineMode::Bezier,
        SplineMode::HermiteBezier,
    ];

    fn points_with_hues(hues: &[f32]) -> Vec<ControlPoint> {
        hues.iter()
            .enumerate()
            .map(|(i, h)| ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, *h), i as f32))
            .collect()
    }

    #[test]
    fn flatten_control_points_unwraps_hue_across_boundary() {
        let hues = [0.9, 0.1, 0.3];
        let flattened = flatten_control_points(&points_with_hues(&hues));
        assert_eq!(flattened.len(), hues.len());
        for pair in flattened.windows(2) {
            assert!((pair[1].val()[2] - pair[0].val()[2]).abs() <= 0.5);
        }
        for (cp, hue) in flattened.iter().zip(hues) {
            assert!((cp.val()[2].rem_euclid(1.0) - hue).abs() < 1e-5);
        }
    }

    #[test]
    fn flatten_control_points_keeps_close_hues() {
        let cps = points_with_hues(&[0.2, 0.4]);
        let flattened = flatten_control_points(&cps);
        assert_eq!(flattened.len(), cps.len());
        for (flat, cp) in flattened.iter().zip(&cps) {
            assert_eq!(flat.val(), cp.val());
        }
    }

    #[test]
    fn sample_without_control_points_is_none() {
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&[], mode);
            for x in [0.0, 0.5, 1.0] {
                assert_eq!(
                    sample_spline_normalized(&spline, mode, x),
                    None,
                    "{mode:?} at {x}"
                );
            }
        }
    }

    #[test]
    fn sample_single_control_point_returns_it() {
        let cps = [ControlPoint::new_simple(
            ControlPointValue::new(0.3, 0.6, 0.7),
            0.0,
        )];
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&cps, mode);
            for x in [0.0, 0.5, 1.0] {
                assert_eq!(
                    sample_spline_normalized(&spline, mode, x),
                    Some(*cps[0].val()),
                    "{mode:?} at {x}"
                );
            }
        }
    }

    #[test]
    fn sample_two_control_points_hits_endpoints() {
        let cps = [
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.2, 0.4), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 0.8, 0.4), 1.0),
        ];
        for mode in [SplineMode::Linear, SplineMode::Bezier] {
            let spline = control_points_to_spline(&cps, mode);
            assert_eq!(
                sample_spline_normalized(&spline, mode, 0.0),
                Some(*cps[0].val()),
                "{mode:?}"
            );
            assert_eq!(
                sample_spline_normalized(&spline, mode, 1.0),
                Some(*cps[1].val()),
                "{mode:?}"
            );
        }
        let spline = control_points_to_spline(&cps, SplineMode::Linear);
        let mid = sample_spline_normalized(&spline, SplineMode::Linear, 0.5).unwrap();
        assert!((mid[0] - 0.5).abs() < 1e-6 && (mid[1] - 0.5).abs() < 1e-6);
    }

    fn with_key_t(
        spline: &Spline<f32, ControlPointValue>,
        key_t: &[f32],
    ) -> Spline<f32, ControlPointValue> {
        let mut keys = spline.keys().to_vec();
        for (key, t) in keys.iter_mut().zip(key_t) {
            key.t = *t;
        }
        Spline::from_vec(keys)
    }

    #[test]
    fn sample_with_preview_key_t_hits_every_key() {
        let cps = three_points();
        for mode in [SplineMode::Linear, SplineMode::Bezier] {
            for key_t in [[50.0, 150.0, 250.0], [50.0, 200.0, 350.0]] {
                let spline = with_key_t(&control_points_to_spline(&cps, mode), &key_t);
                for (x, cp) in [0.0, 0.5, 1.0].into_iter().zip(&cps) {
                    let sample = sample_spline_normalized(&spline, mode, x).unwrap();
                    for c in 0..3 {
                        assert!(
                            (sample[c] - cp.val()[c]).abs() < 1e-5,
                            "{mode:?} {key_t:?} at {x}: {sample:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn find_spline_max_t_is_last_key_t() {
        let cps = three_points();
        let linear = control_points_to_spline(&cps, SplineMode::Linear);
        assert_eq!(find_spline_max_t(&linear), 2.0);
        let hermite = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(find_spline_max_t(&hermite), 4.0);
        let empty = control_points_to_spline(&[], SplineMode::Linear);
        assert_eq!(find_spline_max_t(&empty), 0.0);
    }

    const PICKER_RECT: Rect = Rect {
        min: Pos2::new(100.0, 100.0),
        max: Pos2::new(300.0, 300.0),
    };
    const MIDDLE_CENTER: Pos2 = Pos2::new(200.0, 200.0);

    fn three_points_with_zero_middle_tangents() -> Vec<ControlPoint> {
        let mut cps = three_points();
        *cps[1].tangents_mut() = [Some(ControlPointValue::new(0.0, 0.0, 0.0)); 2];
        cps
    }

    fn run_frames(frames: Vec<Vec<egui::Event>>) -> ControlPointUiResult {
        let ctx = egui::Context::default();
        let points = three_points_with_zero_middle_tangents();
        let mut result = ControlPointUiResult::default();
        for events in std::iter::once(Vec::new()).chain(frames) {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let parent = ui.allocate_rect(PICKER_RECT, Sense::click_and_drag());
                    result = ui_ordered_control_points(ui, &points, Some(1), false, &parent, true);
                });
            });
        }
        result
    }

    fn button(pos: Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn primary_drag(start: Pos2, dx: f32) -> Vec<Vec<egui::Event>> {
        let mut frames = vec![
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, egui::PointerButton::Primary, true)],
        ];
        for step in 1..=3 {
            let x = dx * step as f32 / 3.0;
            frames.push(vec![egui::Event::PointerMoved(start + Vec2::new(x, 0.0))]);
        }
        frames
    }

    #[test]
    fn dragging_selected_bezier_point_center_drags_the_point() {
        let result = run_frames(primary_drag(MIDDLE_CENTER, 20.0));
        assert!(result.dragged_point.is_some());
        assert!(result.dragged_tangent.is_none());
    }

    #[test]
    fn zero_bezier_tangent_is_grabbed_at_min_handle_length() {
        let drawn_handle = MIDDLE_CENTER - Vec2::new(2.0 * CONTROL_POINT_RADIUS, 0.0);
        let result = run_frames(primary_drag(drawn_handle, -21.0));
        assert!(result.dragged_point.is_none());
        assert!(result.dragged_tangent.is_some());
        assert_eq!(result.selected_tangent, Some(0));
        let offset = result.dragged_tangent_offset.unwrap();
        let min_length_sv = 2.0 * CONTROL_POINT_RADIUS / PICKER_RECT.width();
        assert!(
            offset.x < -min_length_sv && offset.y.abs() < 1e-6,
            "{offset:?}"
        );
    }

    #[test]
    fn right_clicking_selected_bezier_point_clicks_the_point() {
        let result = run_frames(vec![
            vec![egui::Event::PointerMoved(MIDDLE_CENTER)],
            vec![button(MIDDLE_CENTER, egui::PointerButton::Secondary, true)],
            vec![button(MIDDLE_CENTER, egui::PointerButton::Secondary, false)],
        ]);
        let (response, index) = result.hovering_control_point.expect("point hovered");
        assert_eq!(index, 1);
        assert!(response.clicked_by(egui::PointerButton::Secondary));
    }
}
