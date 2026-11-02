use std::ops::{Add, Mul, Neg, Sub};

use crate::datatypes::control_point::{
    create_tangent_for_control_point, ControlPoint, ControlPointValue,
};
use crate::ui_egui::control_points::bezier_handle;

const INCOMING: usize = 0;
const OUTGOING: usize = 1;
const MIN_HANDLE_LENGTH: f64 = 1.0e-3;
// Well above MIN_HANDLE_LENGTH, so f32 rounding can't turn a solved handle into a zero one.
const MIN_SOLVED_LENGTH: f64 = 5.0e-3;
const ALIGNED_SINE: f64 = 1.0e-4;
const CURVATURE_TOLERANCE: f64 = 1.0e-3;
const STORAGE_ROUNDING: f64 = 1.0e-6;
const ROOT_SCAN_STEPS: usize = 256;
const BISECTION_STEPS: usize = 60;
const NEWTON_STEPS: usize = 16;
const MAX_PASSES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
struct V2 {
    x: f64,
    y: f64,
}

impl V2 {
    const X: V2 = V2 { x: 1.0, y: 0.0 };

    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn cross(self, other: V2) -> f64 {
        self.x * other.y - self.y * other.x
    }

    fn dot(self, other: V2) -> f64 {
        self.x * other.x + self.y * other.y
    }

    fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    fn unit(self) -> Option<V2> {
        let length = self.length();
        (length.is_finite() && length > f64::EPSILON).then(|| self * (1.0 / length))
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, other: V2) -> V2 {
        V2::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, other: V2) -> V2 {
        V2::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2::new(-self.x, -self.y)
    }
}

impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, factor: f64) -> V2 {
        V2::new(self.x * factor, self.y * factor)
    }
}

fn sv(value: ControlPointValue) -> V2 {
    V2::new(value[0] as f64, value[1] as f64)
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    start: V2,
    end: V2,
    start_tangent: V2,
    end_tangent: V2,
}

impl Segment {
    // From k = (2/3) (B - A) x (C - B) / |B - A|^3: k0 h0^2 + beta h1 = gamma0, k1 h1^2 + beta h0 = gamma1.
    fn coefficients(&self) -> (f64, f64, f64) {
        let beta = 2.0 / 3.0 * self.start_tangent.cross(self.end_tangent);
        let gamma0 = 2.0 / 3.0 * self.start_tangent.cross(self.end - self.start);
        let gamma1 = 2.0 / 3.0 * self.end_tangent.cross(self.start - self.end);
        (beta, gamma0, gamma1)
    }

    fn curvatures(&self, lengths: (f64, f64)) -> Option<(f64, f64)> {
        let (h0, h1) = lengths;
        if h0 < MIN_HANDLE_LENGTH || h1 < MIN_HANDLE_LENGTH {
            return None;
        }
        let (beta, gamma0, gamma1) = self.coefficients();
        let k0 = (gamma0 - beta * h1) / (h0 * h0);
        let k1 = (gamma1 - beta * h0) / (h1 * h1);
        (k0.is_finite() && k1.is_finite()).then_some((k0, k1))
    }

    // First-order change of each endpoint curvature when every point and handle moves by STORAGE_ROUNDING.
    fn rounding_slack(&self, lengths: (f64, f64), curvatures: (f64, f64)) -> (f64, f64) {
        let chord = (self.end - self.start).length();
        let slack = |near: f64, far: f64, k: f64| {
            STORAGE_ROUNDING
                * (4.0 / 3.0 * (chord + 3.0 * near + far) / near.powi(3) + 6.0 * k.abs() / near)
        };
        (
            slack(lengths.0, lengths.1, curvatures.0),
            slack(lengths.1, lengths.0, curvatures.1),
        )
    }
}

fn is_inside_square(point: V2) -> bool {
    (0.0..=1.0).contains(&point.x) && (0.0..=1.0).contains(&point.y)
}

fn max_length_in_square(point: V2, direction: V2) -> f64 {
    if !is_inside_square(point) {
        return 0.0;
    }
    [(point.x, direction.x), (point.y, direction.y)]
        .into_iter()
        .map(|(p, d)| match d {
            d if d > 0.0 => (1.0 - p) / d,
            d if d < 0.0 => -p / d,
            _ => f64::INFINITY,
        })
        .fold(f64::INFINITY, f64::min)
}

