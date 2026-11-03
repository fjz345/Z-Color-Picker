use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickerSpace {
    #[default]
    Hsv,
    Hsl,
    Okhsv,
    Okhsl,
    Oklch {
        #[serde(default)]
        clip_to_srgb: bool,
    },
    Oklab {
        #[serde(default)]
        clip_to_srgb: bool,
    },
}

impl PickerSpace {
    pub const ALL: [Self; 6] = [
        Self::Hsv,
        Self::Hsl,
        Self::Okhsv,
        Self::Okhsl,
        Self::Oklch {
            clip_to_srgb: false,
        },
        Self::Oklab {
            clip_to_srgb: false,
        },
    ];

    pub fn is_y_lightness(self) -> bool {
        matches!(self, Self::Hsl | Self::Okhsl | Self::Oklch { .. })
    }

    pub fn is_z_hue(self) -> bool {
        !matches!(self, Self::Oklab { .. })
    }

    pub fn can_leave_srgb(self) -> bool {
        matches!(self, Self::Oklch { .. } | Self::Oklab { .. })
    }

    pub fn clips_to_srgb(self) -> bool {
        matches!(
            self,
            Self::Oklch { clip_to_srgb: true } | Self::Oklab { clip_to_srgb: true }
        )
    }

    pub fn with_clip_to_srgb(self, clip_to_srgb: bool) -> Self {
        match self {
            Self::Oklch { .. } => Self::Oklch { clip_to_srgb },
            Self::Oklab { .. } => Self::Oklab { clip_to_srgb },
            other => other,
        }
    }

    pub fn is_same_space(self, other: Self) -> bool {
        self.with_clip_to_srgb(false) == other.with_clip_to_srgb(false)
    }

