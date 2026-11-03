use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickerSpace {
    #[default]
    Hsv,
    Hsl,
    Okhsv,
    Okhsl,
}

impl PickerSpace {
    pub const ALL: [Self; 4] = [Self::Hsv, Self::Hsl, Self::Okhsv, Self::Okhsl];

    pub fn is_y_lightness(self) -> bool {
        matches!(self, Self::Hsl | Self::Okhsl)
    }
}

type Mat3 = [[f32; 3]; 3];

const LINEAR_TO_LMS: Mat3 = [
    [0.412_221_46, 0.536_332_55, 0.051_445_995],
    [0.211_903_5, 0.680_699_5, 0.107_396_96],
    [0.088_302_46, 0.281_718_85, 0.629_978_7],
];

const LMS_CBRT_TO_OKLAB: Mat3 = [
    [0.210_454_26, 0.793_617_8, -0.004_072_047],
    [1.977_998_5, -2.428_592_2, 0.450_593_7],
    [0.025_904_037, 0.782_771_77, -0.808_675_77],
];

const OKLAB_TO_LMS_CBRT: Mat3 = [
    [1.0, 0.396_337_78, 0.215_803_76],
    [1.0, -0.105_561_346, -0.063_854_17],
    [1.0, -0.089_484_18, -1.291_485_5],
];

const LMS_TO_LINEAR: Mat3 = [
    [4.076_741_7, -3.307_711_6, 0.230_969_94],
    [-1.268_438, 2.609_757_4, -0.341_319_38],
    [-0.004_196_086_3, -0.703_418_6, 1.707_614_7],
];

const ACHROMATIC_CHROMA: f32 = 1e-5;

fn dot(u: [f32; 3], v: [f32; 3]) -> f32 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

fn mul(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    m.map(|row| dot(row, v))
}

fn max_channel([r, g, b]: [f32; 3]) -> f32 {
    r.max(g).max(b)
}

fn wrap_turns(turns: f32) -> f32 {
    let wrapped = turns.rem_euclid(1.0);
    if wrapped < 1.0 {
        wrapped
    } else {
        0.0
    }
}

pub fn srgb_to_linear(srgb: [f32; 3]) -> [f32; 3] {
    srgb.map(ecolor::linear_from_gamma)
}

pub fn linear_to_oklab(linear: [f32; 3]) -> [f32; 3] {
    let lms = mul(&LINEAR_TO_LMS, linear);
    mul(&LMS_CBRT_TO_OKLAB, lms.map(f32::cbrt))
}

pub fn oklab_to_oklch([l, a, b]: [f32; 3]) -> [f32; 3] {
    let chroma = a.hypot(b);
    let hue = if chroma < ACHROMATIC_CHROMA {
        0.0
    } else {
        wrap_turns(b.atan2(a) / TAU)
    };
    [l, chroma, hue]
}

pub fn srgb_to_hsl([r, g, b]: [f32; 3]) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    let lightness = (max + min) / 2.0;
    if chroma < ACHROMATIC_CHROMA {
        return [0.0, 0.0, lightness];
    }
    let saturation = chroma / (1.0 - (2.0 * lightness - 1.0).abs());
    [hue_turns([r, g, b], max, chroma), saturation, lightness]
}