fn fits(
    segment: &Segment,
    lengths: (f64, f64),
    targets: (Option<f64>, Option<f64>),
    max_lengths: (f64, f64),
) -> bool {
    let meets = |k: f64, slack: f64, target: Option<f64>| {
        target.is_none_or(|target| {
            (k - target).abs() <= CURVATURE_TOLERANCE * target.abs().max(1.0) + slack
        })
    };
    lengths.0 <= max_lengths.0 + STORAGE_ROUNDING
        && lengths.1 <= max_lengths.1 + STORAGE_ROUNDING
        && segment.curvatures(lengths).is_some_and(|k| {
            let slack = segment.rounding_slack(lengths, k);
            meets(k.0, slack.0, targets.0) && meets(k.1, slack.1, targets.1)
        })
}

fn bisect(mut low: f64, low_value: f64, mut high: f64, f: &impl Fn(f64) -> Option<f64>) -> f64 {
    for _ in 0..BISECTION_STEPS {
        let middle = 0.5 * (low + high);
        let Some(value) = f(middle) else {
            break;
        };
        if value == 0.0 {
            return middle;
        }
        if (value < 0.0) == (low_value < 0.0) {
            low = middle;
        } else {
            high = middle;
        }
    }
    0.5 * (low + high)
}

fn sign_change_roots(low: f64, high: f64, f: impl Fn(f64) -> Option<f64>) -> Vec<f64> {
    let mut roots = Vec::new();
    if high <= low {
        return roots;
    }
    let step = (high - low) / ROOT_SCAN_STEPS as f64;
    let mut previous: Option<(f64, f64)> = None;
    for i in 0..=ROOT_SCAN_STEPS {
        let x = low + step * i as f64;
        let value = f(x);
        if let Some(v) = value {
            let crossing = previous.filter(|&(_, pv)| pv != 0.0 && (pv < 0.0) != (v < 0.0));
            if v == 0.0 {
                roots.push(x);
            } else if let Some((px, pv)) = crossing {
                roots.push(bisect(px, pv, x, &f));
            }
        }
        previous = value.map(|v| (x, v));
    }
    roots
}

// Solves the first equation for h_a and scans the second over h_b; the swapped call covers k_a near 0.
fn scanned_solutions(
    k_a: f64,
    gamma_a: f64,
    k_b: f64,
    gamma_b: f64,
    beta: f64,
    max_b: f64,
) -> Vec<(f64, f64)> {
    if k_a == 0.0 {
        return Vec::new();
    }
    let length_a = |h_b: f64| {
        let squared = (gamma_a - beta * h_b) / k_a;
        (squared >= 0.0).then(|| squared.sqrt())
    };
    let residual = |h_b: f64| length_a(h_b).map(|h_a| k_b * h_b * h_b + beta * h_a - gamma_b);
    sign_change_roots(MIN_SOLVED_LENGTH, max_b, residual)
        .into_iter()
        .filter_map(|h_b| length_a(h_b).map(|h_a| (h_a, h_b)))
        .collect()
}

fn newton_polished(lengths: (f64, f64), targets: (f64, f64), segment: &Segment) -> (f64, f64) {
    let (beta, gamma0, gamma1) = segment.coefficients();
    let (k0, k1) = targets;
    let (mut h0, mut h1) = lengths;
    for _ in 0..NEWTON_STEPS {
        let f0 = k0 * h0 * h0 + beta * h1 - gamma0;
        let f1 = k1 * h1 * h1 + beta * h0 - gamma1;
        let (j00, j11) = (2.0 * k0 * h0, 2.0 * k1 * h1);
        let determinant = j00 * j11 - beta * beta;
        let step0 = (j11 * f0 - beta * f1) / determinant;
        let step1 = (j00 * f1 - beta * f0) / determinant;
        if !(step0.is_finite() && step1.is_finite()) {
            break;
        }
        h0 -= step0;
        h1 -= step1;
    }
    (h0, h1)
}

fn two_sided_solutions(
    segment: &Segment,
    targets: (f64, f64),
    max_lengths: (f64, f64),
) -> Vec<(f64, f64)> {
    let (beta, gamma0, gamma1) = segment.coefficients();
    let (k0, k1) = targets;
    let mut solutions = scanned_solutions(k0, gamma0, k1, gamma1, beta, max_lengths.1);
    solutions.extend(
        scanned_solutions(k1, gamma1, k0, gamma0, beta, max_lengths.0)
            .into_iter()
            .map(|(h1, h0)| (h0, h1)),
    );
    if beta != 0.0 {
        let straight_start = gamma0 / beta;
        let straight_end = gamma1 / beta;
        for lengths in [
            (
                (gamma1 - k1 * straight_start * straight_start) / beta,
                straight_start,
            ),
            (
                straight_end,
                (gamma0 - k0 * straight_end * straight_end) / beta,
            ),
        ] {
            solutions.push(newton_polished(lengths, targets, segment));
        }
    }
    solutions
}

