use std::ops::RangeInclusive;

use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Ui, Vec2};
use splines::Spline;

use crate::color_space::{BlendSpace, PickerSpace};
use crate::common::SplineMode;
use crate::datatypes::control_point::{ControlPoint, ControlPointValue};
use crate::ui_egui::blend::Gradient;
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

#[cfg(test)]
fn curvature(cubic: &Cubic, t: f32) -> Option<f32> {
    curvature_from(cubic.derivative(t), cubic.second_derivative(t))
}

fn curvature_from(d1: Vec2, d2: Vec2) -> Option<f32> {
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

fn curvature_normal(d1: Vec2, d2: Vec2) -> Option<Vec2> {
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

#[derive(Clone, Copy, Debug)]
struct CombSample {
    point: Pos2,
    derivatives: Option<(Vec2, Vec2)>,
}

fn is_segment_sample(segment_index: usize, sample_index: usize) -> bool {
    segment_index == 0 || sample_index > 0
}

fn cubic_comb_segments(cubics: &[Cubic]) -> Vec<Vec<CombSample>> {
    (cubics.iter().enumerate())
        .map(|(segment_index, cubic)| {
            (0..=SAMPLES_PER_SEGMENT)
                .filter(|&i| is_segment_sample(segment_index, i))
                .map(|i| {
                    let t = i as f32 / SAMPLES_PER_SEGMENT as f32;
                    CombSample {
                        point: cubic.eval(t),
                        derivatives: Some((cubic.derivative(t), cubic.second_derivative(t))),
                    }
                })
                .collect()
        })
        .collect()
}

const SAMPLE_STENCIL: usize = 4;

fn sampled_comb_segments(gradient: &Gradient, rect: egui::Rect) -> Vec<Vec<CombSample>> {
    let step = SAMPLE_STENCIL as f32 / SAMPLES_PER_SEGMENT as f32;
    (gradient.key_ts().windows(2).enumerate())
        .map(|(segment_index, span)| {
            let positions: Vec<Option<Vec2>> = (0..=SAMPLES_PER_SEGMENT)
                .map(|i| {
                    let x = egui::lerp(span[0]..=span[1], i as f32 / SAMPLES_PER_SEGMENT as f32);
                    let value = gradient.picker_value_at(x)?;
                    Some(normalized_to_screen(rect, value_pos(&value)).to_vec2())
                })
                .collect();
            (0..=SAMPLES_PER_SEGMENT)
                .filter(|&i| is_segment_sample(segment_index, i))
                .filter_map(|i| {
                    let center = i.clamp(SAMPLE_STENCIL, SAMPLES_PER_SEGMENT - SAMPLE_STENCIL);
                    let stencil = [center - SAMPLE_STENCIL, center, center + SAMPLE_STENCIL]
                        .map(|j| positions[j]);
                    let derivatives = match stencil {
                        [Some(before), Some(at), Some(after)] => Some((
                            (after - before) / (2.0 * step),
                            (after - at * 2.0 + before) / (step * step),
                        )),
                        _ => None,
                    };
                    Some(CombSample {
                        point: positions[i]?.to_pos2(),
                        derivatives,
                    })
                })
                .collect()
        })
        .collect()
}

pub fn draw_curvature_comb(
    ui: &mut Ui,
    rect: egui::Rect,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    picker_space: PickerSpace,
    blend_space: BlendSpace,
    scale: f32,
) {
    if control_points.len() < 2 {
        return;
    }
    let segments = if blend_space == BlendSpace::Picker {
        cubic_comb_segments(&comb_cubics(control_points, spline_mode, rect))
    } else {
        let gradient = Gradient::new(control_points, spline_mode, picker_space, blend_space);
        sampled_comb_segments(&gradient, rect)
    };
    let tooth_color = Color32::from_rgba_unmultiplied(255, 255, 255, COMB_ALPHA);
    let outline_color = Color32::from_rgba_unmultiplied(255, 255, 255, COMB_OUTLINE_ALPHA);
    let mut shapes = Vec::with_capacity(segments.len() * (SAMPLES_PER_SEGMENT + 1) * 2);
    for samples in &segments {
        let mut outline_points = Vec::with_capacity(SAMPLES_PER_SEGMENT + 1);

        for &CombSample { point, derivatives } in samples {
            let tooth = derivatives
                .and_then(|(d1, d2)| Some((curvature_from(d1, d2)?, curvature_normal(d1, d2)?)));
            let Some((k, normal)) = tooth else {
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
    use crate::datatypes::control_point::ControlPointStorage;
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

    fn c_shape(degrees: &[f32], ts: &[f32]) -> Vec<ControlPoint> {
        degrees
            .iter()
            .zip(ts)
            .map(|(degree, &t)| {
                let (sin, cos) = degree.to_radians().sin_cos();
                let value = ControlPointValue::new(0.5 + 0.35 * cos, 0.5 + 0.35 * sin, 0.5);
                ControlPoint::new_simple(value, t)
            })
            .collect()
    }

    #[test]
    fn hermite_comb_of_a_c_shape_bends_one_way_on_every_segment() {
        let even = [0.0, 0.25, 0.5, 0.75, 1.0];
        let degrees = [70.0, 130.0, 180.0, 250.0, 290.0];
        let by_angle = degrees.map(|d| (d - degrees[0]) / (degrees[4] - degrees[0]));
        let semicircle = [0.0, 45.0, 90.0, 135.0, 180.0];
        for points in [
            c_shape(&[60.0, 120.0, 180.0, 240.0, 300.0], &even),
            c_shape(&degrees, &even),
            c_shape(&degrees, &by_angle),
            c_shape(&degrees[..4], &[0.0, 0.3, 0.6, 1.0]),
            c_shape(&semicircle, &[0.0, 0.1, 0.5, 0.6, 1.0]),
            c_shape(&semicircle, &[0.0, 0.4, 0.5, 0.9, 1.0]),
        ] {
            let cubics = comb_cubics(&points, SplineMode::HermiteBezier, RECT);
            assert_eq!(cubics.len(), points.len() - 1);
            let curvatures: Vec<f32> = cubics
                .iter()
                .flat_map(|cubic| {
                    (0..=SAMPLES_PER_SEGMENT)
                        .map(|i| curvature(cubic, i as f32 / SAMPLES_PER_SEGMENT as f32).unwrap())
                })
                .collect();
            let sign = curvatures[curvatures.len() / 2].signum();
            let last = curvatures.len() - 1;
            for (i, k) in curvatures.iter().enumerate() {
                let segment = i / (SAMPLES_PER_SEGMENT + 1);
                let message = format!("{} points, segment {segment}: {k}", points.len());
                if i == 0 || i == last {
                    assert!(k.abs() < 1e-6, "{message}");
                } else {
                    assert_eq!(k.signum(), sign, "{message}");
                }
            }
        }
    }

    #[test]
    fn hermite_end_keys_have_zero_second_derivative() {
        type SvAtT = (&'static [(f32, f32)], &'static [f32]);
        let shapes: [SvAtT; 3] = [
            (
                &[(0.25, 0.33), (0.44, 0.38), (0.8, 0.6), (0.9, 0.8)],
                &[0.0, 0.33, 0.66, 1.0],
            ),
            (&[(0.0, 0.0), (0.7, 0.3), (1.0, 1.0)], &[0.0, 0.2, 1.0]),
            (
                &[(0.1, 0.9), (0.3, 0.2), (0.6, 0.7), (0.8, 0.1), (0.95, 0.5)],
                &[0.0, 0.1, 0.45, 0.6, 1.0],
            ),
        ];
        for (sv, ts) in shapes {
            let points: Vec<ControlPoint> = points_sv(sv)
                .into_iter()
                .zip(ts)
                .map(|(mut cp, &t)| {
                    *cp.t_mut() = t;
                    cp
                })
                .collect();
            let cubics = comb_cubics(&points, SplineMode::HermiteBezier, RECT);
            let [first, last] = [cubics[0], cubics[cubics.len() - 1]];
            for (end, second) in [
                ("first", first.second_derivative(0.0)),
                ("last", last.second_derivative(1.0)),
            ] {
                assert!(second.length() < 1e-2, "{sv:?} {end}: {second:?}");
            }
        }
    }

    fn curvatures_of(segments: &[Vec<CombSample>]) -> Vec<Vec<Option<f32>>> {
        segments
            .iter()
            .map(|samples| {
                samples
                    .iter()
                    .map(|sample| {
                        sample
                            .derivatives
                            .and_then(|(d1, d2)| curvature_from(d1, d2))
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn sampled_comb_matches_the_cubic_comb_when_the_blend_is_affine_to_the_picker() {
        let oklab = crate::color_space::PickerSpace::Oklab {
            clip_to_srgb: false,
        };
        let points: Vec<ControlPoint> = [
            ([0.45, 0.55, 0.5], [0.06, 0.04], 0.0),
            ([0.62, 0.6, 0.6], [0.02, -0.05], 0.3),
            ([0.55, 0.4, 0.7], [-0.05, -0.02], 0.7),
            ([0.4, 0.46, 0.55], [-0.03, 0.05], 1.0),
        ]
        .into_iter()
        .map(|(val, [ds, dv], t)| {
            ControlPoint::ControlPointLeftRightTangent(ControlPointStorage {
                val: val.into(),
                t,
                tangents: [
                    Some(ControlPointValue::new(-ds, -dv, 0.0)),
                    Some(ControlPointValue::new(ds, dv, 0.0)),
                ],
            })
        })
        .collect();
        for mode in [SplineMode::Bezier, SplineMode::HermiteBezier] {
            let gradient = Gradient::new(&points, mode, oklab, BlendSpace::Oklab);
            let sampled = curvatures_of(&sampled_comb_segments(&gradient, RECT));
            let exact = curvatures_of(&cubic_comb_segments(&comb_cubics(&points, mode, RECT)));
            assert_eq!(sampled.len(), exact.len(), "{mode:?}");
            for (segment, (sampled, exact)) in sampled.iter().zip(&exact).enumerate() {
                assert_eq!(sampled.len(), exact.len(), "{mode:?} segment {segment}");
                let inner = SAMPLE_STENCIL..sampled.len() - SAMPLE_STENCIL;
                for (k_sampled, k_exact) in sampled[inner.clone()].iter().zip(&exact[inner]) {
                    let [k_sampled, k_exact] = [k_sampled.unwrap(), k_exact.unwrap()];
                    assert!(
                        (k_sampled - k_exact).abs() <= 0.02 * k_exact.abs() + 1e-4,
                        "{mode:?} segment {segment}: {k_sampled} vs {k_exact}"
                    );
                }
            }
        }
    }

    #[test]
    fn linear_spline_blended_in_oklab_bends_on_the_hsv_square() {
        let points = vec![
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.0), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.3, 0.5, 0.6), 1.0),
        ];
        let gradient = Gradient::new(
            &points,
            SplineMode::Linear,
            crate::color_space::PickerSpace::Hsv,
            BlendSpace::Oklab,
        );
        let curvatures = curvatures_of(&sampled_comb_segments(&gradient, RECT));
        let bent = curvatures
            .iter()
            .flatten()
            .flatten()
            .any(|k| k.abs() > 1e-3);
        assert!(bent, "{curvatures:?}");
    }

    #[test]
    fn linear_mode_has_no_comb() {
        let points = points_sv(&[(0.1, 0.2), (0.4, 0.9), (0.7, 0.3)]);
        assert!(comb_cubics(&points, SplineMode::Linear, RECT).is_empty());
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
