use ecolor::Color32;
use eframe::egui::{Pos2, Vec2};
use serde::{Deserialize, Serialize};

use crate::color_space::{
    is_in_srgb_gamut, largest_fraction_where, largest_scale_in_srgb, linear_to_color32,
    linear_to_oklab, map_into_srgb_gamut, picker_to_linear, PickerSpace,
};
use crate::math::hue_lerp;

type HsvKeyValueInnerType = [f32; 3];
#[repr(C)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsvKeyValue {
    pub val: HsvKeyValueInnerType,
}

impl Default for HsvKeyValue {
    fn default() -> Self {
        Self {
            val: [0.0, 0.0, 0.0],
        }
    }
}

impl HsvKeyValue {
    pub fn new(x: f32, y: f32, h: f32) -> Self {
        Self { val: [x, y, h] }
    }
    pub fn vec2(&self) -> Vec2 {
        Vec2::new(self[0], self[1])
    }

    pub fn pos2(&self) -> Pos2 {
        Pos2::new(self[0], self[1])
    }

    pub fn h(&self) -> f32 {
        self[2]
    }

    pub fn s(&self) -> f32 {
        self[0]
    }

    pub fn v(&self) -> f32 {
        self[1]
    }

    pub fn color(&self, space: PickerSpace) -> Color32 {
        linear_to_color32(self.linear(space))
    }

    pub fn linear(&self, space: PickerSpace) -> [f32; 3] {
        map_into_srgb_gamut(self.unmapped_linear(space))
    }

    pub fn unmapped_linear(&self, space: PickerSpace) -> [f32; 3] {
        picker_to_linear(
            space,
            [
                space.z_on_bar(self[2]),
                self[0].clamp(0.0, 1.0),
                self[1].clamp(0.0, 1.0),
            ],
        )
    }

    pub fn is_in_srgb(&self, space: PickerSpace) -> bool {
        is_in_srgb_gamut(self.unmapped_linear(space))
    }

    pub fn oklab_lightness(&self, space: PickerSpace) -> f32 {
        match space {
            PickerSpace::Oklch { .. } => self[1].clamp(0.0, 1.0),
            PickerSpace::Oklab { .. } => space.z_on_bar(self[2]),
            _ => linear_to_oklab(self.unmapped_linear(space))[0],
        }
    }

    pub fn with_oklab_lightness(self, space: PickerSpace, lightness: f32) -> Self {
        let mut value = self;
        match space {
            PickerSpace::Oklch { .. } => value[1] = lightness,
            PickerSpace::Oklab { .. } => value[2] = lightness,
            _ => {
                let lightness_at = |y: f32| Self::new(self[0], y, self[2]).oklab_lightness(space);
                value[1] = if lightness_at(1.0) <= lightness {
                    1.0
                } else {
                    largest_fraction_where(|y| lightness_at(y) <= lightness)
                };
            }
        }
        value
    }

    pub fn clipped_to_srgb(&self, space: PickerSpace) -> Self {
        let [x, y] = [self[0], self[1]].map(|c| c.clamp(0.0, 1.0));
        let z = if space.is_z_hue() {
            self[2]
        } else {
            space.z_on_bar(self[2])
        };
        let in_square = Self::new(x, y, z);
        if in_square.is_in_srgb(space) {
            return in_square;
        }
        let gray = match space {
            PickerSpace::Oklch { .. } => [0.0, y],
            PickerSpace::Oklab { .. } => [0.5, 0.5],
            _ => unreachable!("{space:?} never leaves sRGB"),
        };
        let toward_gray = |scale: f32| {
            Self::new(
                gray[0] + scale * (x - gray[0]),
                gray[1] + scale * (y - gray[1]),
                z,
            )
        };
        toward_gray(largest_scale_in_srgb(|scale| {
            toward_gray(scale).unmapped_linear(space)
        }))
    }
}