// Points on k_a h_a^2 + beta h_b = gamma_a, the one equation of a segment with a free end b.
fn one_sided_solutions(
    k_a: f64,
    gamma_a: f64,
    beta: f64,
    max_lengths: (f64, f64),
    current: (f64, f64),
) -> Vec<(f64, f64)> {
    let length_a = |h_b: f64| {
        let squared = (gamma_a - beta * h_b) / k_a;
        (k_a != 0.0 && squared >= 0.0).then(|| squared.sqrt())
    };
    let length_b = |h_a: f64| (beta != 0.0).then(|| (gamma_a - k_a * h_a * h_a) / beta);
    let samples = |max: f64| {
        (0..=ROOT_SCAN_STEPS).map(move |i| {
            MIN_SOLVED_LENGTH + (max - MIN_SOLVED_LENGTH) * i as f64 / ROOT_SCAN_STEPS as f64
        })
    };
    let by_b = std::iter::once(current.1)
        .chain(samples(max_lengths.1))
        .filter_map(|h_b| length_a(h_b).map(|h_a| (h_a, h_b)));
    let by_a = std::iter::once(current.0)
        .chain(samples(max_lengths.0))
        .filter_map(|h_a| length_b(h_a).map(|h_b| (h_a, h_b)));
    by_b.chain(by_a).collect()
}

