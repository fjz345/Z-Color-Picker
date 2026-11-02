use std::f32::consts::TAU;

use bspline::Interpolate;
use ecolor::Color32;
use eframe::egui::lerp;
use palette::{FromColor, LabHue, Lch, LinSrgb};

pub fn hue_abs_distance(hue0: f32, hue1: f32) -> f32 {
    let d = (hue1 - hue0).rem_euclid(1.0);
    d.min(1.0 - d)
}

pub fn hue_lerp(hue0: f32, hue1: f32, t: f32) -> f32 {
    let hue_dist = hue_abs_distance(hue0, hue1);
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

pub fn color_lerp_ex(
    color_src: Color32,
    color_trg: Color32,
    mut t: f32,
    c: f32,
    _alpha: f32,
) -> Color32 {
    if t < 0.0 || t > 1.0 {
        log::info!("t value {} is not a valid input", t);
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

    log::info!("Prev_src_hue {}", lch_src.chroma);
    log::info!(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_abs_distance_is_wrap_safe_beyond_one_turn() {
        assert!((hue_abs_distance(0.1, 1.6) - 0.5).abs() < 1e-5);
        assert!((hue_abs_distance(0.1, 2.2) - 0.1).abs() < 1e-5);
        assert!((hue_abs_distance(0.9, -1.05) - 0.05).abs() < 1e-5);
    }

    #[test]
    fn hue_abs_distance_takes_shortest_way_around() {
        assert!((hue_abs_distance(0.9, 0.1) - 0.2).abs() < 1e-5);
        assert_eq!(hue_abs_distance(0.2, 0.2), 0.0);
        assert_eq!(hue_abs_distance(0.0, 0.5), 0.5);
    }

    #[test]
    fn hue_lerp_follows_t_beyond_one_turn() {
        assert!((hue_lerp(0.1, 1.3, 0.5) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn hue_lerp_wraps_across_zero() {
        assert!((hue_lerp(0.9, 0.1, 0.25) - 0.95).abs() < 1e-5);
        let midpoint = hue_lerp(0.9, 0.1, 0.5);
        assert!(midpoint < 1e-5 || midpoint > 1.0 - 1e-5);
    }

    #[test]
    fn hue_lerp_hits_endpoints() {
        assert_eq!(hue_lerp(0.2, 0.6, 0.0), 0.2);
        assert!((hue_lerp(0.2, 0.6, 1.0) - 0.6).abs() < 1e-5);
    }
}