impl From<HsvKeyValueInnerType> for HsvKeyValue {
    fn from(item: HsvKeyValueInnerType) -> Self {
        HsvKeyValue { val: item }
    }
}

impl std::ops::Index<usize> for HsvKeyValue {
    type Output = f32;
    fn index(&self, s: usize) -> &f32 {
        match s {
            0 => &self.val[0],
            1 => &self.val[1],
            2 => &self.val[2],
            _ => panic!("unknown field: {}", s),
        }
    }
}

impl std::ops::IndexMut<usize> for HsvKeyValue {
    fn index_mut(&mut self, s: usize) -> &mut f32 {
        match s {
            0 => &mut self.val[0],
            1 => &mut self.val[1],
            2 => &mut self.val[2],
            _ => panic!("unknown field: {}", s),
        }
    }
}

impl std::ops::Add<f32> for HsvKeyValue {
    type Output = Self;

    fn add(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] + rhs, self.val[1] + rhs, self.val[2] + rhs],
        }
    }
}

impl std::ops::Add<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn add(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [rhs.val[0] + self, rhs.val[1] + self, rhs.val[2] + self],
        }
    }
}

impl std::ops::Add<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn add(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] + rhs.val[0],
                self.val[1] + rhs.val[1],
                self.val[2] + rhs.val[2],
            ],
        }
    }
}

impl std::ops::Sub<f32> for HsvKeyValue {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] - rhs, self.val[1] - rhs, self.val[2] - rhs],
        }
    }
}

impl std::ops::Sub<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn sub(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [self - rhs.val[0], self - rhs.val[1], self - rhs.val[2]],
        }
    }
}

impl std::ops::Sub<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn sub(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] - rhs.val[0],
                self.val[1] - rhs.val[1],
                self.val[2] - rhs.val[2],
            ],
        }
    }
}

impl std::ops::Mul<f32> for HsvKeyValue {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] * rhs, self.val[1] * rhs, self.val[2] * rhs],
        }
    }
}

impl std::ops::Mul<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn mul(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [self * rhs.val[0], self * rhs.val[1], self * rhs.val[2]],
        }
    }
}

impl std::ops::Mul<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn mul(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] * rhs.val[0],
                self.val[1] * rhs.val[1],
                self.val[2] * rhs.val[2],
            ],
        }
    }
}

impl std::ops::Div<f32> for HsvKeyValue {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] / rhs, self.val[1] / rhs, self.val[2] / rhs],
        }
    }
}

impl std::ops::Div<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn div(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [self / rhs.val[0], self / rhs.val[1], self / rhs.val[2]],
        }
    }
}

impl std::ops::Div<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn div(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] / rhs.val[0],
                self.val[1] / rhs.val[1],
                self.val[2] / rhs.val[2],
            ],
        }
    }
}

fn mix(from: HsvKeyValue, to: HsvKeyValue, t: f32) -> HsvKeyValue {
    from * (1.0 - t) + to * t
}

fn de_casteljau(
    t: f32,
    a: HsvKeyValue,
    u: HsvKeyValue,
    v: HsvKeyValue,
    b: HsvKeyValue,
) -> HsvKeyValue {
    let (au, uv, vb) = (mix(a, u, t), mix(u, v, t), mix(v, b, t));
    mix(mix(au, uv, t), mix(uv, vb, t), t)
}

impl splines::interpolate::Interpolate<f32> for HsvKeyValue {
    fn step(t: f32, threshold: f32, a: Self, b: Self) -> Self {
        if t >= threshold {
            b
        } else {
            a
        }
    }

    fn cosine(t: f32, a: Self, b: Self) -> Self {
        let eased = 0.5 - 0.5 * (std::f32::consts::PI * t).cos();
        <Self as splines::interpolate::Interpolate<f32>>::lerp(eased, a, b)
    }

    fn lerp(t: f32, a: Self, b: Self) -> Self {
        Self {
            val: [
                a.val[0] * (1. - t) + b.val[0] * t,
                a.val[1] * (1. - t) + b.val[1] * t,
                hue_lerp(a.val[2], b.val[2], t),
            ],
        }
    }