fn solve_lengths(
    segment: &Segment,
    targets: (Option<f64>, Option<f64>),
    max_lengths: (f64, f64),
    current: (f64, f64),
) -> Option<(f64, f64)> {
    if fits(segment, current, targets, max_lengths) {
        return Some(current);
    }
    let (beta, gamma0, gamma1) = segment.coefficients();
    let swapped = |(a, b): (f64, f64)| (b, a);
    let candidates = match targets {
        (Some(k0), Some(k1)) => two_sided_solutions(segment, (k0, k1), max_lengths),
        (Some(k0), None) => one_sided_solutions(k0, gamma0, beta, max_lengths, current),
        (None, Some(k1)) => {
            one_sided_solutions(k1, gamma1, beta, swapped(max_lengths), swapped(current))
                .into_iter()
                .map(swapped)
                .collect()
        }
        (None, None) => Vec::new(),
    };
    let distance = |lengths: &(f64, f64)| (lengths.0 - current.0).hypot(lengths.1 - current.1);
    candidates
        .into_iter()
        .filter(|&lengths| {
            lengths.0.min(lengths.1) >= MIN_SOLVED_LENGTH
                && fits(segment, lengths, targets, max_lengths)
        })
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

fn is_aligned(a: V2, b: V2) -> bool {
    a.dot(b) > 0.0 && a.cross(b).abs() < ALIGNED_SINE
}

fn chord_direction(positions: &[V2], index: usize) -> V2 {
    let here = positions[index];
    let previous = positions[index.saturating_sub(1)];
    let next = positions[(index + 1).min(positions.len() - 1)];
    (next - previous)
        .unit()
        .or((next - here).unit())
        .or((here - previous).unit())
        .unwrap_or(V2::X)
}

fn handle_directions(
    positions: &[V2],
    index: usize,
    handles: [V2; 2],
    pinned_slot: Option<usize>,
) -> [V2; 2] {
    let own = handles.map(|handle| {
        (handle.length() >= MIN_HANDLE_LENGTH)
            .then(|| handle.unit())
            .flatten()
    });
    let fallback = chord_direction(positions, index);
    let is_interior = index > 0 && index + 1 < positions.len();
    if !is_interior || !is_inside_square(positions[index]) {
        return [
            own[INCOMING].unwrap_or(-fallback),
            own[OUTGOING].unwrap_or(fallback),
        ];
    }
    let pinned =
        pinned_slot.and_then(|slot| own[slot].map(|u| if slot == INCOMING { -u } else { u }));
    let outgoing = pinned
        .or_else(|| match own {
            [Some(incoming), Some(outgoing)] if is_aligned(outgoing, -incoming) => Some(outgoing),
            [Some(incoming), Some(outgoing)] => (outgoing - incoming).unit(),
            [Some(incoming), None] => Some(-incoming),
            [None, outgoing] => outgoing,
        })
        .unwrap_or(fallback);
    [-outgoing, outgoing]
}

fn circle_curvature(a: V2, b: V2, c: V2) -> Option<f64> {
    let denominator = (b - a).length() * (c - b).length() * (c - a).length();
    let k = 2.0 * (b - a).cross(c - a) / denominator;
    (denominator > f64::EPSILON && k.is_finite()).then_some(k)
}

fn joint_targets(
    positions: &[V2],
    curvatures: &[Option<(f64, f64)>],
    kept: &[bool],
) -> Vec<Option<f64>> {
    let last = positions.len() - 1;
    (0..=last)
        .map(|joint| {
            if joint == 0 || joint == last {
                return None;
            }
            let arriving = curvatures[joint - 1].map(|(_, k)| k);
            let leaving = curvatures[joint].map(|(k, _)| k);
            match ((arriving, kept[joint - 1]), (leaving, kept[joint])) {
                // A kept segment cannot change, so its neighbour matches it instead.
                ((Some(k), true), _) | (_, (Some(k), true)) => Some(k),
                _ => circle_curvature(positions[joint - 1], positions[joint], positions[joint + 1])
                    .or_else(|| match (arriving, leaving) {
                        (Some(a), Some(l)) => Some(0.5 * (a + l)),
                        (a, l) => a.or(l),
                    }),
            }
        })
        .collect()
}

fn is_continuous(arriving: (f64, f64), leaving: (f64, f64)) -> bool {
    let ((a, arriving_slack), (l, leaving_slack)) = (arriving, leaving);
    let scale = a.abs().max(l.abs()).max(1.0);
    (a - l).abs() <= 2.0 * CURVATURE_TOLERANCE * scale + arriving_slack + leaving_slack
}

fn working_length(handle: V2, chord: f64, max_length: f64) -> f64 {
    match handle.length() {
        length if length < MIN_HANDLE_LENGTH => (chord / 3.0).min(max_length),
        length if length > max_length + STORAGE_ROUNDING => max_length,
        length => length,
    }
}

fn set_handle(cp: &mut ControlPoint, slot: usize, current: V2, direction: V2, length: f64) -> bool {
    let position = sv(*cp.val());
    let current_length = current.length();
    let is_zero = length < MIN_HANDLE_LENGTH;
    let is_unchanged = if current_length < MIN_HANDLE_LENGTH {
        is_zero
    } else {
        length == current_length && current.unit().is_some_and(|u| is_aligned(u, direction))
    };
    if !is_inside_square(position) || is_unchanged {
        return false;
    }
    let inside_length = (max_length_in_square(position, direction) - STORAGE_ROUNDING).max(0.0);
    let offset = if is_zero {
        V2::new(0.0, 0.0)
    } else {
        direction * length.min(inside_length)
    };
    let tangent = &mut cp.tangents_mut()[slot];
    let stored = tangent.map(|t| [t[0], t[1]]);
    let written = [offset.x as f32, offset.y as f32];
    let tangent = tangent.get_or_insert_with(create_tangent_for_control_point);
    tangent[0] = written[0];
    tangent[1] = written[1];
    stored != Some(written)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct AutoComb {
    unsolved_joints: usize,
    is_changed: bool,
}

// A handle that lands on the square's border can need a second pass to settle.
pub fn apply_auto_comb(points: &mut [ControlPoint], pinned: Option<(usize, usize)>) -> usize {
    let mut result = AutoComb::default();
    for _ in 0..MAX_PASSES {
        result = auto_comb(points, pinned);
        if !result.is_changed {
            break;
        }
    }
    result.unsolved_joints
}

fn auto_comb(points: &mut [ControlPoint], pinned: Option<(usize, usize)>) -> AutoComb {
    if points.len() < 2 {
        return AutoComb::default();
    }
    let positions: Vec<V2> = points.iter().map(|cp| sv(*cp.val())).collect();
    let handles: Vec<[V2; 2]> = points
        .iter()
        .zip(&positions)
        .map(|(cp, &position)| {
            [INCOMING, OUTGOING].map(|slot| sv(bezier_handle(cp, slot)) - position)
        })
        .collect();
    let directions: Vec<[V2; 2]> = (0..points.len())
        .map(|i| {
            let pinned_slot = pinned
                .filter(|&(index, _)| index == i)
                .map(|(_, slot)| slot);
            handle_directions(&positions, i, handles[i], pinned_slot)
        })
        .collect();

    let segment_count = points.len() - 1;
    let segments: Vec<Segment> = (0..segment_count)
        .map(|i| Segment {
            start: positions[i],
            end: positions[i + 1],
            start_tangent: directions[i][OUTGOING],
            end_tangent: -directions[i + 1][INCOMING],
        })
        .collect();
    let max_lengths: Vec<(f64, f64)> = (0..segment_count)
        .map(|i| {
            (
                max_length_in_square(positions[i], directions[i][OUTGOING]),
                max_length_in_square(positions[i + 1], directions[i + 1][INCOMING]),
            )
        })
        .collect();
    let cap = |point: V2, max_length: f64| {
        if is_inside_square(point) {
            max_length
        } else {
            f64::INFINITY
        }
    };
    let current: Vec<(f64, f64)> = (0..segment_count)
        .map(|i| {
            let chord = (positions[i + 1] - positions[i]).length();
            (
                working_length(
                    handles[i][OUTGOING],
                    chord,
                    cap(positions[i], max_lengths[i].0),
                ),
                working_length(
                    handles[i + 1][INCOMING],
                    chord,
                    cap(positions[i + 1], max_lengths[i].1),
                ),
            )
        })
        .collect();
    let curvatures: Vec<Option<(f64, f64)>> = segments
        .iter()
        .zip(&current)
        .map(|(segment, &lengths)| segment.curvatures(lengths))
        .collect();

    let mut kept = vec![false; segment_count];
    let mut lengths = current.clone();
    loop {
        let targets = joint_targets(&positions, &curvatures, &kept);
        let mut is_kept_set_grown = false;
        for i in 0..segment_count {
            if kept[i] {
                continue;
            }
            let solved = solve_lengths(
                &segments[i],
                (targets[i], targets[i + 1]),
                max_lengths[i],
                current[i],
            );
            lengths[i] = solved.unwrap_or(current[i]);
            if solved.is_none() {
                kept[i] = true;
                is_kept_set_grown = true;
            }
        }
        if !is_kept_set_grown {
            break;
        }
    }

    let mut is_changed = false;
    for (i, &(start_length, end_length)) in lengths.iter().enumerate() {
        is_changed |= set_handle(
            &mut points[i],
            OUTGOING,
            handles[i][OUTGOING],
            directions[i][OUTGOING],
            start_length,
        );
        is_changed |= set_handle(
            &mut points[i + 1],
            INCOMING,
            handles[i + 1][INCOMING],
            directions[i + 1][INCOMING],
            end_length,
        );
    }
    let with_slack = |i: usize| {
        segments[i]
            .curvatures(lengths[i])
            .map(|k| (k, segments[i].rounding_slack(lengths[i], k)))
    };
    let unsolved_joints = (1..segment_count)
        .filter(|&joint| {
            let arriving = with_slack(joint - 1).map(|(k, slack)| (k.1, slack.1));
            let leaving = with_slack(joint).map(|(k, slack)| (k.0, slack.0));
            let is_smooth = is_aligned(directions[joint][OUTGOING], -directions[joint][INCOMING]);
            !(is_smooth
                && arriving
                    .zip(leaving)
                    .is_some_and(|(a, l)| is_continuous(a, l)))
        })
        .count();
    AutoComb {
        unsolved_joints,
        is_changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::test_support::{
        point_with_tangent, RUNAWAY_POINT, RUNAWAY_TANGENT,
    };

    fn point(position: [f32; 2], incoming: [f32; 2], outgoing: [f32; 2]) -> ControlPoint {
        let mut cp =
            ControlPoint::new_simple(ControlPointValue::new(position[0], position[1], 0.5), 0.0);
        *cp.tangents_mut() =
            [incoming, outgoing].map(|[s, v]| Some(ControlPointValue::new(s, v, 0.0)));
        cp
    }

    fn untangented(position: [f32; 2]) -> ControlPoint {
        ControlPoint::new_simple(ControlPointValue::new(position[0], position[1], 0.2), 0.0)
    }

    fn along(direction: [f32; 2], length: f32) -> [f32; 2] {
        let norm = direction[0].hypot(direction[1]);
        [direction[0] / norm * length, direction[1] / norm * length]
    }

    fn mirrored(
        point_position: [f32; 2],
        outgoing: [f32; 2],
        incoming_length: f32,
    ) -> ControlPoint {
        let back = along([-outgoing[0], -outgoing[1]], incoming_length);
        point(point_position, back, outgoing)
    }

    fn arc(
        center: [f32; 2],
        radius: f32,
        degrees_and_lengths: &[(f32, [f32; 2])],
    ) -> Vec<ControlPoint> {
        degrees_and_lengths
            .iter()
            .map(|&(degrees, [incoming, outgoing])| {
                let angle = degrees.to_radians();
                let position = [
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                ];
                let travel = [-angle.sin(), angle.cos()];
                mirrored(position, along(travel, outgoing), incoming)
            })
            .collect()
    }

    fn c_shape() -> Vec<ControlPoint> {
        arc(
            [0.5, 0.5],
            0.35,
            &[
                (200.0, [0.02, 0.15]),
                (235.0, [0.04, 0.12]),
                (270.0, [0.15, 0.03]),
                (305.0, [0.06, 0.1]),
                (340.0, [0.1, 0.05]),
            ],
        )
    }

    fn near_border_arc() -> Vec<ControlPoint> {
        arc(
            [0.5, 0.5],
            0.47,
            &[
                (190.0, [0.3, 0.3]),
                (240.0, [0.3, 0.3]),
                (290.0, [0.3, 0.3]),
                (340.0, [0.3, 0.3]),
            ],
        )
    }

    fn s_shape() -> Vec<ControlPoint> {
        vec![
            mirrored([0.1, 0.5], along([1.0, 1.96], 0.05), 0.05),
            mirrored([0.3, 0.75], along([1.0, 0.0], 0.12), 0.06),
            mirrored([0.5, 0.5], along([1.0, -1.96], 0.04), 0.09),
            mirrored([0.7, 0.25], along([1.0, 0.0], 0.1), 0.05),
            mirrored([0.9, 0.5], along([1.0, 1.96], 0.06), 0.06),
        ]
    }

    fn ellipse_without_tangents() -> Vec<ControlPoint> {
        (0..8)
            .map(|i| {
                let angle = (100.0 + 40.0 * i as f32).to_radians();
                untangented([0.5 + 0.4 * angle.cos(), 0.5 + 0.3 * angle.sin()])
            })
            .collect()
    }

    fn kinked() -> Vec<ControlPoint> {
        vec![
            point([0.2, 0.3], [-0.1, 0.0], [0.1, 0.1]),
            point([0.5, 0.5], [-0.1, 0.0], [0.0, 0.1]),
            point([0.8, 0.8], [-0.1, -0.05], [0.1, 0.0]),
        ]
    }

    fn degenerate_cases() -> Vec<Vec<ControlPoint>> {
        vec![
            vec![],
            vec![point([0.5, 0.5], [0.0; 2], [0.0; 2])],
            vec![point([0.5, 0.5], [0.0; 2], [0.0; 2]); 4],
            vec![untangented([0.3, 0.3]); 3],
            (0..4)
                .map(|i| point([0.2 + 0.2 * i as f32, 0.5], [0.0; 2], [0.0; 2]))
                .collect(),
            (1..4)
                .map(|i| point([0.25 * i as f32, 0.5], [-0.1, 0.0], [0.1, 0.0]))
                .collect(),
            vec![
                point([0.2, 0.8], [0.0, 0.1], [0.0, -0.2]),
                point([0.5, 0.2], [-0.1, 0.0], [0.1, 0.0]),
                point([0.8, 0.8], [0.0, -0.2], [0.0, 0.1]),
            ],
            vec![
                point([1.0, 0.5], [0.0; 2], [0.3, 0.0]),
                point([1.0, 1.0], [0.2, 0.0], [0.0, 0.2]),
                point([0.0, 1.0], [-0.3, 0.0], [0.0; 2]),
            ],
            vec![
                point_with_tangent(RUNAWAY_POINT, 1, RUNAWAY_TANGENT),
                point_with_tangent([0.775, 0.592, 0.604], 0, [-0.175, 0.315, 0.0]),
                point([0.6, 0.2], [0.05, 0.05], [-0.05, -0.05]),
            ],
            vec![
                point([0.2, 0.2], [0.0; 2], [0.1, 0.0]),
                point([1.3, -0.4], [-0.1, 0.0], [0.1, 0.0]),
                point([0.8, 0.8], [0.0, -0.1], [0.0; 2]),
            ],
        ]
    }

    fn run(mut points: Vec<ControlPoint>) -> (Vec<ControlPoint>, usize) {
        let unsolved = auto_comb(&mut points, None).unsolved_joints;
        (points, unsolved)
    }

    fn cubic(points: &[ControlPoint], segment: usize) -> [V2; 4] {
        let (start, end) = (&points[segment], &points[segment + 1]);
        [
            sv(*start.val()),
            sv(bezier_handle(start, OUTGOING)),
            sv(bezier_handle(end, INCOMING)),
            sv(*end.val()),
        ]
    }

    fn derivative_curvature(cubic: [V2; 4], at_end: bool) -> f64 {
        let [p0, p1, p2, p3] = cubic;
        let (d1, d2) = if at_end {
            ((p3 - p2) * 3.0, (p1 - p2 * 2.0 + p3) * 6.0)
        } else {
            ((p1 - p0) * 3.0, (p0 - p1 * 2.0 + p2) * 6.0)
        };
        d1.cross(d2) / d1.length().powi(3)
    }

    fn joint_gap(points: &[ControlPoint], joint: usize) -> f64 {
        let arriving = derivative_curvature(cubic(points, joint - 1), true);
        let leaving = derivative_curvature(cubic(points, joint), false);
        (arriving - leaving).abs() / arriving.abs().max(leaving.abs()).max(1.0)
    }

    fn handle_direction(cp: &ControlPoint, slot: usize) -> V2 {
        (sv(bezier_handle(cp, slot)) - sv(*cp.val()))
            .unit()
            .unwrap()
    }

    fn assert_parallel(actual: V2, expected: V2) {
        assert!(
            actual.cross(expected).abs() < 1e-5 && actual.dot(expected) > 0.0,
            "{actual:?} vs {expected:?}"
        );
    }

    fn assert_continuous(points: &[ControlPoint]) {
        for joint in 1..points.len() - 1 {
            let cp = &points[joint];
            assert_parallel(
                handle_direction(cp, OUTGOING),
                -handle_direction(cp, INCOMING),
            );
            let gap = joint_gap(points, joint);
            assert!(gap < 2.5e-3, "joint {joint} gap {gap}: {points:?}");
        }
    }

    fn assert_finite(points: &[ControlPoint]) {
        for cp in points {
            let tangents = cp.tangents().iter().flatten();
            for value in std::iter::once(cp.val()).chain(tangents) {
                assert!(value.val.iter().all(|c| c.is_finite()), "{points:?}");
            }
        }
    }

    fn random_configurations(count: usize) -> Vec<Vec<ControlPoint>> {
        let mut seed: u64 = 98765;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        (0..count)
            .map(|_| {
                let len = 3 + (next() * 5.0) as usize;
                (0..len)
                    .map(|_| {
                        let position = [next(), next()];
                        let outgoing = [next() * 0.4 - 0.2, next() * 0.4 - 0.2];
                        let incoming_length = next() * 0.2;
                        if next() < 0.2 {
                            untangented(position)
                        } else {
                            mirrored(position, outgoing, incoming_length)
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn random_configurations_settle_and_stay_finite() {
        for mut points in random_configurations(500) {
            let unsolved = apply_auto_comb(&mut points, None);
            assert_finite(&points);
            let settled = points.clone();
            assert_eq!(apply_auto_comb(&mut points, None), unsolved);
            assert_eq!(points, settled);
        }
    }

    #[test]
    fn endpoint_curvature_matches_the_cubic_derivatives() {
        let cases = [
            ([0.2, 0.3], [0.8, 0.6], [1.0, 0.0], [0.0, 1.0], (0.2, 0.3)),
            ([0.1, 0.9], [0.7, 0.2], [0.6, -0.8], [0.8, 0.6], (0.05, 0.4)),
            ([0.5, 0.5], [0.9, 0.5], [0.0, -1.0], [0.0, 1.0], (0.3, 0.1)),
            (
                [0.7, 0.2],
                [0.3, 0.4],
                [-0.6, 0.8],
                [-0.8, -0.6],
                (0.2, 0.15),
            ),
        ];
        for (start, end, start_tangent, end_tangent, (h0, h1)) in cases {
            let [start, end, start_tangent, end_tangent] =
                [start, end, start_tangent, end_tangent].map(|[x, y]| V2::new(x, y));
            let segment = Segment {
                start,
                end,
                start_tangent,
                end_tangent,
            };
            let cubic = [
                start,
                start + start_tangent * h0,
                end - end_tangent * h1,
                end,
            ];
            let (k0, k1) = segment.curvatures((h0, h1)).unwrap();
            assert!(
                (k0 - derivative_curvature(cubic, false)).abs() < 1e-9,
                "{k0} {cubic:?}"
            );
            assert!(
                (k1 - derivative_curvature(cubic, true)).abs() < 1e-9,
                "{k1} {cubic:?}"
            );
        }
    }

    #[test]
    fn c_shape_becomes_continuous_without_turning_any_handle() {
        let before = c_shape();
        let (after, unsolved) = run(before.clone());
        assert_eq!(unsolved, 0);
        assert_continuous(&after);
        for (old, new) in before.iter().zip(&after) {
            for slot in [INCOMING, OUTGOING] {
                assert_parallel(handle_direction(new, slot), handle_direction(old, slot));
            }
        }
    }

    #[test]
    fn s_shape_becomes_continuous() {
        let (after, unsolved) = run(s_shape());
        assert_eq!(unsolved, 0);
        assert_continuous(&after);
    }

    #[test]
    fn points_without_tangents_get_continuous_handles() {
        let (after, unsolved) = run(ellipse_without_tangents());
        assert_eq!(unsolved, 0);
        assert_continuous(&after);
    }

    #[test]
    fn a_second_pass_changes_nothing() {
        let cases = [
            c_shape(),
            s_shape(),
            ellipse_without_tangents(),
            kinked(),
            near_border_arc(),
        ];
        for points in cases.into_iter().chain(degenerate_cases()) {
            let (once, unsolved) = run(points);
            let (twice, unsolved_again) = run(once.clone());
            assert_eq!(twice, once);
            assert_eq!(unsolved_again, unsolved);
        }
    }

    #[test]
    fn kinked_joint_turns_both_handles_to_the_bisector() {
        let (after, _) = run(kinked());
        let bisector = V2::new(1.0, 1.0).unit().unwrap();
        assert_parallel(handle_direction(&after[1], OUTGOING), bisector);
        assert_parallel(handle_direction(&after[1], INCOMING), -bisector);
        assert_continuous(&after);
    }

    #[test]
    fn pinned_handle_keeps_its_direction_and_the_other_side_follows() {
        let mut points = kinked();
        auto_comb(&mut points, Some((1, INCOMING)));
        assert_parallel(handle_direction(&points[1], INCOMING), -V2::X);
        assert_parallel(handle_direction(&points[1], OUTGOING), V2::X);
    }

    #[test]
    fn tangent_hue_is_kept() {
        let mut points = c_shape();
        for tangent in points
            .iter_mut()
            .flat_map(|cp| cp.tangents_mut().iter_mut().flatten())
        {
            tangent[2] = 0.3;
        }
        auto_comb(&mut points, None);
        for tangent in points.iter().flat_map(|cp| cp.tangents().iter().flatten()) {
            assert_eq!(tangent[2], 0.3);
        }
    }

    #[test]
    fn handles_are_never_clamped_by_the_square() {
        for points in [
            c_shape(),
            s_shape(),
            ellipse_without_tangents(),
            near_border_arc(),
        ] {
            let (after, _) = run(points);
            let last = after.len() - 1;
            let used_handles = (0..last)
                .map(|i| (i, OUTGOING))
                .chain((1..=last).map(|i| (i, INCOMING)));
            for (i, slot) in used_handles {
                let cp = &after[i];
                let unclamped = *cp.val() + cp.tangents()[slot].unwrap_or_default();
                let drawn = bezier_handle(cp, slot);
                assert!((sv(unclamped) - sv(drawn)).length() < 1e-6, "{after:?}");
            }
        }
    }

    #[test]
    fn point_outside_the_square_is_kept_and_counted_while_its_neighbours_match_it() {
        let mut points = c_shape();
        points[2].val_mut()[1] = -0.2;
        let outside = points[2].clone();
        let unsolved = auto_comb(&mut points, None).unsolved_joints;
        assert_eq!(unsolved, 1);
        assert_eq!(points[2], outside);
        for joint in [1, 3] {
            assert!(
                joint_gap(&points, joint) < 2.5e-3,
                "joint {joint}: {points:?}"
            );
        }
    }

    #[test]
    fn degenerate_inputs_stay_finite() {
        for points in degenerate_cases() {
            let (after, _) = run(points);
            assert_finite(&after);
        }
    }

    #[test]
    fn parallel_tangents_solve_each_end_alone() {
        let segment = Segment {
            start: V2::new(0.2, 0.3),
            end: V2::new(0.4, 0.5),
            start_tangent: V2::X,
            end_tangent: V2::X,
        };
        let max_lengths = (0.8, 0.4);
        let targets = (Some(1.0), Some(-1.0));
        let lengths = solve_lengths(&segment, targets, max_lengths, (0.1, 0.1)).unwrap();
        let (k0, k1) = segment.curvatures(lengths).unwrap();
        assert!(
            (k0 - 1.0).abs() < 1e-6 && (k1 + 1.0).abs() < 1e-6,
            "{lengths:?}"
        );
        let s_bend = (Some(-1.0), Some(-1.0));
        assert_eq!(
            solve_lengths(&segment, s_bend, max_lengths, (0.1, 0.1)),
            None
        );
    }

    #[test]
    fn free_end_keeps_its_length_when_the_other_end_can_meet_the_target() {
        let segment = Segment {
            start: V2::new(0.2, 0.3),
            end: V2::new(0.4, 0.5),
            start_tangent: V2::X,
            end_tangent: V2::X,
        };
        let lengths = solve_lengths(&segment, (None, Some(-1.0)), (0.8, 0.4), (0.1, 0.1)).unwrap();
        assert_eq!(lengths.0, 0.1);
        assert!(
            (segment.curvatures(lengths).unwrap().1 + 1.0).abs() < 1e-6,
            "{lengths:?}"
        );
    }

    #[test]
    fn zero_target_curvature_is_solved() {
        let segment = Segment {
            start: V2::new(0.2, 0.2),
            end: V2::new(0.8, 0.6),
            start_tangent: V2::X,
            end_tangent: V2::new(0.0, 1.0),
        };
        let targets = (Some(0.0), Some(2.0));
        let lengths = solve_lengths(&segment, targets, (0.8, 0.6), (0.2, 0.2)).unwrap();
        let (k0, k1) = segment.curvatures(lengths).unwrap();
        assert!(k0.abs() < 1e-6 && (k1 - 2.0).abs() < 1e-6, "{lengths:?}");
    }
}
