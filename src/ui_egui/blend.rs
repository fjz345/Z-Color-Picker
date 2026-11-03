use std::ops::{Add, Div, Mul, Sub};

use ecolor::Color32;
use splines::Spline;

use crate::{
    color_space::{
        blend_to_linear, linear_to_blend, linear_to_color32, linear_to_picker, map_into_srgb_gamut,
        BlendSpace, PickerSpace, ACHROMATIC_CHROMA,
    },
    common::SplineMode,
    datatypes::control_point::{ControlPoint, ControlPointValue},
    math::shortest_hue_delta,
    ui_egui::control_points::{
        bezier_handle, control_points_to_spline, flatten_control_points, key_ts,
        sample_spline_normalized, spline_through_keys,
    },
};

#[derive(Clone, Copy)]
struct BlendValue([f32; 3]);

impl Add for BlendValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + rhs.0[i]))
    }
}

impl Sub for BlendValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] - rhs.0[i]))
    }
}

impl Mul<f32> for BlendValue {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self(self.0.map(|c| c * rhs))
    }
}

impl Div<f32> for BlendValue {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        Self(self.0.map(|c| c / rhs))
    }
}

splines::impl_Interpolate!(f32, BlendValue, std::f32::consts::PI);

enum BlendedSpline {
    Picker(Spline<f32, ControlPointValue>),
    PickerComponentwise(Spline<f32, BlendValue>),
    Mixed(BlendSpace, Spline<f32, BlendValue>),
}

pub struct Gradient {
    spline: BlendedSpline,
    spline_mode: SplineMode,
    picker_space: PickerSpace,
    key_ts: Vec<f32>,
}

impl Gradient {
    pub fn new(
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
        picker_space: PickerSpace,
        blend_space: BlendSpace,
    ) -> Self {
        let key_ts = key_ts(control_points);
        let spline = match blend_space {
            BlendSpace::Picker if picker_space.is_z_hue() => BlendedSpline::Picker(
                control_points_to_spline(&flatten_control_points(control_points), spline_mode),
            ),
            BlendSpace::Picker => {
                let keys = picker_keys(control_points);
                BlendedSpline::PickerComponentwise(spline_through_keys(&key_ts, &keys, spline_mode))
            }
            mixed => {
                let keys = blend_keys(control_points, picker_space, mixed);
                BlendedSpline::Mixed(mixed, spline_through_keys(&key_ts, &keys, spline_mode))
            }
        };
        Self {
            spline,
            spline_mode,
            picker_space,
            key_ts,
        }
    }

    pub fn key_ts(&self) -> &[f32] {
        &self.key_ts
    }

    pub fn linear_at(&self, x: f32) -> Option<[f32; 3]> {
        let linear = match &self.spline {
            BlendedSpline::Picker(spline) => {
                sample_spline_normalized(spline, self.spline_mode, x)?.linear(self.picker_space)
            }
            BlendedSpline::PickerComponentwise(spline) => {
                ControlPointValue::from(sample_spline_normalized(spline, self.spline_mode, x)?.0)
                    .linear(self.picker_space)
            }
            BlendedSpline::Mixed(space, spline) => blend_to_linear(
                *space,
                sample_spline_normalized(spline, self.spline_mode, x)?.0,
            ),
        };
        Some(map_into_srgb_gamut(linear))
    }

    pub fn color_at(&self, x: f32) -> Option<Color32> {
        self.linear_at(x).map(linear_to_color32)
    }

    pub fn picker_value_at(&self, x: f32) -> Option<ControlPointValue> {
        match &self.spline {
            BlendedSpline::Picker(spline) => sample_spline_normalized(spline, self.spline_mode, x),
            BlendedSpline::PickerComponentwise(spline) => {
                sample_spline_normalized(spline, self.spline_mode, x)
                    .map(|sample| ControlPointValue::from(sample.0))
            }
            BlendedSpline::Mixed(..) => {
                let [hue, s, y] = linear_to_picker(self.picker_space, self.linear_at(x)?);
                Some(ControlPointValue::new(
                    s.clamp(0.0, 1.0),
                    y.clamp(0.0, 1.0),
                    hue,
                ))
            }
        }
    }
}