fn hue_turns([r, g, b]: [f32; 3], max: f32, chroma: f32) -> f32 {
    let hue_sector = if max == r {
        (g - b) / chroma
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    wrap_turns(hue_sector / 6.0)
}

pub fn linear_to_srgb(linear: [f32; 3]) -> [f32; 3] {
    linear.map(ecolor::gamma_from_linear)
}

pub fn oklab_to_linear(lab: [f32; 3]) -> [f32; 3] {
    let lms = mul(&OKLAB_TO_LMS_CBRT, lab).map(|c| c * c * c);
    mul(&LMS_TO_LINEAR, lms)
}

pub fn picker_to_linear(space: PickerSpace, hsy: [f32; 3]) -> [f32; 3] {
    match space {
        PickerSpace::Hsv => hsv_to_linear(hsy),
        PickerSpace::Hsl => srgb_to_linear(hsl_to_srgb(hsy)),
        PickerSpace::Okhsv => okhsv_to_linear(hsy),
        PickerSpace::Okhsl => okhsl_to_linear(hsy),
    }
}

pub fn linear_to_picker(space: PickerSpace, linear: [f32; 3]) -> [f32; 3] {
    match space {
        PickerSpace::Hsv => linear_to_hsv(linear),
        PickerSpace::Hsl => srgb_to_hsl(linear_to_srgb(linear)),
        PickerSpace::Okhsv => linear_to_okhsv(linear),
        PickerSpace::Okhsl => linear_to_okhsl(linear),
    }
}

// ecolor's HsvaGamma: HSV over linear RGB, with only V gamma-encoded.
fn hsv_to_linear([h, s, v]: [f32; 3]) -> [f32; 3] {
    ecolor::rgb_from_hsv((h, s, ecolor::linear_from_gamma(v)))
}

fn linear_to_hsv(linear: [f32; 3]) -> [f32; 3] {
    let max = max_channel(linear);
    let chroma = max - linear[0].min(linear[1]).min(linear[2]);
    let value = ecolor::gamma_from_linear(max);
    if chroma < ACHROMATIC_CHROMA {
        return [0.0, 0.0, value];
    }
    [hue_turns(linear, max, chroma), chroma / max, value]
}

fn hsl_to_srgb([h, s, l]: [f32; 3]) -> [f32; 3] {
    let amplitude = s * l.min(1.0 - l);
    [0.0, 8.0, 4.0].map(|offset: f32| {
        let k = (offset + 12.0 * h).rem_euclid(12.0);
        l - amplitude * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    })
}

// OKHSV and OKHSL: ported from https://bottosson.github.io/posts/colorpicker/ (MIT, (c) 2021 Bjorn Ottosson).

#[derive(Clone, Copy)]
struct LightnessChroma {
    l: f32,
    c: f32,
}

fn hue_direction(hue: f32) -> (f32, f32) {
    let (b, a) = (TAU * hue).sin_cos();
    (a, b)
}

fn lms_direction(a: f32, b: f32) -> [f32; 3] {
    OKLAB_TO_LMS_CBRT.map(|row| row[1] * a + row[2] * b)
}

const BRANCH_TIE: f32 = 1e-4;
const HALLEY_STEPS: usize = 3;

// Deviates from the reference at sRGB blue, where both chords are 1: its f32 tie chose the blue
// branch, and red's first zero lies before blue; green first and 3 Halley steps put the cusp on blue.
fn compute_max_saturation(a: f32, b: f32) -> f32 {
    let (k, channel) = if 1.814_441_1 * a - 1.194_452_8 * b > 1.0 - BRANCH_TIE {
        (
            [
                0.739_565_15,
                -0.459_544_04,
                0.082_854_27,
                0.125_410_7,
                0.145_032_04,
            ],
            1,
        )
    } else if -1.881_703_3 * a - 0.809_364_9 * b > 1.0 - BRANCH_TIE {
        (
            [
                1.190_862_8,
                1.765_767_3,
                0.596_626_4,
                0.755_152,
                0.567_712_4,
            ],
            0,
        )
    } else {
        (
            [
                1.357_336_5,
                -0.009_157_99,
                -1.151_302_1,
                -0.505_596_06,
                0.006_921_67,
            ],
            2,
        )
    };
    let weights = LMS_TO_LINEAR[channel];
    let mut saturation = k[0] + k[1] * a + k[2] * b + k[3] * a * a + k[4] * a * b;

    let k_lms = lms_direction(a, b);
    for _ in 0..HALLEY_STEPS {
        let lms_cbrt = k_lms.map(|k| 1.0 + saturation * k);
        let f = dot(weights, lms_cbrt.map(|c| c * c * c));
        let f1 = dot(
            weights,
            [0, 1, 2].map(|i| 3.0 * k_lms[i] * lms_cbrt[i] * lms_cbrt[i]),
        );
        let f2 = dot(
            weights,
            [0, 1, 2].map(|i| 6.0 * k_lms[i] * k_lms[i] * lms_cbrt[i]),
        );
        saturation -= f * f1 / (f1 * f1 - 0.5 * f * f2);
    }
    saturation
}

fn find_cusp(a: f32, b: f32) -> LightnessChroma {
    let max_saturation = compute_max_saturation(a, b);
    let rgb_at_max = oklab_to_linear([1.0, max_saturation * a, max_saturation * b]);
    let l = (1.0 / max_channel(rgb_at_max)).cbrt();
    LightnessChroma {
        l,
        c: l * max_saturation,
    }
}

// t where L = l0 * (1 - t) + t * l1, C = t * c1 meets the gamut edge.
fn find_gamut_intersection(
    a: f32,
    b: f32,
    l1: f32,
    c1: f32,
    l0: f32,
    cusp: LightnessChroma,
) -> f32 {
    if (l1 - l0) * cusp.c - (cusp.l - l0) * c1 <= 0.0 {
        return cusp.c * l0 / (c1 * cusp.l + cusp.c * (l0 - l1));
    }
    let t = cusp.c * (l0 - 1.0) / (c1 * (cusp.l - 1.0) + cusp.c * (l0 - l1));

    let k_lms = lms_direction(a, b);
    let lms_dt = k_lms.map(|k| l1 - l0 + c1 * k);
    let l = l0 * (1.0 - t) + t * l1;
    let c = t * c1;
    let lms_cbrt = k_lms.map(|k| l + c * k);
    let lms = lms_cbrt.map(|x| x * x * x);
    let lms_d1 = [0, 1, 2].map(|i| 3.0 * lms_dt[i] * lms_cbrt[i] * lms_cbrt[i]);
    let lms_d2 = [0, 1, 2].map(|i| 6.0 * lms_dt[i] * lms_dt[i] * lms_cbrt[i]);
    let halley_steps = LMS_TO_LINEAR.map(|weights| {
        let f = dot(weights, lms) - 1.0;
        let f1 = dot(weights, lms_d1);
        let f2 = dot(weights, lms_d2);
        let u = f1 / (f1 * f1 - 0.5 * f * f2);
        if u >= 0.0 {
            -f * u
        } else {
            f32::MAX
        }
    });
    t + halley_steps[0].min(halley_steps[1]).min(halley_steps[2])
}

const TOE_K1: f32 = 0.206;
const TOE_K2: f32 = 0.03;
const TOE_K3: f32 = (1.0 + TOE_K1) / (1.0 + TOE_K2);

fn toe(x: f32) -> f32 {
    let k = TOE_K3 * x - TOE_K1;
    0.5 * (k + (k * k + 4.0 * TOE_K2 * TOE_K3 * x).sqrt())
}

fn toe_inv(x: f32) -> f32 {
    (x * x + TOE_K1 * x) / (TOE_K3 * (x + TOE_K2))
}

fn to_st(cusp: LightnessChroma) -> [f32; 2] {
    [cusp.c / cusp.l, cusp.c / (1.0 - cusp.l)]
}

fn get_st_mid(a: f32, b: f32) -> [f32; 2] {
    let s = 0.115_169_93
        + 1.0
            / (7.447_789_7
                + 4.159_012_3 * b
                + a * (-2.195_573_6
                    + 1.751_984 * b
                    + a * (-2.137_049_4 - 10.023_01 * b
                        + a * (-4.248_945_7 + 5.387_708 * b + 4.698_91 * a))));
    let t = 0.112_396_42
        + 1.0
            / (1.613_203_2 - 0.681_243_8 * b
                + a * (0.403_706_12
                    + 0.901_481_2 * b
                    + a * (-0.270_879_43
                        + 0.612_239_9 * b
                        + a * (0.002_992_15 - 0.453_995_68 * b - 0.146_618_72 * a))));
    [s, t]
}

struct ChromaStops {
    zero: f32,
    mid: f32,
    max: f32,
}

fn get_cs(l: f32, a: f32, b: f32) -> ChromaStops {
    let cusp = find_cusp(a, b);
    let max = find_gamut_intersection(a, b, l, 1.0, l, cusp);
    let [s_max, t_max] = to_st(cusp);
    let k = max / (l * s_max).min((1.0 - l) * t_max);

    let [s_mid, t_mid] = get_st_mid(a, b);
    let c_a = l * s_mid;
    let c_b = (1.0 - l) * t_mid;
    let mid = 0.9
        * k
        * (1.0 / (1.0 / c_a.powi(4) + 1.0 / c_b.powi(4)))
            .sqrt()
            .sqrt();

    let c_a = l * 0.4;
    let c_b = (1.0 - l) * 0.8;
    let zero = (1.0 / (1.0 / (c_a * c_a) + 1.0 / (c_b * c_b))).sqrt();

    ChromaStops { zero, mid, max }
}

const OKHSL_MID: f32 = 0.8;
const OKHSL_MID_INV: f32 = 1.25;

fn okhsl_to_linear([h, s, l]: [f32; 3]) -> [f32; 3] {
    if l >= 1.0 {
        return [1.0; 3];
    }
    if l <= 0.0 {
        return [0.0; 3];
    }
    let (a, b) = hue_direction(h);
    let lightness = toe_inv(l);
    let cs = get_cs(lightness, a, b);
    let chroma = if s < OKHSL_MID {
        let t = OKHSL_MID_INV * s;
        let k_1 = OKHSL_MID * cs.zero;
        let k_2 = 1.0 - k_1 / cs.mid;
        t * k_1 / (1.0 - k_2 * t)
    } else {
        let t = (s - OKHSL_MID) / (1.0 - OKHSL_MID);
        let k_1 = (1.0 - OKHSL_MID) * cs.mid * cs.mid * OKHSL_MID_INV * OKHSL_MID_INV / cs.zero;
        let k_2 = 1.0 - k_1 / (cs.max - cs.mid);
        cs.mid + t * k_1 / (1.0 - k_2 * t)
    };
    oklab_to_linear([lightness, chroma * a, chroma * b])
}

fn linear_to_okhsl(linear: [f32; 3]) -> [f32; 3] {
    let lab = linear_to_oklab(linear);
    let [lightness, chroma, hue] = oklab_to_oklch(lab);
    let l = toe(lightness);
    if chroma < ACHROMATIC_CHROMA {
        return [0.0, 0.0, l];
    }
    let (a, b) = (lab[1] / chroma, lab[2] / chroma);
    let cs = get_cs(lightness, a, b);
    let s = if chroma < cs.mid {
        let k_1 = OKHSL_MID * cs.zero;
        let k_2 = 1.0 - k_1 / cs.mid;
        OKHSL_MID * chroma / (k_1 + k_2 * chroma)
    } else {
        let k_1 = (1.0 - OKHSL_MID) * cs.mid * cs.mid * OKHSL_MID_INV * OKHSL_MID_INV / cs.zero;
        let k_2 = 1.0 - k_1 / (cs.max - cs.mid);
        let t = (chroma - cs.mid) / (k_1 + k_2 * (chroma - cs.mid));
        OKHSL_MID + (1.0 - OKHSL_MID) * t
    };
    [hue, s, l]
}

const OKHSV_S0: f32 = 0.5;

// Scale that moves the toe-corrected triangle edge onto the curved top of the gamut.
fn okhsv_scale_l(a: f32, b: f32, l_v: f32, c_v: f32) -> f32 {
    let l_vt = toe_inv(l_v);
    let c_vt = c_v * l_vt / l_v;
    let rgb_scale = oklab_to_linear([l_vt, a * c_vt, b * c_vt]);
    (1.0 / max_channel(rgb_scale).max(0.0)).cbrt()
}

fn okhsv_to_linear([h, s, v]: [f32; 3]) -> [f32; 3] {
    if v <= 0.0 {
        return [0.0; 3];
    }
    let (a, b) = hue_direction(h);
    let [s_max, t_max] = to_st(find_cusp(a, b));
    let k = 1.0 - OKHSV_S0 / s_max;
    let denominator = OKHSV_S0 + t_max - t_max * k * s;
    let l_v = 1.0 - s * OKHSV_S0 / denominator;
    let c_v = s * t_max * OKHSV_S0 / denominator;

    let l = toe_inv(v * l_v);
    let c = v * c_v * l / (v * l_v);
    let scale_l = okhsv_scale_l(a, b, l_v, c_v);
    oklab_to_linear([l * scale_l, c * scale_l * a, c * scale_l * b])
}

fn linear_to_okhsv(linear: [f32; 3]) -> [f32; 3] {
    let lab = linear_to_oklab(linear);
    let [lightness, chroma, hue] = oklab_to_oklch(lab);
    if lightness <= 0.0 {
        return [0.0; 3];
    }
    let is_achromatic = chroma < ACHROMATIC_CHROMA;
    let (a, b) = if is_achromatic {
        (1.0, 0.0)
    } else {
        (lab[1] / chroma, lab[2] / chroma)
    };
    let [s_max, t_max] = to_st(find_cusp(a, b));
    let k = 1.0 - OKHSV_S0 / s_max;
    let t = t_max / (chroma + lightness * t_max);
    let l_v = t * lightness;
    let c_v = t * chroma;

    let scale_l = okhsv_scale_l(a, b, l_v, c_v);
    let v = toe(lightness / scale_l) / l_v;
    if is_achromatic {
        return [0.0, 0.0, v];
    }
    let s = (OKHSV_S0 + t_max) * c_v / (t_max * OKHSV_S0 + t_max * k * c_v);
    [hue, s, v]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: [f32; 3], expected: [f32; 3], tolerance: f32) {
        for i in 0..3 {
            assert!(
                (actual[i] - expected[i]).abs() < tolerance,
                "{actual:?} != {expected:?}"
            );
        }
    }

    fn srgb_u8(rgb: [u8; 3]) -> [f32; 3] {
        rgb.map(|c| c as f32 / 255.0)
    }

    fn oklab_of_srgb(rgb: [u8; 3]) -> [f32; 3] {
        linear_to_oklab(srgb_to_linear(srgb_u8(rgb)))
    }

    #[test]
    fn srgb_to_linear_matches_the_srgb_transfer_curve() {
        assert_close(
            srgb_to_linear([0.0, 0.5, 1.0]),
            [0.0, 0.214_041_14, 1.0],
            1e-6,
        );
        assert_close(
            srgb_to_linear([0.02, 0.0, 0.0]),
            [0.02 / 12.92, 0.0, 0.0],
            1e-7,
        );
    }

    #[test]
    fn oklab_of_white_is_one_zero_zero() {
        assert_close(linear_to_oklab([1.0; 3]), [1.0, 0.0, 0.0], 1e-4);
    }

    #[test]
    fn oklab_of_black_is_zero() {
        assert_close(linear_to_oklab([0.0; 3]), [0.0; 3], 1e-6);
    }

    #[test]
    fn oklab_of_srgb_primaries_matches_published_values() {
        assert_close(
            oklab_of_srgb([255, 0, 0]),
            [0.627_955, 0.224_863, 0.125_846],
            1e-4,
        );
        assert_close(
            oklab_of_srgb([0, 255, 0]),
            [0.866_440, -0.233_888, 0.179_498],
            1e-4,
        );
        assert_close(
            oklab_of_srgb([0, 0, 255]),
            [0.452_014, -0.032_457, -0.311_528],
            1e-4,
        );
    }

    #[test]
    fn oklch_of_srgb_primaries_matches_published_values() {
        let red = oklab_to_oklch(oklab_of_srgb([255, 0, 0]));
        assert_close(red, [0.627_955, 0.257_683, 29.2339 / 360.0], 1e-4);
        let blue = oklab_to_oklch(oklab_of_srgb([0, 0, 255]));
        assert_close(blue, [0.452_014, 0.313_214, 264.052 / 360.0], 1e-4);
    }

    #[test]
    fn oklch_hue_is_in_turns_from_zero_below_one() {
        let lch = oklab_to_oklch([0.5, 0.1, -1e-9]);
        assert!((0.0..1.0).contains(&lch[2]), "{lch:?}");
        assert_close(oklab_to_oklch([0.5, 0.0, -0.1]), [0.5, 0.1, 0.75], 1e-6);
    }

    #[test]
    fn oklch_of_gray_has_zero_chroma_and_hue() {
        for gray in [0, 1, 128, 254, 255] {
            let lch = oklab_to_oklch(oklab_of_srgb([gray; 3]));
            assert!(lch[1] < 1e-4, "{gray}: {lch:?}");
            assert_eq!(lch[2], 0.0, "{gray}: {lch:?}");
        }
    }

    #[test]
    fn hsl_of_known_colors() {
        assert_close(srgb_to_hsl(srgb_u8([255, 0, 0])), [0.0, 1.0, 0.5], 1e-6);
        assert_close(
            srgb_to_hsl(srgb_u8([51, 102, 153])),
            [210.0 / 360.0, 0.5, 0.4],
            1e-6,
        );
        assert_close(
            srgb_to_hsl(srgb_u8([255, 128, 0])),
            [30.117_647 / 360.0, 1.0, 0.5],
            1e-6,
        );
        assert_close(
            srgb_to_hsl(srgb_u8([255, 0, 255])),
            [300.0 / 360.0, 1.0, 0.5],
            1e-6,
        );
        assert_close(
            srgb_to_hsl(srgb_u8([191, 255, 191])),
            [1.0 / 3.0, 1.0, 0.874_51],
            1e-5,
        );
    }

    #[test]
    fn hsl_of_grays_has_zero_hue_and_saturation() {
        assert_eq!(srgb_to_hsl([0.0; 3]), [0.0; 3]);
        assert_eq!(srgb_to_hsl([1.0; 3]), [0.0, 0.0, 1.0]);
        assert_eq!(srgb_to_hsl([0.5; 3]), [0.0, 0.0, 0.5]);
    }

    #[test]
    fn hsl_hue_is_in_turns_from_zero_below_one() {
        let hsl = srgb_to_hsl([1.0, 0.0, 1e-9]);
        assert!((0.0..1.0).contains(&hsl[0]), "{hsl:?}");
    }

    fn linear_grid() -> Vec<[f32; 3]> {
        let steps = [0.0, 0.002, 0.03, 0.2, 0.5, 0.8, 0.97, 1.0];
        let mut grid = Vec::new();
        for r in steps {
            for g in steps {
                for b in steps {
                    grid.push([r, g, b]);
                }
            }
        }
        grid
    }

    const PRIMARIES: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn every_picker_space_round_trips_linear_colors() {
        for space in PickerSpace::ALL {
            for linear in linear_grid() {
                let picked = linear_to_picker(space, linear);
                let back = picker_to_linear(space, picked);
                for i in 0..3 {
                    assert!(
                        (back[i] - linear[i]).abs() < 1e-4,
                        "{space:?} {linear:?} -> {picked:?} -> {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn srgb_colors_land_in_the_unit_cube() {
        for space in PickerSpace::ALL {
            for linear in linear_grid() {
                let [h, s, y] = linear_to_picker(space, linear);
                assert!((0.0..1.0).contains(&h), "{space:?} {linear:?}: h {h}");
                for c in [s, y] {
                    assert!(
                        (-1e-4..=1.0 + 1e-4).contains(&c),
                        "{space:?} {linear:?}: {s}, {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn grays_have_zero_hue_and_saturation_whichever_space_made_them() {
        for from in PickerSpace::ALL {
            for y in [0.0, 0.01, 0.37, 0.8, 1.0] {
                let gray = picker_to_linear(from, [0.37, 0.0, y]);
                for to in PickerSpace::ALL {
                    let [h, s, _] = linear_to_picker(to, gray);
                    assert_eq!([h, s], [0.0, 0.0], "{from:?} y {y} -> {to:?}");
                }
            }
        }
    }

    #[test]
    fn okhsv_and_okhsl_of_white_and_black() {
        let [_, s, v] = linear_to_picker(PickerSpace::Okhsv, [1.0; 3]);
        assert_eq!(s, 0.0);
        assert!((v - 1.0).abs() < 1e-4, "{v}");
        let [_, s, l] = linear_to_picker(PickerSpace::Okhsl, [1.0; 3]);
        assert_eq!(s, 0.0);
        assert!((l - 1.0).abs() < 1e-4, "{l}");
        assert_eq!(linear_to_picker(PickerSpace::Okhsv, [0.0; 3]), [0.0; 3]);
        assert_eq!(linear_to_picker(PickerSpace::Okhsl, [0.0; 3]), [0.0; 3]);
    }

    #[test]
    fn okhsv_and_okhsl_ends_of_the_y_axis_are_white_and_black() {
        for space in [PickerSpace::Okhsv, PickerSpace::Okhsl] {
            assert_close(picker_to_linear(space, [0.3, 0.0, 1.0]), [1.0; 3], 1e-4);
            assert_eq!(picker_to_linear(space, [0.3, 0.7, 0.0]), [0.0; 3]);
        }
        assert_eq!(
            picker_to_linear(PickerSpace::Okhsl, [0.3, 0.7, 1.0]),
            [1.0; 3]
        );
    }

    #[test]
    fn srgb_primaries_are_fully_saturated_in_okhsv_and_okhsl() {
        for primary in PRIMARIES {
            let [_, s, v] = linear_to_picker(PickerSpace::Okhsv, primary);
            assert!((s - 1.0).abs() < 1e-3, "{primary:?}: s {s}");
            assert!((v - 1.0).abs() < 1e-3, "{primary:?}: v {v}");
            let [_, s, _] = linear_to_picker(PickerSpace::Okhsl, primary);
            assert!((s - 1.0).abs() < 1e-3, "{primary:?}: s {s}");
        }
    }

    #[test]
    fn okhsv_and_okhsl_hue_of_red_is_the_oklch_hue() {
        let red = PRIMARIES[0];
        let oklch_hue = oklab_to_oklch(linear_to_oklab(red))[2];
        assert_eq!(linear_to_picker(PickerSpace::Okhsv, red)[0], oklch_hue);
        assert_eq!(linear_to_picker(PickerSpace::Okhsl, red)[0], oklch_hue);
    }

    #[test]
    fn hsl_space_is_css_hsl_over_gamma_srgb() {
        let srgb = srgb_u8([51, 102, 153]);
        let hsl = [210.0 / 360.0, 0.5, 0.4];
        assert_close(
            linear_to_picker(PickerSpace::Hsl, srgb_to_linear(srgb)),
            hsl,
            1e-5,
        );
        assert_close(
            picker_to_linear(PickerSpace::Hsl, hsl),
            srgb_to_linear(srgb),
            1e-5,
        );
    }

    #[test]
    fn hsv_space_is_ecolor_hsva_gamma() {
        let hsv = [0.1, 0.6, 0.7];
        let expected = ecolor::Rgba::from(ecolor::HsvaGamma {
            h: hsv[0],
            s: hsv[1],
            v: hsv[2],
            a: 1.0,
        });
        let [r, g, b] = picker_to_linear(PickerSpace::Hsv, hsv);
        assert_eq!([r, g, b, 1.0], expected.to_array());
        assert_close(linear_to_picker(PickerSpace::Hsv, [r, g, b]), hsv, 1e-6);
    }
}