    fn cubic_hermite(
        t: f32,
        before: (f32, Self),
        start: (f32, Self),
        end: (f32, Self),
        after: (f32, Self),
    ) -> Self {
        let start_velocity = (end.1 - before.1) / (end.0 - before.0);
        let end_velocity = (after.1 - start.1) / (after.0 - start.0);
        let handle_time = (end.0 - start.0) / 3.0;
        de_casteljau(
            t,
            start.1,
            start.1 + start_velocity * handle_time,
            end.1 - end_velocity * handle_time,
            end.1,
        )
    }

    fn quadratic_bezier(t: f32, a: Self, u: Self, b: Self) -> Self {
        mix(mix(a, u, t), mix(u, b, t), t)
    }

    fn cubic_bezier(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        de_casteljau(t, a, u, v, b)
    }

    fn cubic_bezier_mirrored(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        de_casteljau(t, a, u, b + (b - v), b)
    }
}

#[cfg(test)]
pub mod test_support {
    use super::HsvKeyValue;

    pub fn assert_close(actual: HsvKeyValue, expected: HsvKeyValue) {
        for i in 0..3 {
            assert!(
                (actual[i] - expected[i]).abs() < 1e-5,
                "{actual:?} != {expected:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::assert_close;
    use super::*;
    use crate::color_space::{is_in_srgb_gamut, map_into_srgb_gamut};
    use ecolor::HsvaGamma;
    use splines::interpolate::Interpolate;

    fn bezier(t: f32, a: HsvKeyValue, b: HsvKeyValue) -> HsvKeyValue {
        <HsvKeyValue as Interpolate<f32>>::cubic_bezier(t, a, a, b, b)
    }

    #[test]
    fn cubic_bezier_interpolates_at_half_turn_hue_distance() {
        let a = HsvKeyValue::new(0.2, 0.3, 0.0);
        let b = HsvKeyValue::new(0.8, 0.9, 0.5);
        assert_close(bezier(0.0, a, b), a);
        assert_close(bezier(1.0, a, b), b);
        assert_close(bezier(0.5, a, b), HsvKeyValue::new(0.5, 0.6, 0.25));
    }

    #[test]
    fn cubic_bezier_hits_endpoints() {
        let a = HsvKeyValue::new(0.2, 0.3, 0.1);
        let b = HsvKeyValue::new(0.8, 0.9, 0.3);
        assert_close(bezier(0.0, a, b), a);
        assert_close(bezier(1.0, a, b), b);
    }

    #[test]
    fn step_holds_a_until_the_threshold() {
        let (a, b) = (
            HsvKeyValue::new(0.1, 0.2, 0.3),
            HsvKeyValue::new(0.7, 0.8, 0.9),
        );
        assert_eq!(<HsvKeyValue as Interpolate<f32>>::step(0.49, 0.5, a, b), a);
        assert_eq!(<HsvKeyValue as Interpolate<f32>>::step(0.5, 0.5, a, b), b);
    }

    #[test]
    fn cosine_eases_between_the_keys() {
        let (a, b) = (
            HsvKeyValue::new(0.0, 0.2, 0.1),
            HsvKeyValue::new(1.0, 0.6, 0.3),
        );
        let cosine = |t| <HsvKeyValue as Interpolate<f32>>::cosine(t, a, b);
        assert_close(cosine(0.0), a);
        assert_close(cosine(1.0), b);
        assert_close(cosine(0.5), HsvKeyValue::new(0.5, 0.4, 0.2));
        let eased = 0.146_446_6;
        assert_close(
            cosine(0.25),
            HsvKeyValue::new(eased, 0.2 + 0.4 * eased, 0.1 + 0.2 * eased),
        );
    }

    #[test]
    fn cubic_hermite_uses_catmull_rom_tangents_over_uneven_key_times() {
        let before = (0.0, HsvKeyValue::new(0.0, 0.0, 0.1));
        let start = (1.0, HsvKeyValue::new(1.0, 0.0, 0.2));
        let end = (3.0, HsvKeyValue::new(1.0, 1.0, 0.4));
        let after = (4.0, HsvKeyValue::new(0.0, 1.0, 0.5));
        let hermite =
            |t| <HsvKeyValue as Interpolate<f32>>::cubic_hermite(t, before, start, end, after);
        assert_close(hermite(0.0), start.1);
        assert_close(hermite(1.0), end.1);
        assert_close(hermite(0.5), HsvKeyValue::new(7.0 / 6.0, 0.5, 0.3));
    }

    #[test]
    fn quadratic_bezier_weights_the_handle_by_its_bernstein_basis() {
        let a = HsvKeyValue::new(0.0, 0.0, 0.0);
        let u = HsvKeyValue::new(1.0, 0.5, 0.2);
        let b = HsvKeyValue::new(0.0, 1.0, 0.4);
        let quadratic = |t| <HsvKeyValue as Interpolate<f32>>::quadratic_bezier(t, a, u, b);
        assert_close(quadratic(0.0), a);
        assert_close(quadratic(1.0), b);
        assert_close(quadratic(0.5), HsvKeyValue::new(0.5, 0.5, 0.2));
    }

    #[test]
    fn cubic_bezier_midpoint_weights_inner_handles_three_eighths() {
        let a = HsvKeyValue::new(0.0, 0.0, 0.0);
        let u = HsvKeyValue::new(0.8, 0.0, 0.8);
        let v = HsvKeyValue::new(0.8, 0.8, 0.0);
        let b = HsvKeyValue::new(0.0, 0.8, 0.8);
        let mid = <HsvKeyValue as Interpolate<f32>>::cubic_bezier(0.5, a, u, v, b);
        assert_close(mid, HsvKeyValue::new(0.6, 0.4, 0.4));
    }

    #[test]
    fn cubic_bezier_mirrored_reflects_the_end_handle_through_the_end() {
        let a = HsvKeyValue::new(0.1, 0.1, 0.1);
        let u = HsvKeyValue::new(0.4, 0.2, 0.2);
        let v = HsvKeyValue::new(0.6, 0.5, 0.3);
        let b = HsvKeyValue::new(0.8, 0.9, 0.5);
        let reflected = HsvKeyValue::new(1.0, 1.3, 0.7);
        for t in [0.0, 0.3, 0.5, 0.8, 1.0] {
            assert_close(
                <HsvKeyValue as Interpolate<f32>>::cubic_bezier_mirrored(t, a, u, v, b),
                <HsvKeyValue as Interpolate<f32>>::cubic_bezier(t, a, u, reflected, b),
            );
        }
    }

    #[test]
    fn lerp_mixes_sv_linearly_and_hue_shortest_way() {
        let mixed = <HsvKeyValue as Interpolate<f32>>::lerp(
            0.5,
            HsvKeyValue::new(0.0, 0.0, 0.9),
            HsvKeyValue::new(1.0, 1.0, 0.1),
        );
        assert!((mixed.s() - 0.5).abs() < 1e-5);
        assert!((mixed.v() - 0.5).abs() < 1e-5);
        assert!(mixed.h() < 1e-5 || mixed.h() > 1.0 - 1e-5);
    }

    #[test]
    fn color_of_out_of_range_sv_matches_clamped_value() {
        for space in PickerSpace::ALL {
            for s in [-0.2, 0.5, 1.3] {
                for v in [-0.2, 0.5, 1.3] {
                    let raw = HsvKeyValue::new(s, v, 0.1);
                    let clamped = HsvKeyValue::new(s.clamp(0.0, 1.0), v.clamp(0.0, 1.0), 0.1);
                    assert_eq!(
                        raw.color(space),
                        clamped.color(space),
                        "{space:?} s = {s}, v = {v}"
                    );
                }
            }
        }
    }

    #[test]
    fn hsv_color_is_the_hsva_gamma_color() {
        let expected = Color32::from(HsvaGamma {
            h: 0.1,
            s: 0.3,
            v: 0.7,
            a: 1.0,
        });
        assert_eq!(
            HsvKeyValue::new(0.3, 0.7, 0.1).color(PickerSpace::Hsv),
            expected
        );
    }

    fn hue_spaces() -> impl Iterator<Item = PickerSpace> {
        PickerSpace::ALL
            .into_iter()
            .filter(|space| space.is_z_hue())
    }

    const OKLCH: PickerSpace = PickerSpace::Oklch {
        clip_to_srgb: false,
    };
    const OKLAB: PickerSpace = PickerSpace::Oklab {
        clip_to_srgb: false,
    };

    #[test]
    fn with_oklab_lightness_reaches_the_target_keeping_x_and_z() {
        for space in PickerSpace::ALL {
            for (x, z) in [(0.0, 0.3), (0.25, 0.6), (0.25, 0.95)] {
                let value = HsvKeyValue::new(x, 0.9, z);
                for target in [0.1, 0.45, 0.8] {
                    let moved = value.with_oklab_lightness(space, target);
                    let lightness = moved.oklab_lightness(space);
                    assert!(
                        (lightness - target).abs() < 1e-5,
                        "{space:?} {value:?} -> {moved:?}: L {lightness}, wanted {target}"
                    );
                    let kept = if space.is_z_hue() { [0, 2] } else { [0, 1] };
                    for axis in kept {
                        assert_eq!(moved[axis], value[axis], "{space:?} {value:?} -> {moved:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn with_oklab_lightness_ignores_the_current_lightness_coordinate() {
        for space in PickerSpace::ALL {
            let axis = if space.is_z_hue() { 1 } else { 2 };
            let moved_from = |start: f32| {
                let mut value = HsvKeyValue::new(0.3, 0.4, 0.55);
                value[axis] = start;
                value.with_oklab_lightness(space, 0.6)
            };
            for start in [0.0, 0.2, 0.7, 1.0] {
                assert_eq!(moved_from(start), moved_from(0.5), "{space:?} from {start}");
            }
        }
    }

    #[test]
    fn with_oklab_lightness_stops_at_the_square_edge_out_of_reach() {
        let blue = HsvKeyValue::new(1.0, 0.5, 2.0 / 3.0);
        for space in [PickerSpace::Hsv, PickerSpace::Okhsv] {
            assert_eq!(blue.with_oklab_lightness(space, 0.95).v(), 1.0, "{space:?}");
            assert_eq!(blue.with_oklab_lightness(space, 0.0).v(), 0.0, "{space:?}");
        }
    }

    #[test]
    fn color_wraps_hue() {
        for space in hue_spaces() {
            let expected = HsvKeyValue::new(0.3, 0.7, 0.9).color(space);
            for hue in [-0.1, 1.9] {
                assert_eq!(
                    HsvKeyValue::new(0.3, 0.7, hue).color(space),
                    expected,
                    "{space:?} {hue}"
                );
            }
        }
    }

    #[test]
    fn oklab_color_clamps_lightness() {
        let at = |l| HsvKeyValue::new(0.45, 0.55, l).color(OKLAB);
        assert_eq!(at(-0.1), at(0.0));
        assert_eq!(at(1.9), at(1.0));
        assert_ne!(at(0.0), at(1.0));
    }

    #[test]
    fn color_outside_srgb_is_the_gamut_mapped_color() {
        let vivid = HsvKeyValue::new(1.0, 0.6, 0.4);
        let unmapped = picker_to_linear(OKLCH, [0.4, 1.0, 0.6]);
        assert!(!is_in_srgb_gamut(unmapped));
        assert_eq!(vivid.unmapped_linear(OKLCH), unmapped);
        assert_eq!(vivid.linear(OKLCH), map_into_srgb_gamut(unmapped));
        assert_ne!(
            vivid.color(OKLCH),
            linear_to_color32(unmapped.map(|c| c.clamp(0.0, 1.0)))
        );
    }

    fn sample_values() -> Vec<HsvKeyValue> {
        let mut values = Vec::new();
        for z in [0.0, 0.1, 0.35, 0.6, 0.85, 1.0] {
            for x in [0.0, 0.2, 0.5, 0.8, 1.0] {
                for y in [0.0, 0.15, 0.5, 0.75, 1.0] {
                    values.push(HsvKeyValue::new(x, y, z));
                }
            }
        }
        values
    }

    #[test]
    fn clip_keeps_values_inside_srgb_bit_for_bit() {
        for space in [OKLCH, OKLAB] {
            for value in sample_values().into_iter().filter(|v| v.is_in_srgb(space)) {
                assert_eq!(value.clipped_to_srgb(space), value, "{space:?}");
            }
        }
    }

    #[test]
    fn clip_reduces_chroma_onto_srgb_keeping_lightness_and_hue() {
        for space in [OKLCH, OKLAB] {
            let outside = sample_values().into_iter().filter(|v| !v.is_in_srgb(space));
            for value in outside {
                let clipped = value.clipped_to_srgb(space);
                assert!(
                    clipped.is_in_srgb(space),
                    "{space:?} {value:?}: {clipped:?}"
                );
                assert_eq!(
                    clipped.clipped_to_srgb(space),
                    clipped,
                    "{space:?} {value:?}"
                );
                assert_eq!(clipped.h(), value.h(), "{space:?} {value:?}");
                let gray = if space == OKLCH {
                    assert_eq!(clipped.v(), value.v(), "{value:?}");
                    Vec2::new(0.0, value.v())
                } else {
                    Vec2::splat(0.5)
                };
                let [from_gray, clipped_from_gray] = [value, clipped].map(|v| v.vec2() - gray);
                let scale = clipped_from_gray.length() / from_gray.length();
                assert!((0.0..1.0).contains(&scale), "{space:?} {value:?}: {scale}");
                assert!(
                    (clipped_from_gray - scale * from_gray).length() < 1e-6,
                    "{space:?} {value:?}: {clipped:?}"
                );
            }
        }
    }

    #[test]
    fn clip_lands_on_the_srgb_surface() {
        let value = HsvKeyValue::new(1.0, 0.5, 0.7);
        for space in [OKLCH, OKLAB] {
            let clipped = value.clipped_to_srgb(space);
            let linear = clipped.unmapped_linear(space);
            assert!(
                linear.iter().any(|c| !(1e-4..=1.0 - 1e-4).contains(c)),
                "{space:?}: {linear:?}"
            );
        }
    }

    #[test]
    fn clip_pulls_values_off_the_square_back_in() {
        let clipped = HsvKeyValue::new(-0.4, 1.3, 1.7).clipped_to_srgb(OKLAB);
        assert_eq!(
            clipped,
            HsvKeyValue::new(0.0, 1.0, 1.0).clipped_to_srgb(OKLAB)
        );
        let clipped = HsvKeyValue::new(1.6, -0.2, 2.25).clipped_to_srgb(OKLCH);
        assert_eq!(clipped.h(), 2.25);
        assert_eq!(clipped.v(), 0.0);
    }

    #[test]
    fn y_is_lightness_in_hsl_spaces() {
        let top_left = HsvKeyValue::new(0.0, 1.0, 0.3);
        for space in hue_spaces() {
            assert_eq!(top_left.color(space), Color32::WHITE, "{space:?}");
        }
        let full_saturation_mid_y = HsvKeyValue::new(1.0, 0.5, 0.0);
        assert_eq!(
            full_saturation_mid_y.color(PickerSpace::Hsl),
            Color32::from_rgb(255, 0, 0)
        );
    }
}