fn picker_keys(control_points: &[ControlPoint]) -> Vec<[BlendValue; 3]> {
    control_points
        .iter()
        .map(|cp| {
            [*cp.val(), bezier_handle(cp, 0), bezier_handle(cp, 1)].map(|v| BlendValue(v.val))
        })
        .collect()
}

fn blend_keys(
    control_points: &[ControlPoint],
    picker_space: PickerSpace,
    space: BlendSpace,
) -> Vec<[BlendValue; 3]> {
    let mut keys: Vec<[BlendValue; 3]> = control_points
        .iter()
        .map(|cp| {
            [*cp.val(), bezier_handle(cp, 0), bezier_handle(cp, 1)]
                .map(|value| BlendValue(linear_to_blend(space, value.linear(picker_space))))
        })
        .collect();
    if space == BlendSpace::Oklch {
        unwrap_oklch_hues(&mut keys);
    }
    keys
}

fn chromatic_hue(oklch: &BlendValue) -> Option<f32> {
    let [_, chroma, hue] = oklch.0;
    (chroma >= ACHROMATIC_CHROMA).then_some(hue)
}

fn unwrap_oklch_hues(keys: &mut [[BlendValue; 3]]) {
    let mut carried_hue = keys
        .iter()
        .find_map(|[value, ..]| chromatic_hue(value))
        .unwrap_or(0.0);
    let mut previous_hue: Option<f32> = None;
    for key in keys.iter_mut() {
        carried_hue = chromatic_hue(&key[0]).unwrap_or(carried_hue);
        let hue = previous_hue.map_or(carried_hue, |previous| {
            previous + shortest_hue_delta(previous, carried_hue)
        });
        key[0].0[2] = hue;
        for handle in &mut key[1..] {
            let handle_hue = chromatic_hue(handle).unwrap_or(hue);
            handle.0[2] = hue + shortest_hue_delta(hue, handle_hue);
        }
        previous_hue = Some(hue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_space::{
        linear_to_oklab, linear_to_srgb, oklab_to_linear, oklab_to_oklch, srgb_to_linear,
    };
    use crate::math::hue_abs_distance;

    fn point(s: f32, v: f32, h: f32, t: f32) -> ControlPoint {
        ControlPoint::new_simple(ControlPointValue::new(s, v, h), t)
    }

    fn with_handles(mut cp: ControlPoint, handles: [[f32; 3]; 2]) -> ControlPoint {
        *cp.tangents_mut() = handles.map(|[s, v, h]| Some(ControlPointValue::new(s, v, h)));
        cp
    }

    fn curved_points() -> Vec<ControlPoint> {
        vec![
            with_handles(
                point(0.8, 0.92, 0.95, 0.0),
                [[-0.1, 0.0, 0.0], [0.2, -0.1, 0.05]],
            ),
            with_handles(
                point(0.3, 0.6, 0.1, 0.3),
                [[-0.1, 0.1, -0.02], [0.1, -0.1, 0.02]],
            ),
            point(0.0, 0.45, 0.4, 0.55),
            with_handles(
                point(0.9, 0.4, 0.6, 1.0),
                [[0.0, 0.3, 0.1], [0.1, 0.0, 0.0]],
            ),
        ]
    }

    fn xs() -> impl Iterator<Item = f32> {
        (0..=64).map(|i| i as f32 / 64.0)
    }

    fn oklch_of(linear: [f32; 3]) -> [f32; 3] {
        oklab_to_oklch(linear_to_oklab(linear))
    }

    #[test]
    fn picker_blend_is_the_picker_spline_bit_for_bit() {
        let mut wrapped = curved_points();
        *wrapped[1].val_mut() = ControlPointValue::new(0.3, 0.6, 2.1);
        for points in [curved_points(), wrapped] {
            for mode in SplineMode::ALL {
                let spline = control_points_to_spline(&flatten_control_points(&points), mode);
                for space in PickerSpace::ALL.into_iter().filter(|s| s.is_z_hue()) {
                    let gradient = Gradient::new(&points, mode, space, BlendSpace::Picker);
                    for x in xs() {
                        let sample = sample_spline_normalized(&spline, mode, x);
                        assert_eq!(
                            gradient.picker_value_at(x),
                            sample,
                            "{mode:?} {space:?} {x}"
                        );
                        assert_eq!(
                            gradient.color_at(x),
                            sample.map(|value| value.color(space)),
                            "{mode:?} {space:?} {x}"
                        );
                    }
                }
            }
        }
    }

    const OKLAB_PICKER: PickerSpace = PickerSpace::Oklab {
        clip_to_srgb: false,
    };

    #[test]
    fn oklab_picker_blend_moves_lightness_straight_without_wrapping() {
        let points = [point(0.4, 0.6, 0.9, 0.0), point(0.6, 0.45, 0.1, 1.0)];
        for mode in SplineMode::ALL {
            let gradient = Gradient::new(&points, mode, OKLAB_PICKER, BlendSpace::Picker);
            let mut previous = 0.9;
            for x in xs() {
                let l = gradient.picker_value_at(x).unwrap().h();
                assert!(
                    l <= previous + 1e-6 && (0.1 - 1e-6..=0.9 + 1e-6).contains(&l),
                    "{mode:?} at {x}: L {l} after {previous}"
                );
                previous = l;
            }
            let mid = gradient.picker_value_at(0.5).unwrap();
            assert!((mid.h() - 0.5).abs() < 1e-5, "{mode:?}: {mid:?}");
            assert_eq!(gradient.color_at(0.5), Some(mid.color(OKLAB_PICKER)));
        }
    }

    #[test]
    fn oklab_picker_blend_lerps_every_coordinate_in_linear_mode() {
        let points = curved_points();
        let gradient = Gradient::new(
            &points,
            SplineMode::Linear,
            OKLAB_PICKER,
            BlendSpace::Picker,
        );
        for pair in points.windows(2) {
            let (a, b) = (*pair[0].val(), *pair[1].val());
            let x = 0.5 * (pair[0].t() + pair[1].t());
            let mid = gradient.picker_value_at(x).unwrap();
            for i in 0..3 {
                assert!(
                    (mid[i] - 0.5 * (a[i] + b[i])).abs() < 1e-5,
                    "{mid:?} {a:?} {b:?}"
                );
            }
        }
    }

    #[test]
    fn every_blend_hits_the_key_colors_at_their_t() {
        let points = curved_points();
        for blend in BlendSpace::ALL {
            for mode in SplineMode::ALL {
                for space in PickerSpace::ALL {
                    let gradient = Gradient::new(&points, mode, space, blend);
                    for cp in &points {
                        assert_eq!(
                            gradient.color_at(*cp.t()),
                            Some(cp.val().color(space)),
                            "{blend:?} {mode:?} {space:?} at T {}",
                            cp.t()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn componentwise_blends_lerp_their_coordinates() {
        type Conversion = fn([f32; 3]) -> [f32; 3];
        let spaces: [(BlendSpace, Conversion, Conversion); 3] = [
            (BlendSpace::Srgb, linear_to_srgb, srgb_to_linear),
            (BlendSpace::LinearRgb, |c| c, |c| c),
            (BlendSpace::Oklab, linear_to_oklab, oklab_to_linear),
        ];
        let points = [point(0.8, 0.9, 0.05, 0.0), point(0.5, 0.6, 0.6, 1.0)];
        for (blend, to_blend, from_blend) in spaces {
            let gradient = Gradient::new(&points, SplineMode::Linear, PickerSpace::Hsv, blend);
            let [a, b] = points
                .each_ref()
                .map(|cp| to_blend(cp.val().linear(PickerSpace::Hsv)));
            let expected = from_blend(std::array::from_fn(|i| 0.5 * (a[i] + b[i])));
            let mid = gradient.linear_at(0.5).unwrap();
            for i in 0..3 {
                assert!(
                    (mid[i] - expected[i]).abs() < 1e-5,
                    "{blend:?}: {mid:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn oklch_takes_the_short_hue_arc_across_zero() {
        let points = [point(0.6, 0.8, 0.0, 0.0), point(0.6, 0.8, 0.9, 1.0)];
        let [from, to] = points
            .each_ref()
            .map(|cp| oklch_of(cp.val().linear(PickerSpace::Hsv))[2]);
        let arc = shortest_hue_delta(from, to);
        assert!(!(0.0..1.0).contains(&(from + arc)), "{from} -> {to}");
        for mode in SplineMode::ALL {
            let gradient = Gradient::new(&points, mode, PickerSpace::Hsv, BlendSpace::Oklch);
            for x in [0.25, 0.5, 0.75] {
                let hue = oklch_of(gradient.linear_at(x).unwrap())[2];
                let progress = shortest_hue_delta(from, hue) * arc.signum();
                assert!(
                    (-1e-3..=arc.abs() + 1e-3).contains(&progress),
                    "{mode:?} at {x}: {hue} is off the arc {from} + {arc}"
                );
                if mode == SplineMode::Linear {
                    assert!(
                        hue_abs_distance(hue, from + x * arc) < 1e-3,
                        "at {x}: {hue} vs {}",
                        from + x * arc
                    );
                }
            }
        }
    }

    #[test]
    fn oklch_gray_keys_take_a_neighbours_hue() {
        let orange = point(0.8, 0.9, 0.08, 0.0);
        let gray = point(0.0, 0.5, 0.6, 0.0);
        let orange_hue = oklch_of(orange.val().linear(PickerSpace::Hsv))[2];
        let with_t = |cp: &ControlPoint, t| point(cp.val().s(), cp.val().v(), cp.val().h(), t);
        for points in [
            [with_t(&gray, 0.0), with_t(&orange, 1.0)],
            [with_t(&orange, 0.0), with_t(&gray, 1.0)],
        ] {
            let gradient = Gradient::new(
                &points,
                SplineMode::Linear,
                PickerSpace::Hsv,
                BlendSpace::Oklch,
            );
            for x in [0.25, 0.5, 0.75] {
                let hue = oklch_of(gradient.linear_at(x).unwrap())[2];
                assert!(hue_abs_distance(hue, orange_hue) < 1e-3, "at {x}: {hue}");
            }
        }
    }

    #[test]
    fn oklch_samples_outside_srgb_keep_their_oklch_lightness_and_hue() {
        let green = point(1.0, 1.0, 1.0 / 3.0, 0.0);
        let blue = point(1.0, 1.0, 2.0 / 3.0, 1.0);
        let [from, to] = [&green, &blue].map(|cp| oklch_of(cp.val().linear(PickerSpace::Hsv)));
        let unmapped = [
            0.5 * (from[0] + to[0]),
            0.5 * (from[1] + to[1]),
            from[2] + 0.5 * shortest_hue_delta(from[2], to[2]),
        ];
        let clipped =
            oklch_of(blend_to_linear(BlendSpace::Oklch, unmapped).map(|c| c.clamp(0.0, 1.0)));
        assert!(
            hue_abs_distance(clipped[2], unmapped[2]) > 0.01,
            "{clipped:?} {unmapped:?}"
        );
        let gradient = Gradient::new(
            &[green, blue],
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Oklch,
        );
        let mapped = gradient.linear_at(0.5).unwrap();
        assert!(mapped.iter().all(|c| (0.0..=1.0).contains(c)), "{mapped:?}");
        let [l, _, hue] = oklch_of(mapped);
        assert!((l - unmapped[0]).abs() < 1e-4, "{l} vs {unmapped:?}");
        assert!(
            hue_abs_distance(hue, unmapped[2]) < 1e-3,
            "{hue} vs {unmapped:?}"
        );
    }

    #[test]
    fn mixed_blend_picker_values_stay_in_the_square() {
        let points = curved_points();
        for blend in BlendSpace::ALL {
            for mode in SplineMode::ALL {
                for space in PickerSpace::ALL {
                    let gradient = Gradient::new(&points, mode, space, blend);
                    for x in xs() {
                        let value = gradient.picker_value_at(x).unwrap();
                        assert!(value.h().is_finite(), "{blend:?} {mode:?} {space:?} {x}");
                        if blend != BlendSpace::Picker {
                            for c in [value.s(), value.v()] {
                                assert!(
                                    (0.0..=1.0).contains(&c),
                                    "{blend:?} {mode:?} {space:?} {x}: {value:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn gradient_without_points_has_no_samples() {
        for blend in BlendSpace::ALL {
            for mode in SplineMode::ALL {
                let gradient = Gradient::new(&[], mode, PickerSpace::Hsv, blend);
                assert!(gradient.key_ts().is_empty());
                assert_eq!(gradient.color_at(0.5), None, "{blend:?} {mode:?}");
                assert_eq!(gradient.picker_value_at(0.5), None, "{blend:?} {mode:?}");
            }
        }
    }
}
