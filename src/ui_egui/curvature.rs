use std::ops::RangeInclusive;

use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Ui, Vec2};
use splines::Spline;

use crate::common::SplineMode;
use crate::datatypes::control_point::{ControlPoint, ControlPointValue};
use crate::ui_egui::control_points::{
    bezier_handle, control_point_pos, control_points_to_spline, value_pos,
};

pub const COMB_SCALE_RANGE: RangeInclusive<f32> = 0.2..=5.0;
const SAMPLES_PER_SEGMENT: usize = 64;
const CURVATURE_SCALE: f32 = 100.0;
const MAX_COMB_LENGTH: f32 = 45.0;
const MIN_DERIVATIVE_LENGTH: f32 = 1.0e-3;
const MIN_TOOTH_LENGTH: f32 = 0.25;
const COMB_STROKE_WIDTH: f32 = 1.0;
const COMB_ALPHA: u8 = 120;
const COMB_OUTLINE_STROKE_WIDTH: f32 = 1.0;
const COMB_OUTLINE_ALPHA: u8 = 210;

#[derive(Clone, Copy, Debug)]
struct Cubic {
    p0: Pos2,
    p1: Pos2,
    p2: Pos2,
    p3: Pos2,
}

impl Cubic {
    fn eval(&self, t: f32) -> Pos2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;

        let p = self.p0.to_vec2() * (u * u * u)
            + self.p1.to_vec2() * (3.0 * u * u * t)
            + self.p2.to_vec2() * (3.0 * u * t * t)
            + self.p3.to_vec2() * (t * t * t);

        p.to_pos2()
    }

    fn derivative(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;

        (self.p1 - self.p0) * (3.0 * u * u)
            + (self.p2 - self.p1) * (6.0 * u * t)
            + (self.p3 - self.p2) * (3.0 * t * t)
    }

    fn second_derivative(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;

        let a = self.p2.to_vec2() - self.p1.to_vec2() * 2.0 + self.p0.to_vec2();
        let b = self.p3.to_vec2() - self.p2.to_vec2() * 2.0 + self.p1.to_vec2();

        a * (6.0 * u) + b * (6.0 * t)
    }
}

fn curvature(cubic: &Cubic, t: f32) -> Option<f32> {
    let d1 = cubic.derivative(t);
    let d2 = cubic.second_derivative(t);

    let speed_squared = d1.length_sq();
    if !speed_squared.is_finite() {
        return None;
    }
    let speed = speed_squared.sqrt();
    if !speed.is_finite() || speed < MIN_DERIVATIVE_LENGTH {
        return None;
    }

    let cross = d1.x * d2.y - d1.y * d2.x;
    let denominator = speed_squared * speed;
    if !cross.is_finite() || !denominator.is_finite() || denominator <= f32::EPSILON {
        return None;
    }

    let k = -cross / denominator;
    k.is_finite().then_some(k)
}

fn curvature_normal(cubic: &Cubic, t: f32) -> Option<Vec2> {
    let d1 = cubic.derivative(t);
    let d2 = cubic.second_derivative(t);

    let speed_squared = d1.length_sq();
    if !speed_squared.is_finite() {
        return None;
    }
    let speed = speed_squared.sqrt();
    if !speed.is_finite() || speed < MIN_DERIVATIVE_LENGTH {
        return None;
    }

    let cross = d1.x * d2.y - d1.y * d2.x;
    if !cross.is_finite() || cross.abs() < f32::EPSILON {
        return None;
    }

    let tangent = d1 / speed;
    let left_normal = Vec2::new(-tangent.y, tangent.x);
    Some(left_normal * -cross.signum())
}

fn normalized_to_screen(rect: egui::Rect, position: Pos2) -> Pos2 {
    Pos2::new(
        rect.left() + position.x * rect.width(),
        rect.top() + position.y * rect.height(),
    )
}

