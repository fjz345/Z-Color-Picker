use ecolor::Color32;
use eframe::{
    egui::{self, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    emath::RectTransform,
    epaint::PathShape,
};
use splines::{Interpolation, Key, Spline};

use crate::{
    common::SplineMode,
    datatypes::control_point::{ControlPoint, ControlPointValue},
    math::shortest_hue_delta,
    ui_egui::{tangents::ui_control_point_tangents, ui_common::contrast_color},
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
    value_pos(cp.val())
}

pub fn value_pos(val: &ControlPointValue) -> Pos2 {
    Pos2::new(val[0].clamp(0.0, 1.0), 1.0 - val[1].clamp(0.0, 1.0))
}

fn to_screen_pos(to_screen: &RectTransform, cp: &ControlPoint) -> Pos2 {
    to_screen.transform_pos(control_point_pos(cp))
}

fn control_point_outline(fill: Color32, emphasized: bool) -> Stroke {
    let width = if emphasized { 2.5 } else { 1.5 };
    Stroke::new(width, contrast_color(fill))
}

fn control_point_hint(index: usize, count: usize) -> String {
    format!(
        "Point {} of {count}. Drag: move. Right-click: remove",
        index + 1
    )
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
            let response = ui
                .interact(rect, parent_response.id.with(i), Sense::click_and_drag())
                .on_hover_and_drag_cursor(CursorIcon::Grab)
                .on_hover_text(control_point_hint(i, control_points.len()));
            if response.dragged_by(Primary) {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }

            if dragged_point_response.is_none()
                && (response.dragged_by(Primary) || response.clicked_by(Primary))
            {
                selected_index = Some(i);
                dragged_point_response = Some(response.clone());
            }

            if hovering_control_point.is_none() && response.hovered() {
                hovering_control_point = Some((response, i));
            }

            Shape::circle_filled(
                point_in_screen,
                FILL_RADIUS_SCALE * control_point_radius,
                cp.val().color(),
            )
        })
        .collect();

    let hovered_index = hovering_control_point.as_ref().map(|(_, i)| *i);
    let control_point_shapes: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let point = to_screen_pos(&to_screen, cp);
            let emphasized = hovered_index == Some(i) || marked_control_point_index == Some(i);
            let outline = control_point_outline(cp.val().color(), emphasized);

            if i == 0 || i == control_points.len() - 1 {
                Shape::rect_stroke(
                    Rect::from_center_size(point, Vec2::splat(control_point_radius)),
                    0.0,
                    outline,
                    egui::StrokeKind::Middle,
                )
            } else {
                Shape::circle_stroke(point, control_point_radius, outline)
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
    ui.painter()
        .extend(tangent_paths.into_iter().map(Into::into));
    ui.painter().extend(tangent_shapes);

    if let Some(marked) = marked_control_point_index {
        let marked_point = &control_points[marked];
        let point = to_screen_pos(&to_screen, marked_point);
        ui.painter().add(Shape::rect_stroke(
            Rect::from_center_size(point, Vec2::splat(control_point_radius * 0.5)),
            0.0,
            control_point_outline(marked_point.val().color(), true),
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
    let mut flattened = control_points.to_vec();
    for i in 1..flattened.len() {
        let prev_raw_hue = control_points[i - 1].val()[2];
        let raw_hue = control_points[i].val()[2];
        let target = flattened[i - 1].val()[2] + shortest_hue_delta(prev_raw_hue, raw_hue);
        flattened[i].val_mut()[2] = raw_hue + (target - raw_hue).round();
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
    let x = x.clamp(0.0, 1.0);
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

pub fn bezier_handle(cp: &ControlPoint, slot: usize) -> ControlPointValue {
    let mut handle = *cp.val() + cp.tangents()[slot].unwrap_or_default();
    handle[0] = handle[0].clamp(0.0, 1.0);
    handle[1] = handle[1].clamp(0.0, 1.0);
    handle
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
                        Interpolation::StrokeBezier(bezier_handle(e, 0), bezier_handle(e, 1)),
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
    use crate::math::hue_abs_distance;

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
    fn flatten_control_points_unwraps_any_hue_sequence() {
        let sequences: [&[f32]; 4] = [
            &[2.3, 0.2, 0.4],
            &[-1.4, 0.3],
            &[0.9, 0.1, 0.9, 0.1],
            &[0.0, 0.4, 0.8, 0.2, 0.6, 0.0],
        ];
        for hues in sequences {
            let flattened = flatten_control_points(&points_with_hues(hues));
            assert_eq!(flattened.len(), hues.len());
            for pair in flattened.windows(2) {
                let step = pair[1].val()[2] - pair[0].val()[2];
                assert!(step.abs() <= 0.5 + 1e-5, "{hues:?}: step {step}");
            }
            for (cp, hue) in flattened.iter().zip(hues) {
                let flat_hue = cp.val()[2];
                assert!(
                    hue_abs_distance(flat_hue, *hue) < 1e-5,
                    "{hues:?}: {flat_hue} vs {hue}"
                );
            }
        }
    }

    #[test]
    fn two_point_hue_route_is_mode_independent() {
        let sample_hue = |mode, from: f32, to: f32, x: f32| {
            let flattened = flatten_control_points(&points_with_hues(&[from, to]));
            let spline = control_points_to_spline(&flattened, mode);
            sample_spline_normalized(&spline, mode, x).unwrap()[2]
        };
        let grid: Vec<f32> = (0..20).map(|i| i as f32 / 20.0).collect();
        for &a in &grid {
            for &b in &grid {
                let arc = shortest_hue_delta(a, b);
                let linear_mid = sample_hue(SplineMode::Linear, a, b, 0.5);
                for mode in SAMPLED_MODES {
                    for x in [0.25, 0.5, 0.75] {
                        let hue = sample_hue(mode, a, b, x);
                        let progress = shortest_hue_delta(a, hue) * arc.signum();
                        assert!(
                            (-1e-4..=arc.abs() + 1e-4).contains(&progress),
                            "{mode:?} ({a}, {b}) at {x}: {hue} is off the arc {arc}"
                        );
                        let reversed = sample_hue(mode, b, a, 1.0 - x);
                        assert!(
                            hue_abs_distance(hue, reversed) < 1e-4,
                            "{mode:?} ({a}, {b}) at {x}: {hue} vs reversed {reversed}"
                        );
                    }
                    let mid = sample_hue(mode, a, b, 0.5);
                    assert!(
                        hue_abs_distance(mid, linear_mid) < 1e-4,
                        "{mode:?} ({a}, {b}): mid {mid} vs linear {linear_mid}"
                    );
                }
            }
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

    fn assert_sample_near(
        sample: Option<ControlPointValue>,
        expected: ControlPointValue,
        context: &str,
    ) {
        let sample = sample.unwrap_or_else(|| panic!("{context}: no sample"));
        for c in 0..3 {
            assert!(
                (sample[c] - expected[c]).abs() < 1e-5,
                "{context}: {sample:?} vs {expected:?}"
            );
        }
    }

    fn distinct_points(n: usize) -> Vec<ControlPoint> {
        (0..n)
            .map(|i| {
                let i = i as f32;
                let val =
                    ControlPointValue::new((0.1 + 0.37 * i) % 1.0, (0.8 + 0.61 * i) % 1.0, 0.4);
                ControlPoint::new_simple(val, i)
            })
            .collect()
    }

    #[test]
    fn bezier_segment_uses_outgoing_slot_of_start_and_incoming_slot_of_end() {
        let cps = [
            point_with_tangent([0.0, 0.0, 0.0], 1, [0.3, 0.0, 0.0]),
            point_with_tangent([1.0, 1.0, 0.0], 0, [0.0, -0.2, 0.0]),
        ];
        let spline = control_points_to_spline(&cps, SplineMode::Bezier);
        assert_sample_near(
            sample_spline_normalized(&spline, SplineMode::Bezier, 0.5),
            ControlPointValue::new(0.6125, 0.425, 0.0),
            "bezier midpoint",
        );
    }

    #[test]
    fn sample_hits_every_key() {
        for n in [3, 4, 5, 7] {
            let cps = distinct_points(n);
            for mode in SAMPLED_MODES {
                let spline = control_points_to_spline(&cps, mode);
                for (i, cp) in cps.iter().enumerate() {
                    let x = i as f32 / (n - 1) as f32;
                    assert_sample_near(
                        sample_spline_normalized(&spline, mode, x),
                        *cp.val(),
                        &format!("{mode:?} n={n} key {i}"),
                    );
                }
            }
        }
    }

    #[test]
    fn sample_out_of_range_x_clamps_to_end_keys() {
        for n in [2, 3] {
            let cps = distinct_points(n);
            for mode in SAMPLED_MODES {
                let spline = control_points_to_spline(&cps, mode);
                for (x, key) in [(-1.0, 0), (-0.25, 0), (1.25, n - 1), (1.5, n - 1)] {
                    assert_sample_near(
                        sample_spline_normalized(&spline, mode, x),
                        *cps[key].val(),
                        &format!("{mode:?} n={n} at {x}"),
                    );
                }
            }
        }
    }

    #[test]
    fn hue_is_continuous_across_wrap_for_three_keys() {
        let flattened = flatten_control_points(&points_with_hues(&[0.9, 0.1, 0.3]));
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&flattened, mode);
            let hues: Vec<f32> = (0..=200)
                .map(|i| sample_spline_normalized(&spline, mode, i as f32 / 200.0).unwrap()[2])
                .collect();
            for pair in hues.windows(2) {
                assert!(
                    hue_abs_distance(pair[0], pair[1]) < 0.01,
                    "{mode:?}: jump {} -> {}",
                    pair[0],
                    pair[1]
                );
            }
            for hue in hues {
                assert!(
                    hue_abs_distance(hue, 0.1) <= 0.2 + 1e-4,
                    "{mode:?}: {hue} is off the arc 0.9 -> 0.3"
                );
            }
        }
    }

    #[test]
    fn hermite_two_point_gradient_eases_at_the_ends() {
        let two = [
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.2), 1.0),
        ];
        let three = three_points();
        for (cps, x, sv) in [
            (&two[..], 0.25, 0.203125),
            (&two[..], 0.5, 0.5),
            (&two[..], 0.75, 0.796875),
            (&three[..], 0.25, 0.21875),
        ] {
            let spline = control_points_to_spline(cps, SplineMode::HermiteBezier);
            assert_sample_near(
                sample_spline_normalized(&spline, SplineMode::HermiteBezier, x),
                ControlPointValue::new(sv, sv, 0.2),
                &format!("{} points at {x}", cps.len()),
            );
        }
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
        run_frames_with_output(frames).0
    }

    fn run_frames_with_output(
        frames: Vec<Vec<egui::Event>>,
    ) -> (ControlPointUiResult, egui::FullOutput) {
        let ctx = egui::Context::default();
        let points = three_points_with_zero_middle_tangents();
        let mut result = ControlPointUiResult::default();
        let mut output = egui::FullOutput::default();
        for events in std::iter::once(Vec::new()).chain(frames) {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            };
            output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let parent = ui.allocate_rect(PICKER_RECT, Sense::click_and_drag());
                    result = ui_ordered_control_points(ui, &points, Some(1), false, &parent, true);
                });
            });
        }
        (result, output)
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

    fn point_with_tangent(val: [f32; 3], slot: usize, offset: [f32; 3]) -> ControlPoint {
        let mut cp = ControlPoint::new_simple(val.into(), 0.0);
        cp.tangents_mut()[slot] = Some(offset.into());
        cp
    }

    const RUNAWAY_POINT: [f32; 3] = [0.035, 0.855, 0.566];
    const RUNAWAY_TANGENT: [f32; 3] = [161.96, 35.47, 0.0];

    #[test]
    fn bezier_handle_clamps_runaway_tangent_to_square() {
        let cp = point_with_tangent(RUNAWAY_POINT, 1, RUNAWAY_TANGENT);
        assert_eq!(
            bezier_handle(&cp, 1),
            ControlPointValue::new(1.0, 1.0, 0.566)
        );
    }

    #[test]
    fn bezier_handle_in_range_is_point_plus_offset() {
        let offset = [0.1, -0.2, 0.05];
        let cp = point_with_tangent([0.4, 0.5, 0.2], 0, offset);
        assert_eq!(
            bezier_handle(&cp, 0),
            *cp.val() + ControlPointValue::from(offset)
        );
    }

    #[test]
    fn bezier_handle_clamps_negative_overshoot_to_zero() {
        let cp = point_with_tangent([0.2, 0.3, 0.5], 0, [-0.5, -0.9, 0.0]);
        assert_eq!(bezier_handle(&cp, 0), ControlPointValue::new(0.0, 0.0, 0.5));
    }

    #[test]
    fn bezier_handle_keeps_unwrapped_hue() {
        let cp = point_with_tangent([0.5, 0.5, 0.9], 1, [0.0, 0.0, 0.3]);
        let hue = bezier_handle(&cp, 1)[2];
        assert!((hue - 1.2).abs() < 1e-6, "{hue}");
    }

    #[test]
    fn bezier_spline_with_runaway_tangent_stays_in_square() {
        let cps = [
            point_with_tangent(RUNAWAY_POINT, 1, RUNAWAY_TANGENT),
            point_with_tangent([0.775, 0.592, 0.604], 0, [-0.175, 0.315, 0.0]),
        ];
        let spline = control_points_to_spline(&cps, SplineMode::Bezier);
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let sample = sample_spline_normalized(&spline, SplineMode::Bezier, x).unwrap();
            assert!(
                (0.0..=1.0).contains(&sample[0]) && (0.0..=1.0).contains(&sample[1]),
                "at {x}: {sample:?}"
            );
        }
    }

    #[test]
    fn control_point_outline_is_white_on_dark_fill() {
        let outline = control_point_outline(Color32::from_rgb(20, 20, 60), false);
        assert_eq!(outline.color, Color32::WHITE);
    }

    #[test]
    fn control_point_outline_is_black_on_bright_fill() {
        let outline = control_point_outline(Color32::from_rgb(240, 240, 200), false);
        assert_eq!(outline.color, Color32::BLACK);
    }

    #[test]
    fn emphasized_control_point_outline_is_wider() {
        let fill = Color32::from_rgb(20, 20, 60);
        assert!(control_point_outline(fill, true).width > control_point_outline(fill, false).width);
    }

    #[test]
    fn control_point_hint_is_one_based_and_mentions_right_click() {
        let hint = control_point_hint(1, 4);
        assert!(hint.contains("2 of 4"), "{hint}");
        assert!(hint.to_lowercase().contains("right-click"), "{hint}");
    }

    #[test]
    fn hovering_a_point_sets_grab_cursor() {
        let (_, output) =
            run_frames_with_output(vec![vec![egui::Event::PointerMoved(MIDDLE_CENTER)]]);
        assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::Grab);
    }
}
