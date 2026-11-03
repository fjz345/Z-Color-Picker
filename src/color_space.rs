use std::f32::consts::TAU;

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

const ACHROMATIC_CHROMA: f32 = 1e-5;

fn mul(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    m.map(|row| row[0] * v[0] + row[1] * v[1] + row[2] * v[2])
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

pub fn linear_to_srgb(linear: [f32; 3]) -> [f32; 3] {
    linear.map(ecolor::gamma_from_linear)
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

pub fn srgb_to_hsv([r, g, b]: [f32; 3]) -> [f32; 3] {
    let max = r.max(g).max(b);
    let chroma = max - r.min(g).min(b);
    if chroma < ACHROMATIC_CHROMA {
        return [0.0, 0.0, max];
    }
    [hue_turns([r, g, b], max, chroma), chroma / max, max]
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
    fn linear_to_srgb_inverts_srgb_to_linear() {
        for srgb in [[0.0, 0.5, 1.0], [0.02, 0.3, 0.9]] {
            assert_close(linear_to_srgb(srgb_to_linear(srgb)), srgb, 1e-6);
        }
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

    #[test]
    fn hsv_of_known_colors() {
        assert_close(srgb_to_hsv(srgb_u8([255, 0, 0])), [0.0, 1.0, 1.0], 1e-6);
        assert_close(
            srgb_to_hsv(srgb_u8([51, 102, 153])),
            [210.0 / 360.0, 2.0 / 3.0, 0.6],
            1e-6,
        );
        assert_close(
            srgb_to_hsv(srgb_u8([255, 0, 255])),
            [300.0 / 360.0, 1.0, 1.0],
            1e-6,
        );
    }

    #[test]
    fn hsv_of_grays_has_zero_hue_and_saturation() {
        assert_eq!(srgb_to_hsv([0.0; 3]), [0.0; 3]);
        assert_eq!(srgb_to_hsv([1.0; 3]), [0.0, 0.0, 1.0]);
        assert_eq!(srgb_to_hsv([0.5; 3]), [0.0, 0.0, 0.5]);
    }
}