fn cubic_for_segment(start: &ControlPoint, end: &ControlPoint, rect: egui::Rect) -> Cubic {
    Cubic {
        p0: normalized_to_screen(rect, control_point_pos(start)),
        p1: normalized_to_screen(rect, value_pos(&bezier_handle(start, 1))),
        p2: normalized_to_screen(rect, value_pos(&bezier_handle(end, 0))),
        p3: normalized_to_screen(rect, control_point_pos(end)),
    }
}

fn unclamped_picker_pos(val: &ControlPointValue) -> Pos2 {
    Pos2::new(val[0], 1.0 - val[1])
}

fn catmull_rom_cubics(spline: &Spline<f32, ControlPointValue>) -> Vec<Cubic> {
    spline
        .keys()
        .windows(4)
        .map(|keys| {
            let [before, start, end, after] = [&keys[0], &keys[1], &keys[2], &keys[3]];
            let pos = |key: &splines::Key<f32, ControlPointValue>| unclamped_picker_pos(&key.value);
            let span = end.t - start.t;
            let start_tangent = (pos(end) - pos(before)) / (end.t - before.t) * span;
            let end_tangent = (pos(after) - pos(start)) / (after.t - start.t) * span;
            Cubic {
                p0: pos(start),
                p1: pos(start) + start_tangent / 3.0,
                p2: pos(end) - end_tangent / 3.0,
                p3: pos(end),
            }
        })
        .collect()
}

pub fn catmull_rom_handles(control_points: &[ControlPoint], index: usize) -> [Option<Pos2>; 2] {
    let spline = control_points_to_spline(control_points, SplineMode::HermiteBezier);
    let cubics = catmull_rom_cubics(&spline);
    let incoming = index
        .checked_sub(1)
        .and_then(|i| cubics.get(i))
        .map(|c| c.p2);
    let outgoing = cubics.get(index).map(|c| c.p1);
    [incoming, outgoing]
}

fn comb_cubics(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    rect: egui::Rect,
) -> Vec<Cubic> {
    match spline_mode {
        SplineMode::Linear => Vec::new(),
        SplineMode::Bezier => control_points
            .windows(2)
            .map(|pair| cubic_for_segment(&pair[0], &pair[1], rect))
            .collect(),
        SplineMode::HermiteBezier => {
            catmull_rom_cubics(&control_points_to_spline(control_points, spline_mode))
                .into_iter()
                .map(|c| Cubic {
                    p0: normalized_to_screen(rect, c.p0),
                    p1: normalized_to_screen(rect, c.p1),
                    p2: normalized_to_screen(rect, c.p2),
                    p3: normalized_to_screen(rect, c.p3),
                })
                .collect()
        }
    }
}

fn curvature_to_comb_length(k: f32, scale: f32) -> f32 {
    if !k.is_finite() || k == 0.0 {
        return 0.0;
    }
    let scaled_curvature = k.abs() * CURVATURE_SCALE;
    scaled_curvature.atan() * (2.0 * MAX_COMB_LENGTH / std::f32::consts::PI) * scale
}

