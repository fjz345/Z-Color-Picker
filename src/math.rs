use std::f32::consts::TAU;

use bspline::Interpolate;
use ecolor::Color32;
use eframe::egui::{lerp, Vec2};
use palette::{FromColor, LabHue, Lch, LinSrgb};

pub fn factorial(n: u64) -> u64 {
    (1..=n).product()
}

// n! / (n - r)!

pub fn combination(n: u64, r: u64) -> u64 {
    factorial(n) / factorial(n - r)
}

pub fn count_combinations(n: u64, r: u64) -> u64 {
    if r > n {
        0
    } else {
        (1..=r.min(n - r)).fold(1, |acc, val| acc * (n - val + 1) / val)
    }
}

pub fn count_permutations(n: u64, r: u64) -> u64 {
    (n - r + 1..=n).product()
}

pub fn mul_array<const D: usize, T: std::ops::MulAssign + std::marker::Copy>(
    mut lhs: [T; D],
    rhs: T,
) -> [T; D] {
    for i in 0..D {
        lhs[i] *= rhs;
    }
    lhs
}

pub fn add_array<const D: usize, T: std::ops::AddAssign + std::marker::Copy>(
    mut lhs: [T; D],
    rhs: T,
) -> [T; D] {
    for i in 0..D {
        lhs[i] += rhs;
    }
    lhs
}

pub fn add_array_array<const D: usize, T: std::ops::AddAssign + std::marker::Copy>(
    mut lhs: [T; D],
    rhs: [T; D],
) -> [T; D] {
    for i in 0..D {
        lhs[i] += rhs[i];
    }
    lhs
}

pub fn dist_vec2(vec: &[f32; 2]) -> f32 {
    (vec[0] * vec[0] + vec[1] * vec[1]).sqrt()
}

pub fn norm_vec2(vec: &[f32; 2]) -> Vec2 {
    let dist = dist_vec2(vec);
    Vec2::new(vec[0] / dist, vec[1] / dist)
}

pub fn hue_distance(hue0: f32, hue1: f32) -> f32 {
    (hue1 - hue0).abs().min(1.0 - (hue1 - hue0).abs())
}

pub fn hue_lerp(hue0: f32, hue1: f32, t: f32) -> f32 {
    let hue_dist = hue_distance(hue0, hue1);
    if hue_dist <= 0.0001 {
        return hue0;
    }
    let dist_right = (hue1 - hue0).rem_euclid(1.0);
    let dist_left = (hue0 - hue1).rem_euclid(1.0);
    let closest_right = dist_right <= dist_left;

    let to_right = closest_right as i8 * 2 - 1;
    let hue_diff = hue_dist * to_right as f32;

    let target_hue = hue0 + hue_diff;
    let hue = lerp(hue0..=target_hue, t);

    hue.rem_euclid(1.0)
}

pub fn color_lerp(color_src: Color32, color_trg: Color32, t: f32) -> Color32 {
    const C: f32 = 0.7;
    const ALPHA: f32 = 0.1;
    color_lerp_ex(color_src, color_trg, t, C, ALPHA)
}

pub fn color_lerp_ex(
    color_src: Color32,
    color_trg: Color32,
    mut t: f32,
    c: f32,
    _alpha: f32,
) -> Color32 {
    if t < 0.0 || t > 1.0 {
        println!("t value {} is not a valid input", t);
        t = t.clamp(0.0, 1.0);
    }

    let color_src_linsrgb = LinSrgb::new(
        color_src.r() as f32 / 255.0,
        color_src.g() as f32 / 255.0,
        color_src.b() as f32 / 255.0,
    );
    let color_trg_linsrgb = LinSrgb::new(
        color_trg.r() as f32 / 255.0,
        color_trg.g() as f32 / 255.0,
        color_trg.b() as f32 / 255.0,
    );
    let lch_src = Lch::from_color(color_src_linsrgb);
    let lch_trg = Lch::from_color(color_trg_linsrgb);

    // Lerp hue
    let lerped_hue_normalized = hue_lerp(
        f32::to_radians(Into::<f32>::into(lch_src.hue)) / TAU,
        f32::to_radians(Into::<f32>::into(lch_trg.hue)) / TAU,
        t,
    );
    let new_hue = LabHue::new(f32::to_degrees(lerped_hue_normalized * TAU));
    // desaturate towards C (t<= 0.5), t> 0.5, mirror
    let new_chroma_normalized = if t <= 0.5 {
        (lch_src.chroma as f32 / Lch::<f32>::max_chroma() as f32).interpolate(&(1.0 - c), t * 2.0)
    } else {
        (1.0 - c).interpolate(
            &(lch_trg.chroma as f32 / Lch::<f32>::max_chroma()),
            -1.0 + t * 2.0,
        )
    };

    println!("Prev_src_hue {}", lch_src.chroma);
    println!(
        "new_hue {} (LAB){}",
        lerped_hue_normalized,
        new_hue.into_degrees()
    );

    let new_lch: Lch = Lch::new(
        lch_src.l,
        new_chroma_normalized * Lch::<f32>::max_chroma(),
        lerped_hue_normalized,
    );
    let new_color = LinSrgb::from_color(new_lch);
    Color32::from_rgb(
        (new_color.red * 255.0) as u8,
        (new_color.green * 255.0) as u8,
        (new_color.blue * 255.0) as u8,
    )
}