    pub fn z_on_bar(self, z: f32) -> f32 {
        if self.is_z_hue() {
            z.rem_euclid(1.0)
        } else {
            z.clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendSpace {
    #[default]
    Picker,
    Srgb,
    LinearRgb,
    Oklab,
    Oklch,
}

impl BlendSpace {
    pub const ALL: [Self; 5] = [
        Self::Picker,
        Self::Srgb,
        Self::LinearRgb,
        Self::Oklab,
        Self::Oklch,
    ];
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

pub const ACHROMATIC_CHROMA: f32 = 1e-5;

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

pub fn linear_to_srgb(linear: [f32; 3]) -> [f32; 3] {
    linear.map(ecolor::gamma_from_linear)
}

pub fn linear_to_color32([r, g, b]: [f32; 3]) -> ecolor::Color32 {
    ecolor::Rgba::from_rgb(r, g, b).into()
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

pub fn oklab_to_linear(lab: [f32; 3]) -> [f32; 3] {
    let lms = mul(&OKLAB_TO_LMS_CBRT, lab).map(|c| c * c * c);
    mul(&LMS_TO_LINEAR, lms)
}

// Just past sRGB's largest Oklab chroma, 0.3225 (magenta).
pub const OK_SQUARE_CHROMA: f32 = 0.33;

pub fn square_to_ab(x: f32) -> f32 {
    (2.0 * x - 1.0) * OK_SQUARE_CHROMA
}

pub fn ab_to_square(c: f32) -> f32 {
    0.5 + c / (2.0 * OK_SQUARE_CHROMA)
}

pub fn picker_to_linear(space: PickerSpace, hsy: [f32; 3]) -> [f32; 3] {
    match space {
        PickerSpace::Hsv => hsv_to_linear(hsy),
        PickerSpace::Hsl => srgb_to_linear(hsl_to_srgb(hsy)),
        PickerSpace::Okhsv => okhsv_to_linear(hsy),
        PickerSpace::Okhsl => okhsl_to_linear(hsy),
        PickerSpace::Oklch { .. } => {
            let [hue, x, l] = hsy;
            oklab_to_linear(oklch_to_oklab([l, x * OK_SQUARE_CHROMA, hue]))
        }
        PickerSpace::Oklab { .. } => {
            let [l, x, y] = hsy;
            oklab_to_linear([l, square_to_ab(x), square_to_ab(y)])
        }
    }
}

pub fn linear_to_picker(space: PickerSpace, linear: [f32; 3]) -> [f32; 3] {
    match space {
        PickerSpace::Hsv => linear_to_hsv(linear),
        PickerSpace::Hsl => srgb_to_hsl(linear_to_srgb(linear)),
        PickerSpace::Okhsv => linear_to_okhsv(linear),
        PickerSpace::Okhsl => linear_to_okhsl(linear),
        PickerSpace::Oklch { .. } => {
            let [l, chroma, hue] = oklab_to_oklch(linear_to_oklab(linear));
            if chroma < ACHROMATIC_CHROMA {
                return [0.0, 0.0, l];
            }
            [hue, chroma / OK_SQUARE_CHROMA, l]
        }
        PickerSpace::Oklab { .. } => {
            let [l, a, b] = linear_to_oklab(linear);
            [l, ab_to_square(a), ab_to_square(b)]
        }
    }
}

pub fn oklch_to_oklab([l, chroma, hue]: [f32; 3]) -> [f32; 3] {
    let (a, b) = hue_direction(hue);
    [l, chroma * a, chroma * b]
}

const PICKER_BLEND_HAS_NO_LINEAR_FORM: &str = "the picker blend interpolates picker coordinates";

pub fn linear_to_blend(space: BlendSpace, linear: [f32; 3]) -> [f32; 3] {
    match space {
        BlendSpace::Picker => unreachable!("{PICKER_BLEND_HAS_NO_LINEAR_FORM}"),
        BlendSpace::Srgb => linear_to_srgb(linear),
        BlendSpace::LinearRgb => linear,
        BlendSpace::Oklab => linear_to_oklab(linear),
        BlendSpace::Oklch => oklab_to_oklch(linear_to_oklab(linear)),
    }
}

pub fn blend_to_linear(space: BlendSpace, blended: [f32; 3]) -> [f32; 3] {
    match space {
        BlendSpace::Picker => unreachable!("{PICKER_BLEND_HAS_NO_LINEAR_FORM}"),
        BlendSpace::Srgb => srgb_to_linear(blended),
        BlendSpace::LinearRgb => blended,
        BlendSpace::Oklab => oklab_to_linear(blended),
        BlendSpace::Oklch => {
            let [l, chroma, hue] = blended;
            // A spline overshooting below zero chroma would otherwise flip the hue.
            oklab_to_linear(oklch_to_oklab([l, chroma.max(0.0), hue]))
        }
    }
}

const GAMUT_BISECTION_STEPS: usize = 24;

pub fn is_in_srgb_gamut(linear: [f32; 3]) -> bool {
    linear.iter().all(|c| (0.0..=1.0).contains(c))
}

// Just past sRGB blue's hue a ray can leave sRGB and re-enter; this may return the inner edge.
pub fn largest_scale_in_srgb(linear_at_scale: impl Fn(f32) -> [f32; 3]) -> f32 {
    largest_fraction_where(|scale| is_in_srgb_gamut(linear_at_scale(scale)))
}

pub fn largest_fraction_where(is_inside: impl Fn(f32) -> bool) -> f32 {
    let (mut inside, mut outside) = (0.0, 1.0);
    for _ in 0..GAMUT_BISECTION_STEPS {
        let mid = 0.5 * (inside + outside);
        if is_inside(mid) {
            inside = mid;
        } else {
            outside = mid;
        }
    }
    inside
}

pub fn map_into_srgb_gamut(linear: [f32; 3]) -> [f32; 3] {
    if is_in_srgb_gamut(linear) {
        return linear;
    }
    let [l, a, b] = linear_to_oklab(linear);
    let l = l.clamp(0.0, 1.0);
    let with_chroma_scaled = |scale: f32| oklab_to_linear([l, scale * a, scale * b]);
    with_chroma_scaled(largest_scale_in_srgb(with_chroma_scaled))
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

pub fn hsl_to_srgb([h, s, l]: [f32; 3]) -> [f32; 3] {
    let amplitude = s * l.min(1.0 - l);
    [0.0, 8.0, 4.0].map(|offset: f32| {
        let k = (offset + 12.0 * h).rem_euclid(12.0);
        l - amplitude * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    })
}

fn hue_direction(hue: f32) -> (f32, f32) {
    let (b, a) = (TAU * hue).sin_cos();
    (a, b)
}

#[derive(Clone, Copy)]
struct HuePlane {
    dir: [f32; 2],
    lms_cbrt_per_chroma: [f32; 3],
}

impl HuePlane {
    fn new(a: f32, b: f32) -> Self {
        Self {
            dir: [a, b],
            lms_cbrt_per_chroma: OKLAB_TO_LMS_CBRT.map(|row| row[1] * a + row[2] * b),
        }
    }

    fn at_turns(hue: f32) -> Self {
        let (a, b) = hue_direction(hue);
        Self::new(a, b)
    }

    fn of_oklab(lab: [f32; 3], chroma: f32) -> Self {
        Self::new(lab[1] / chroma, lab[2] / chroma)
    }

    fn linear(self, l: f32, c: f32) -> [f32; 3] {
        let [a, b] = self.dir;
        oklab_to_linear([l, c * a, c * b])
    }

    // The black edge is straight: at a fixed c / l linear RGB only scales with l^3, so a channel
    // reaches 0 at one c / l.
    fn black_edge_slope(self) -> f32 {
        let [a, b] = self.dir;
        // At sRGB blue both region tests are 1: a strict f32 test there picks blue's fit, and red's
        // finds a zero short of blue. Green goes first, with a margin; 3 Halley steps reach blue.
        let fit = CHANNEL_REGION_FITS
            .iter()
            .find(|(region, _)| region[0] * a + region[1] * b > 1.0 - BRANCH_TIE)
            .map_or(&BLUE_CHANNEL_FIT, |(_, fit)| fit);
        let weights = LMS_TO_LINEAR[fit.channel];
        let mut slope = fit.initial_slope(a, b);
        for _ in 0..HALLEY_STEPS {
            let channel = channel_along(weights, [1.0; 3], self.lms_cbrt_per_chroma, slope);
            slope -= channel[0] * halley_rate(channel);
        }
        slope
    }

    fn cusp(self) -> Cusp {
        let slope = self.black_edge_slope();
        let l = max_channel(self.linear(1.0, slope)).recip().cbrt();
        Cusp { l, c: l * slope }
    }

    fn max_chroma(self, l: f32, cusp: Cusp) -> f32 {
        let slopes = cusp.slopes();
        if l <= cusp.l {
            return l * slopes.black;
        }
        // Above the cusp the gamut edge is curved: one Halley step per channel from the triangle's.
        let guess = (1.0 - l) * slopes.white;
        let step = LMS_TO_LINEAR
            .iter()
            .filter_map(|&weights| {
                let [value, slope, curvature] =
                    channel_along(weights, [l; 3], self.lms_cbrt_per_chroma, guess);
                let excess = value - 1.0;
                let rate = halley_rate([excess, slope, curvature]);
                // A channel falling as chroma grows never leaves the gamut through 1.
                (rate >= 0.0).then_some(-excess * rate)
            })
            .fold(f32::MAX, f32::min);
        guess + step
    }
}

#[derive(Clone, Copy)]
struct Cusp {
    l: f32,
    c: f32,
}

impl Cusp {
    fn slopes(self) -> EdgeSlopes {
        EdgeSlopes {
            black: self.c / self.l,
            white: self.c / (1.0 - self.l),
        }
    }
}

// The triangle black - cusp - white of a hue plane: c / l along its black edge, c / (1 - l) along
// its white edge.
#[derive(Clone, Copy)]
struct EdgeSlopes {
    black: f32,
    white: f32,
}

impl EdgeSlopes {
    fn edge_chromas(self, l: f32) -> [f32; 2] {
        [l * self.black, (1.0 - l) * self.white]
    }

    fn chroma(self, l: f32) -> f32 {
        let [black, white] = self.edge_chromas(l);
        black.min(white)
    }

    // Soft minimum of the edge chromas, sharper as p grows.
    fn rounded_chroma(self, l: f32, p: i32) -> f32 {
        let [black, white] = self.edge_chromas(l);
        (black.powi(-p) + white.powi(-p)).powf(-1.0 / p as f32)
    }
}

// Ottosson's fits: in a channel's region, region . (a, b) > 1, that channel leaves sRGB first along
// the black edge, and the polynomial in 1, a, b, a^2, ab guesses the edge's slope.
struct BlackEdgeFit {
    channel: usize,
    coefficients: [f32; 5],
}

impl BlackEdgeFit {
    fn initial_slope(&self, a: f32, b: f32) -> f32 {
        [1.0, a, b, a * a, a * b]
            .iter()
            .zip(self.coefficients)
            .map(|(monomial, k)| monomial * k)
            .sum()
    }
}

const BRANCH_TIE: f32 = 1e-4;
const HALLEY_STEPS: usize = 3;

const CHANNEL_REGION_FITS: [([f32; 2], BlackEdgeFit); 2] = [
    (
        [1.814_441_1, -1.194_452_8],
        BlackEdgeFit {
            channel: 1,
            coefficients: [
                0.739_565_15,
                -0.459_544_04,
                0.082_854_27,
                0.125_410_7,
                0.145_032_04,
            ],
        },
    ),
    (
        [-1.881_703_3, -0.809_364_9],
        BlackEdgeFit {
            channel: 0,
            coefficients: [
                1.190_862_8,
                1.765_767_3,
                0.596_626_4,
                0.755_152,
                0.567_712_4,
            ],
        },
    ),
];

const BLUE_CHANNEL_FIT: BlackEdgeFit = BlackEdgeFit {
    channel: 2,
    coefficients: [
        1.357_336_5,
        -0.009_157_99,
        -1.151_302_1,
        -0.505_596_06,
        0.006_921_67,
    ],
};

// Ottosson's fits for the edge slopes of the rounder triangle that sets Okhsl's mid chroma.
struct MidSlopeFit {
    floor: f32,
    terms: [[f32; 2]; 4],
    quartic: f32,
}

impl MidSlopeFit {
    fn eval(&self, a: f32, b: f32) -> f32 {
        let denominator = self
            .terms
            .iter()
            .rev()
            .fold(self.quartic, |acc, &[p, q]| acc * a + (p + q * b));
        self.floor + 1.0 / denominator
    }
}

const MID_BLACK_SLOPE: MidSlopeFit = MidSlopeFit {
    floor: 0.115_169_93,
    terms: [
        [7.447_789_7, 4.159_012_3],
        [-2.195_573_6, 1.751_984],
        [-2.137_049_4, -10.023_01],
        [-4.248_945_7, 5.387_708],
    ],
    quartic: 4.698_91,
};

const MID_WHITE_SLOPE: MidSlopeFit = MidSlopeFit {
    floor: 0.112_396_42,
    terms: [
        [1.613_203_2, -0.681_243_8],
        [0.403_706_12, 0.901_481_2],
        [-0.270_879_43, 0.612_239_9],
        [0.002_992_15, -0.453_995_68],
    ],
    quartic: -0.146_618_72,
};

// A linear channel along lms_cbrt = origin + t * velocity, and its first two derivatives in t.
fn channel_along(weights: [f32; 3], origin: [f32; 3], velocity: [f32; 3], t: f32) -> [f32; 3] {
    let lms_cbrt = [0, 1, 2].map(|i| origin[i] + t * velocity[i]);
    [
        dot(weights, lms_cbrt.map(|x| x * x * x)),
        3.0 * dot(
            weights,
            [0, 1, 2].map(|i| velocity[i] * lms_cbrt[i] * lms_cbrt[i]),
        ),
        6.0 * dot(
            weights,
            [0, 1, 2].map(|i| velocity[i] * velocity[i] * lms_cbrt[i]),
        ),
    ]
}

// Halley's method steps t by -f * rate toward a root of f.
fn halley_rate([f, slope, curvature]: [f32; 3]) -> f32 {
    slope / (slope * slope - 0.5 * f * curvature)
}

const TOE_K1: f32 = 0.206;
const TOE_K2: f32 = 0.03;
const TOE_K3: f32 = (1.0 + TOE_K1) / (1.0 + TOE_K2);

// Lr is Oklab's l with a toe near black, closer to CIELab's lightness; Okhsl's l is Lr.
fn lr_to_l(lr: f32) -> f32 {
    lr * (lr + TOE_K1) / (TOE_K3 * (lr + TOE_K2))
}

// The positive root of lr^2 + (k1 - k3 l) lr - k2 k3 l = 0, which is lr_to_l solved for lr.
fn l_to_lr(l: f32) -> f32 {
    let vertex = 0.5 * (TOE_K3 * l - TOE_K1);
    vertex + (vertex * vertex + TOE_K2 * TOE_K3 * l).sqrt()
}

const OKHSV_S0: f32 = 0.5;

fn okhsv_bend(slopes: EdgeSlopes) -> f32 {
    1.0 - OKHSV_S0 / slopes.black
}

// Okhsv's v = 1 edge, in Lr, is the white edge c = white * (1 - lr), from white at s 0 to the cusp
// at s 1.
fn okhsv_edge(s: f32, slopes: EdgeSlopes) -> [f32; 2] {
    let darkness = s * OKHSV_S0 / (OKHSV_S0 + slopes.white * (1.0 - okhsv_bend(slopes) * s));
    [1.0 - darkness, slopes.white * darkness]
}

fn okhsv_s_on_edge([_, edge_c]: [f32; 2], slopes: EdgeSlopes) -> f32 {
    (OKHSV_S0 + slopes.white) * edge_c / (slopes.white * (OKHSV_S0 + okhsv_bend(slopes) * edge_c))
}

// The v = 1 edge, moved from Lr to l along its ray from black, misses the curved top of the gamut;
// scaling along that ray by this factor lands it there.
fn okhsv_top_scale(plane: HuePlane, [edge_lr, edge_c]: [f32; 2]) -> f32 {
    let l = lr_to_l(edge_lr);
    let brightest = max_channel(plane.linear(l, edge_c * l / edge_lr));
    brightest.max(0.0).recip().cbrt()
}

fn okhsv_to_linear([h, s, v]: [f32; 3]) -> [f32; 3] {
    if v <= 0.0 {
        return [0.0; 3];
    }
    let plane = HuePlane::at_turns(h);
    let edge = okhsv_edge(s, plane.cusp().slopes());
    let l = lr_to_l(v * edge[0]);
    let c = edge[1] * l / edge[0];
    let scale = okhsv_top_scale(plane, edge);
    plane.linear(l * scale, c * scale)
}

fn linear_to_okhsv(linear: [f32; 3]) -> [f32; 3] {
    let lab = linear_to_oklab(linear);
    let [l, c, hue] = oklab_to_oklch(lab);
    if l <= 0.0 {
        return [0.0; 3];
    }
    let is_gray = c < ACHROMATIC_CHROMA;
    let plane = if is_gray {
        HuePlane::new(1.0, 0.0)
    } else {
        HuePlane::of_oklab(lab, c)
    };
    let slopes = plane.cusp().slopes();
    let to_edge = slopes.white / (c + l * slopes.white);
    let edge = [to_edge * l, to_edge * c];
    let v = l_to_lr(l / okhsv_top_scale(plane, edge)) / edge[0];
    if is_gray {
        return [0.0, 0.0, v];
    }
    [hue, okhsv_s_on_edge(edge, slopes), v]
}

// A rational ramp over t in [0, 1]: 0 at 0 with slope `start_slope`, `end` at 1.
#[derive(Clone, Copy)]
struct Ramp {
    start_slope: f32,
    end: f32,
}

impl Ramp {
    fn bend(self) -> f32 {
        1.0 - self.start_slope / self.end
    }

    fn at(self, t: f32) -> f32 {
        self.start_slope * t / (1.0 - self.bend() * t)
    }

    fn param_of(self, value: f32) -> f32 {
        value / (self.start_slope + self.bend() * value)
    }

    fn end_slope(self) -> f32 {
        self.end * self.end / self.start_slope
    }
}

const OKHSL_MID_S: f32 = 0.8;
const OKHSL_MID_SCALE: f32 = 0.9;
const OKHSL_GRAY_EDGES: EdgeSlopes = EdgeSlopes {
    black: 0.4,
    white: 0.8,
};
// Below it the rounded chromas' c^-4 overflows f32 into NaN; the color is black far past display precision.
const OKHSL_BLACK_L: f32 = 1e-6;

// Okhsl's s at one lightness: a ramp from 0 to `mid` chroma up to s 0.8, then one from `mid` to the
// gamut's `max`, continuing its slope; `gray_slope` is dc/ds at s 0.
#[derive(Clone, Copy)]
struct OkhslChromas {
    gray_slope: f32,
    mid: f32,
    max: f32,
}

impl OkhslChromas {
    fn at(plane: HuePlane, l: f32) -> Self {
        let cusp = plane.cusp();
        let max = plane.max_chroma(l, cusp);
        let bulge = max / cusp.slopes().chroma(l);
        let [a, b] = plane.dir;
        let mid_edges = EdgeSlopes {
            black: MID_BLACK_SLOPE.eval(a, b),
            white: MID_WHITE_SLOPE.eval(a, b),
        };
        Self {
            gray_slope: OKHSL_GRAY_EDGES.rounded_chroma(l, 2),
            mid: OKHSL_MID_SCALE * bulge * mid_edges.rounded_chroma(l, 4),
            max,
        }
    }

    fn lower(self) -> Ramp {
        Ramp {
            start_slope: OKHSL_MID_S * self.gray_slope,
            end: self.mid,
        }
    }

    fn upper(self) -> Ramp {
        let span = 1.0 - OKHSL_MID_S;
        Ramp {
            start_slope: self.lower().end_slope() * span / OKHSL_MID_S,
            end: self.max - self.mid,
        }
    }

    fn chroma(self, s: f32) -> f32 {
        if s < OKHSL_MID_S {
            self.lower().at(s / OKHSL_MID_S)
        } else {
            self.mid + self.upper().at((s - OKHSL_MID_S) / (1.0 - OKHSL_MID_S))
        }
    }

    fn saturation(self, c: f32) -> f32 {
        if c < self.mid {
            OKHSL_MID_S * self.lower().param_of(c)
        } else {
            OKHSL_MID_S + (1.0 - OKHSL_MID_S) * self.upper().param_of(c - self.mid)
        }
    }
}

fn okhsl_to_linear([h, s, l]: [f32; 3]) -> [f32; 3] {
    if l >= 1.0 {
        return [1.0; 3];
    }
    if l <= OKHSL_BLACK_L {
        return [0.0; 3];
    }
    let plane = HuePlane::at_turns(h);
    let lightness = lr_to_l(l);
    plane.linear(lightness, OkhslChromas::at(plane, lightness).chroma(s))
}

fn linear_to_okhsl(linear: [f32; 3]) -> [f32; 3] {
    let lab = linear_to_oklab(linear);
    let [lightness, c, hue] = oklab_to_oklch(lab);
    let l = l_to_lr(lightness);
    if c < ACHROMATIC_CHROMA {
        return [0.0, 0.0, l];
    }
    let plane = HuePlane::of_oklab(lab, c);
    [hue, OkhslChromas::at(plane, lightness).saturation(c), l]
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
    fn linear_to_color32_gamma_encodes_to_srgb_bytes() {
        use ecolor::Color32;
        assert_eq!(linear_to_color32([1.0, 0.0, 0.0]), Color32::RED);
        assert_eq!(linear_to_color32([0.5; 3]), Color32::from_gray(188));
        assert_eq!(
            linear_to_color32(srgb_to_linear(srgb_u8([51, 102, 153]))),
            Color32::from_rgb(51, 102, 153)
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

    const MIXED_BLEND_SPACES: [BlendSpace; 4] = [
        BlendSpace::Srgb,
        BlendSpace::LinearRgb,
        BlendSpace::Oklab,
        BlendSpace::Oklch,
    ];

    #[test]
    fn every_mixed_blend_space_round_trips_linear_colors() {
        for space in MIXED_BLEND_SPACES {
            for linear in linear_grid() {
                let blended = linear_to_blend(space, linear);
                let back = blend_to_linear(space, blended);
                for i in 0..3 {
                    assert!(
                        (back[i] - linear[i]).abs() < 1e-4,
                        "{space:?} {linear:?} -> {blended:?} -> {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn oklch_blend_reads_negative_chroma_as_gray() {
        let gray = blend_to_linear(BlendSpace::Oklch, [0.6, 0.0, 0.3]);
        assert_close(
            blend_to_linear(BlendSpace::Oklch, [0.6, -0.05, 0.3]),
            gray,
            1e-6,
        );
    }

    fn out_of_gamut_oklch() -> Vec<[f32; 3]> {
        let mut colors = Vec::new();
        for l in [0.05, 0.2, 0.45, 0.62, 0.8, 0.93] {
            for chroma in [0.33, 0.6] {
                for hue in (0..24).map(|i| i as f32 / 24.0) {
                    colors.push([l, chroma, hue]);
                }
            }
        }
        colors
    }

    fn assert_in_unit_cube(linear: [f32; 3], context: impl std::fmt::Debug) {
        assert!(
            linear.iter().all(|c| (0.0..=1.0).contains(c)),
            "{context:?}: {linear:?}"
        );
    }

    #[test]
    fn gamut_map_keeps_srgb_colors_bit_for_bit() {
        for linear in linear_grid() {
            assert_eq!(map_into_srgb_gamut(linear), linear);
        }
    }

    #[test]
    fn gamut_map_reduces_chroma_onto_the_srgb_surface_at_the_same_lightness_and_hue() {
        for lch in out_of_gamut_oklch() {
            let unmapped = oklab_to_linear(oklch_to_oklab(lch));
            assert!(!unmapped.iter().all(|c| (0.0..=1.0).contains(c)), "{lch:?}");
            let [unmapped_l, unmapped_chroma, unmapped_hue] =
                oklab_to_oklch(linear_to_oklab(unmapped));
            let mapped = map_into_srgb_gamut(unmapped);
            assert_in_unit_cube(mapped, lch);
            let [l, chroma, hue] = oklab_to_oklch(linear_to_oklab(mapped));
            assert!((l - unmapped_l).abs() < 1e-4, "{lch:?}: L {l}");
            assert!(chroma < unmapped_chroma, "{lch:?}: chroma {chroma}");
            assert!(
                crate::math::hue_abs_distance(hue, unmapped_hue) < 1e-3,
                "{lch:?}: hue {hue}"
            );
            let on_surface = mapped.iter().any(|c| !(1e-5..=1.0 - 1e-5).contains(c));
            assert!(on_surface, "{lch:?}: {mapped:?}");
        }
    }

    #[test]
    fn gamut_map_sends_lightness_above_one_to_white_and_below_zero_to_black() {
        for linear in [[1.5; 3], [2.0, 1.5, 1.0], [1.0, 1.0, 1.2]] {
            let mapped = map_into_srgb_gamut(linear);
            assert_in_unit_cube(mapped, linear);
            assert_close(mapped, [1.0; 3], 1e-6);
        }
        for linear in [[-0.2; 3], [-0.5, -0.1, 0.0], [0.0, 0.0, -1e-3]] {
            let mapped = map_into_srgb_gamut(linear);
            assert_in_unit_cube(mapped, linear);
            assert_close(mapped, [0.0; 3], 1e-6);
        }
    }

    #[test]
    fn gamut_map_of_grays_at_lightness_one_is_white_inside_the_cube() {
        for l in [1.0, 1.0 + 1e-6, 0.999_999] {
            let gray = oklab_to_linear([l, 0.0, 0.0]);
            let mapped = map_into_srgb_gamut(gray);
            assert_in_unit_cube(mapped, l);
            assert_close(mapped, [1.0; 3], 1e-5);
        }
    }

    #[test]
    fn picker_space_clip_toggle_exists_only_where_the_space_leaves_srgb() {
        for space in PickerSpace::ALL {
            let clipped = space.with_clip_to_srgb(true);
            assert_eq!(clipped.clips_to_srgb(), space.can_leave_srgb(), "{space:?}");
            assert!(clipped.is_same_space(space), "{space:?}");
            assert_eq!(clipped.with_clip_to_srgb(false), space, "{space:?}");
        }
        assert!(!OKLCH.is_same_space(OKLAB));
    }

    #[test]
    fn picker_space_serializes_old_names_unchanged_and_the_clip_toggle_with_its_space() {
        assert_eq!(
            serde_json::to_string(&PickerSpace::Okhsl).unwrap(),
            "\"Okhsl\""
        );
        let clipped = OKLCH.with_clip_to_srgb(true);
        let json = serde_json::to_string(&clipped).unwrap();
        assert_eq!(json, r#"{"Oklch":{"clip_to_srgb":true}}"#);
        assert_eq!(serde_json::from_str::<PickerSpace>(&json).unwrap(), clipped);
        assert_eq!(
            serde_json::from_str::<PickerSpace>(r#"{"Oklab":{}}"#).unwrap(),
            OKLAB
        );
    }

    #[test]
    fn z_on_bar_wraps_hues_and_clamps_oklab_lightness() {
        for space in hue_spaces() {
            assert_eq!(space.z_on_bar(-0.25), 0.75);
            assert_eq!(space.z_on_bar(1.25), 0.25);
            assert_eq!(space.z_on_bar(1.0), 0.0);
        }
        assert_eq!(OKLAB.z_on_bar(-0.25), 0.0);
        assert_eq!(OKLAB.z_on_bar(1.25), 1.0);
        assert_eq!(OKLAB.z_on_bar(1.0), 1.0);
        assert_eq!(OKLAB.z_on_bar(0.4), 0.4);
    }

    #[test]
    fn gamut_map_of_a_mapped_color_is_itself() {
        for lch in out_of_gamut_oklch() {
            let mapped = map_into_srgb_gamut(oklab_to_linear(oklch_to_oklab(lch)));
            assert_eq!(map_into_srgb_gamut(mapped), mapped, "{lch:?}");
        }
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
    fn oklch_to_oklab_inverts_oklab_to_oklch() {
        let lab = oklab_of_srgb([200, 40, 90]);
        assert_close(oklch_to_oklab(oklab_to_oklch(lab)), lab, 1e-6);
    }

    #[test]
    fn srgb_colors_land_in_the_unit_cube() {
        for space in PickerSpace::ALL {
            for linear in linear_grid() {
                let [h, s, y] = linear_to_picker(space, linear);
                if space.is_z_hue() {
                    assert!((0.0..1.0).contains(&h), "{space:?} {linear:?}: h {h}");
                }
                for c in [h, s, y] {
                    assert!(
                        (-1e-4..=1.0 + 1e-4).contains(&c),
                        "{space:?} {linear:?}: {s}, {y}"
                    );
                }
            }
        }
    }

    fn hue_spaces() -> impl Iterator<Item = PickerSpace> {
        PickerSpace::ALL
            .into_iter()
            .filter(|space| space.is_z_hue())
    }

    const OKLAB: PickerSpace = PickerSpace::Oklab {
        clip_to_srgb: false,
    };
    const OKLCH: PickerSpace = PickerSpace::Oklch {
        clip_to_srgb: false,
    };

    #[test]
    fn grays_have_zero_hue_and_saturation_whichever_space_made_them() {
        for from in hue_spaces() {
            for y in [0.0, 0.01, 0.37, 0.8, 1.0] {
                let gray = picker_to_linear(from, [0.37, 0.0, y]);
                for to in hue_spaces() {
                    let [h, s, _] = linear_to_picker(to, gray);
                    assert_eq!([h, s], [0.0, 0.0], "{from:?} y {y} -> {to:?}");
                }
                let [_, a, b] = linear_to_picker(OKLAB, gray);
                assert_close([a, b, 0.0], [0.5, 0.5, 0.0], 1e-4);
            }
        }
    }

    #[test]
    fn oklch_square_is_chroma_by_lightness_with_hue_on_the_bar() {
        let red = linear_to_picker(OKLCH, PRIMARIES[0]);
        assert_close(
            red,
            [29.2339 / 360.0, 0.257_683 / OK_SQUARE_CHROMA, 0.627_955],
            1e-4,
        );
        assert_close(picker_to_linear(OKLCH, red), PRIMARIES[0], 1e-4);
        assert_close(
            linear_to_oklab(picker_to_linear(OKLCH, [0.3, 0.0, 0.6])),
            [0.6, 0.0, 0.0],
            1e-5,
        );
    }

    #[test]
    fn oklab_square_is_a_by_b_with_lightness_on_the_bar() {
        let blue = linear_to_picker(OKLAB, PRIMARIES[2]);
        let to_square = |c: f32| 0.5 + c / (2.0 * OK_SQUARE_CHROMA);
        assert_close(
            blue,
            [0.452_014, to_square(-0.032_457), to_square(-0.311_528)],
            1e-4,
        );
        assert_close(picker_to_linear(OKLAB, blue), PRIMARIES[2], 1e-4);
        assert_close(
            linear_to_oklab(picker_to_linear(OKLAB, [0.6, 0.5, 0.5])),
            [0.6, 0.0, 0.0],
            1e-5,
        );
        assert_close(
            linear_to_oklab(picker_to_linear(OKLAB, [0.6, 1.0, 0.0])),
            [0.6, OK_SQUARE_CHROMA, -OK_SQUARE_CHROMA],
            1e-5,
        );
    }

    #[test]
    fn oklch_and_oklab_round_trip_picker_values_inside_and_outside_srgb() {
        for space in [OKLCH, OKLAB] {
            for z in [0.05, 0.3, 0.55, 0.8, 0.95] {
                for x in [0.1, 0.3, 0.5, 0.7, 1.0] {
                    for y in [0.05, 0.2, 0.5, 0.8, 1.0] {
                        let linear = picker_to_linear(space, [z, x, y]);
                        let tolerance = if is_in_srgb_gamut(linear) { 1e-4 } else { 1e-3 };
                        let back = linear_to_picker(space, linear);
                        assert_close(back, [z, x, y], tolerance);
                    }
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
    fn values_next_to_black_convert_to_finite_black_in_every_space() {
        let tiny_ys = [
            f32::from_bits(1),
            f32::MIN_POSITIVE,
            1e-30,
            1e-20,
            1e-10,
            1e-9,
            1e-8,
            1e-7,
        ];
        let black_at_y_zero = PickerSpace::ALL
            .into_iter()
            .filter(|space| !space.can_leave_srgb());
        for from in black_at_y_zero {
            for y in tiny_ys {
                for h in (0..=36).map(|i| i as f32 / 36.0) {
                    for s in [0.0, 0.3, 0.79, 0.8, 0.95, 1.0] {
                        let linear = picker_to_linear(from, [h, s, y]);
                        assert!(
                            linear.iter().all(|c| c.abs() < 1e-6),
                            "{from:?} [{h}, {s}, {y}]: {linear:?}"
                        );
                        for to in PickerSpace::ALL {
                            let back = linear_to_picker(to, linear);
                            assert!(
                                back.iter().all(|c| c.is_finite()),
                                "{from:?} [{h}, {s}, {y}] -> {to:?}: {back:?}"
                            );
                        }
                    }
                }
            }
        }
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
    fn okhsv_and_okhsl_reproduce_recorded_values_from_linear() {
        let linear = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.2, 0.5, 0.8],
            [0.8, 0.2, 0.03],
            [0.002, 0.002, 0.03],
            [0.97, 0.8, 0.5],
            [0.2, 0.2, 0.2],
            [0.001, 0.0, 1.0],
            [0.0, 0.001, 1.0],
            [0.000_1, 0.0, 0.001],
        ];
        let okhsv = [
            [0.081_205_234, 0.999_999_6, 1.000_000_2],
            [0.395_820_3, 0.999_997_9, 0.999_999_8],
            [0.733_477_83, 1.0, 0.999_999_94],
            [0.304_914_53, 1.000_000_7, 1.000_000_1],
            [0.664_781_87, 0.478_741_17, 0.914_341_1],
            [0.142_707_87, 0.862_772_17, 0.912_999_9],
            [0.759_291_8, 0.882_850_8, 0.163_592_89],
            [0.238_234_45, 0.215_706_44, 0.988_142_7],
            [0.0, 0.0, 0.518_360_1],
            [0.734_082_1, 1.0, 0.999_999_94],
            [0.733_526_95, 0.998_263_9, 1.0],
            [0.781_116_5, 1.000_000_1, 0.026_201_444],
        ];
        let okhsl = [
            [0.081_205_234, 1.000_000_1, 0.568_084_66],
            [0.395_820_3, 0.999_999_5, 0.844_528_9],
            [0.733_477_83, 1.0, 0.366_565_35],
            [0.304_914_53, 1.000_000_2, 0.962_704_36],
            [0.664_781_87, 0.634_880_54, 0.728_885_53],
            [0.142_707_87, 0.871_654_7, 0.644_672_4],
            [0.759_291_8, 0.906_801_8, 0.071_106_44],
            [0.238_234_45, 0.792_122_1, 0.923_288_4],
            [0.0, 0.0, 0.518_359_9],
            [0.734_082_1, 1.0, 0.367_082_27],
            [0.733_526_95, 1.0, 0.367_772_94],
            [0.781_116_5, 1.0, 0.010_860_868],
        ];
        for ((linear, okhsv), okhsl) in linear.into_iter().zip(okhsv).zip(okhsl) {
            assert_close(linear_to_picker(PickerSpace::Okhsv, linear), okhsv, 1e-5);
            assert_close(linear_to_picker(PickerSpace::Okhsl, linear), okhsl, 1e-5);
        }
    }

    #[test]
    fn okhsv_and_okhsl_reproduce_recorded_values_to_linear() {
        let picked = [
            [0.0, 0.6, 0.5],
            [0.166_666_67, 1.0, 0.75],
            [0.25, 0.81, 0.25],
            [0.5, 0.4, 0.9],
            [0.625, 0.79, 0.1],
            [0.75, 0.2, 0.01],
            [0.916_666_7, 0.9, 0.5],
            [0.733_377_8, 1.0, 0.5],
            [0.733_477_83, 1.0, 0.5],
            [0.733_577_85, 1.0, 0.5],
        ];
        let from_okhsv = [
            [0.198_204_95, 0.042_400_97, 0.080_857_17],
            [0.499_470_95, 0.138_588_32, 0.0],
            [0.045_559_38, 0.029_042_974, 0.002_661_552_3],
            [0.251_365, 0.765_741_35, 0.637_894_9],
            [0.000_700_353_2, 0.006_248_573, 0.009_781_081],
            [6.031_887e-5, 6.944_004_4e-5, 9.956_257_4e-5],
            [0.202_493_03, 0.010_887_202, 0.187_920_99],
            [0.0, 0.009_387_426, 0.229_647_46],
            [0.0, 0.0, 0.236_102_48],
            [3.900_751_5e-5, 0.0, 0.236_089_6],
        ];
        let from_okhsl = [
            [0.457_966_77, 0.080_632_67, 0.173_722_57],
            [0.999_999_76, 0.346_061_14, 0.060_888_53],
            [0.062_535_88, 0.039_643_68, 0.003_302_786_5],
            [0.596_745_55, 0.828_321_93, 0.770_853_9],
            [0.000_780_519_56, 0.009_931_075, 0.015_867_969],
            [7.986_945e-5, 9.678_812e-5, 0.000_156_971_86],
            [0.515_329_5, 0.014_381_923, 0.479_517_46],
            [0.022_931_725, 0.131_387_6, 1.000_002],
            [0.023_161_933, 0.131_292_15, 1.000_007_6],
            [0.023_392_767, 0.131_197_36, 1.000_007_6],
        ];
        for ((picked, okhsv), okhsl) in picked.into_iter().zip(from_okhsv).zip(from_okhsl) {
            assert_close(picker_to_linear(PickerSpace::Okhsv, picked), okhsv, 1e-5);
            assert_close(picker_to_linear(PickerSpace::Okhsl, picked), okhsl, 1e-5);
        }
    }
    #[test]
    fn okhsv_and_okhsl_round_trip_picker_values() {
        for space in [PickerSpace::Okhsv, PickerSpace::Okhsl] {
            for h in (0..24).map(|i| (i as f32 + 0.5) / 24.0) {
                for s in [0.1, 0.3, 0.5, 0.79, 0.8, 0.81, 0.95, 1.0] {
                    for y in [0.05, 0.2, 0.4, 0.6, 0.8, 0.9] {
                        let back = linear_to_picker(space, picker_to_linear(space, [h, s, y]));
                        assert_close(back, [h, s, y], 1e-4);
                    }
                }
            }
        }
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

    #[test]
    fn hsv_of_grays_has_zero_hue_and_saturation() {
        assert_eq!(srgb_to_hsv([0.0; 3]), [0.0; 3]);
        assert_eq!(srgb_to_hsv([1.0; 3]), [0.0, 0.0, 1.0]);
        assert_eq!(srgb_to_hsv([0.5; 3]), [0.0, 0.0, 0.5]);
    }
}