pub fn draw_curvature_comb(
    ui: &mut Ui,
    rect: egui::Rect,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    scale: f32,
) {
    let tooth_color = Color32::from_rgba_unmultiplied(255, 255, 255, COMB_ALPHA);
    let outline_color = Color32::from_rgba_unmultiplied(255, 255, 255, COMB_OUTLINE_ALPHA);
    let cubics = comb_cubics(control_points, spline_mode, rect);
    let mut shapes = Vec::with_capacity(cubics.len() * (SAMPLES_PER_SEGMENT + 1) * 2);
    for (segment_index, cubic) in cubics.iter().enumerate() {
        let mut outline_points = Vec::with_capacity(SAMPLES_PER_SEGMENT + 1);

        for sample_index in 0..=SAMPLES_PER_SEGMENT {
            if segment_index > 0 && sample_index == 0 {
                continue;
            }

            let t = sample_index as f32 / SAMPLES_PER_SEGMENT as f32;
            let point = cubic.eval(t);
            let Some(k) = curvature(cubic, t) else {
                if outline_points.len() >= 2 {
                    shapes.push(Shape::line(
                        outline_points.clone(),
                        Stroke::new(COMB_OUTLINE_STROKE_WIDTH, outline_color),
                    ));
                }
                outline_points.clear();
                continue;
            };

            let Some(normal) = curvature_normal(cubic, t) else {
                if outline_points.len() >= 2 {
                    shapes.push(Shape::line(
                        outline_points.clone(),
                        Stroke::new(COMB_OUTLINE_STROKE_WIDTH, outline_color),
                    ));
                }
                outline_points.clear();
                continue;
            };

            let tooth_length = curvature_to_comb_length(k, scale);
            let tooth_end = if tooth_length < MIN_TOOTH_LENGTH {
                point
            } else {
                point + normal * tooth_length
            };

            if tooth_length >= MIN_TOOTH_LENGTH {
                shapes.push(Shape::line_segment(
                    [point, tooth_end],
                    Stroke::new(COMB_STROKE_WIDTH, tooth_color),
                ));
            }
            outline_points.push(tooth_end);
        }

        if outline_points.len() >= 2 {
            shapes.push(Shape::line(
                outline_points,
                Stroke::new(COMB_OUTLINE_STROKE_WIDTH, outline_color),
            ));
        }
    }

    ui.painter().with_clip_rect(rect).extend(shapes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::test_support::{
        point_with_tangent, RUNAWAY_POINT, RUNAWAY_TANGENT,
    };
    use splines::{Interpolation, Key};

    const RECT: egui::Rect = egui::Rect {
        min: Pos2::new(100.0, 100.0),
        max: Pos2::new(300.0, 300.0),
    };

    #[test]
    fn runaway_outgoing_tangent_puts_p1_on_border_at_clamped_handle() {
        let start = point_with_tangent(RUNAWAY_POINT, 1, RUNAWAY_TANGENT);
        let end = point_with_tangent([0.775, 0.592, 0.604], 0, [-0.175, 0.315, 0.0]);
        let cubic = cubic_for_segment(&start, &end, RECT);
        assert_eq!(cubic.p1, RECT.right_top());
    }

    #[test]
    fn in_range_tangents_offset_the_endpoints() {
        let start = point_with_tangent([0.2, 0.3, 0.5], 1, [0.1, 0.2, 0.0]);
        let end = point_with_tangent([0.8, 0.6, 0.5], 0, [-0.1, 0.15, 0.0]);
        let cubic = cubic_for_segment(&start, &end, RECT);
        let size = RECT.size();
        let expected_p1 = cubic.p0 + Vec2::new(0.1 * size.x, -0.2 * size.y);
        let expected_p2 = cubic.p3 + Vec2::new(-0.1 * size.x, -0.15 * size.y);
        assert!((cubic.p1 - expected_p1).length() < 1e-3, "{cubic:?}");
        assert!((cubic.p2 - expected_p2).length() < 1e-3, "{cubic:?}");
    }

    fn points_sv(sv: &[(f32, f32)]) -> Vec<ControlPoint> {
        sv.iter()
            .map(|&(s, v)| ControlPoint::new_simple(ControlPointValue::new(s, v, 0.5), 0.0))
            .collect()
    }

    fn catmull_rom_spline(times: [f32; 6]) -> Spline<f32, ControlPointValue> {
        let sv = [
            (0.1, 0.2),
            (0.1, 0.2),
            (0.4, 0.9),
            (0.7, 0.3),
            (0.9, 0.6),
            (0.9, 0.6),
        ];
        Spline::from_vec(
            times
                .into_iter()
                .zip(sv)
                .map(|(t, (s, v))| {
                    Key::new(
                        t,
                        ControlPointValue::new(s, v, 0.5),
                        Interpolation::CatmullRom,
                    )
                })
                .collect(),
        )
    }

    fn assert_cubics_match_spline(spline: &Spline<f32, ControlPointValue>) {
        let cubics = catmull_rom_cubics(spline);
        let real_keys = &spline.keys()[1..spline.len() - 1];
        assert_eq!(cubics.len(), real_keys.len() - 1);
        for (cubic, segment) in cubics.iter().zip(real_keys.windows(2)) {
            for nt in [0.0, 0.2, 0.5, 0.75, 0.95] {
                let t = segment[0].t + nt * (segment[1].t - segment[0].t);
                let sample = spline.sample(t).unwrap();
                let expected = Pos2::new(sample[0], 1.0 - sample[1]);
                let actual = cubic.eval(nt);
                assert!(
                    (actual - expected).length() < 1e-5,
                    "t {t}: {actual:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn catmull_rom_cubics_match_spline_with_uniform_key_times() {
        assert_cubics_match_spline(&catmull_rom_spline([0.0, 1.0, 2.0, 3.0, 4.0, 5.0]));
    }

    #[test]
    fn catmull_rom_cubics_match_spline_with_non_uniform_key_times() {
        assert_cubics_match_spline(&catmull_rom_spline([-0.4, 0.0, 0.15, 0.7, 1.0, 1.9]));
    }

    #[test]
    fn catmull_rom_cubics_match_the_hermite_spline_of_the_control_points() {
        let points = points_sv(&[(0.1, 0.2), (0.4, 0.9), (0.7, 0.3), (0.9, 0.6)]);
        assert_cubics_match_spline(&control_points_to_spline(
            &points,
            SplineMode::HermiteBezier,
        ));
    }

    #[test]
    fn hermite_comb_cubics_run_through_every_control_point() {
        let points = points_sv(&[(0.1, 0.2), (0.4, 0.9), (0.7, 0.3)]);
        let cubics = comb_cubics(&points, SplineMode::HermiteBezier, RECT);
        assert_eq!(cubics.len(), 2);
        let screen = |cp: &ControlPoint| normalized_to_screen(RECT, control_point_pos(cp));
        for (cubic, pair) in cubics.iter().zip(points.windows(2)) {
            assert!((cubic.p0 - screen(&pair[0])).length() < 1e-3, "{cubic:?}");
            assert!((cubic.p3 - screen(&pair[1])).length() < 1e-3, "{cubic:?}");
        }
    }

    #[test]
    fn linear_mode_has_no_comb() {
        let points = points_sv(&[(0.1, 0.2), (0.4, 0.9), (0.7, 0.3)]);
        assert!(comb_cubics(&points, SplineMode::Linear, RECT).is_empty());
    }

    #[test]
    fn hermite_handles_of_a_middle_point_span_a_sixth_of_the_neighbour_chord() {
        let points = points_sv(&[(0.1, 0.2), (0.4, 0.9), (0.7, 0.3)]);
        let [incoming, outgoing] = catmull_rom_handles(&points, 1);
        let point = Pos2::new(0.4, 0.1);
        let chord = Vec2::new(0.6, -0.1);
        assert!((incoming.unwrap() - (point - chord / 6.0)).length() < 1e-5);
        assert!((outgoing.unwrap() - (point + chord / 6.0)).length() < 1e-5);
    }

    #[test]
    fn hermite_end_points_have_only_their_inner_handle() {
        let points = points_sv(&[(0.1, 0.2), (0.4, 0.9), (0.7, 0.3)]);
        assert!(matches!(catmull_rom_handles(&points, 0), [None, Some(_)]));
        assert!(matches!(catmull_rom_handles(&points, 2), [Some(_), None]));
        assert!(matches!(catmull_rom_handles(&points[..1], 0), [None, None]));
    }

    #[test]
    fn comb_scale_multiplies_every_tooth_length() {
        for k in [1.0e-4, 0.01, 0.5, 100.0] {
            let unscaled = curvature_to_comb_length(k, 1.0);
            let scaled = curvature_to_comb_length(k, 2.5);
            assert!((scaled - 2.5 * unscaled).abs() < 1e-4, "k {k}");
        }
    }

    #[test]
    fn scaled_comb_length_saturates_at_scaled_max() {
        let length = curvature_to_comb_length(1.0e9, 3.0);
        assert!(length <= 3.0 * MAX_COMB_LENGTH, "{length}");
        assert!(length > 2.99 * MAX_COMB_LENGTH, "{length}");
    }
}
