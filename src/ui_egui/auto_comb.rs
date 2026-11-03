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
const FEASIBILITY_SAMPLES: usize = 64;
const STORAGE_ROUNDING: f64 = 1.0e-6;
const BISECTION_STEPS: usize = 200;
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

fn swapped<T>((a, b): (T, T)) -> (T, T) {
    (b, a)
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

    fn residuals(&self, targets: (f64, f64), lengths: (f64, f64)) -> (f64, f64) {
        let (beta, gamma0, gamma1) = self.coefficients();
        let (h0, h1) = lengths;
        (
            targets.0 * h0 * h0 + beta * h1 - gamma0,
            targets.1 * h1 * h1 + beta * h0 - gamma1,
        )
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

    // Traversed backwards, so lengths swap and signed curvatures swap and negate.
    fn reversed(&self) -> Segment {
        Segment {
            start: self.end,
            end: self.start,
            start_tangent: -self.end_tangent,
            end_tangent: -self.start_tangent,
        }
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

fn meets(k: f64, slack: f64, target: f64) -> bool {
    (k - target).abs() <= CURVATURE_TOLERANCE * target.abs().max(1.0) + slack
}

fn fits(
    segment: &Segment,
    lengths: (f64, f64),
    targets: (f64, f64),
    max_lengths: (f64, f64),
) -> bool {
    lengths.0 <= max_lengths.0 + STORAGE_ROUNDING
        && lengths.1 <= max_lengths.1 + STORAGE_ROUNDING
        && segment.curvatures(lengths).is_some_and(|k| {
            let slack = segment.rounding_slack(lengths, k);
            meets(k.0, slack.0, targets.0) && meets(k.1, slack.1, targets.1)
        })
}

fn polynomial_at(coefficients: &[f64], x: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |value, &coefficient| value * x + coefficient)
}

fn slope(coefficients: &[f64]) -> Vec<f64> {
    coefficients
        .iter()
        .zip(0..)
        .skip(1)
        .map(|(&coefficient, power)| coefficient * f64::from(power))
        .collect()
}

fn bisected_root(coefficients: &[f64], mut low: f64, low_value: f64, mut high: f64) -> f64 {
    for _ in 0..BISECTION_STEPS {
        let middle = 0.5 * (low + high);
        let value = polynomial_at(coefficients, middle);
        if value == 0.0 || middle <= low || middle >= high {
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

// Coefficients lowest power first. Between neighbouring roots of the derivative the polynomial is
// monotone, so each such stretch holds at most one root.
fn roots_in(coefficients: &[f64], low: f64, high: f64) -> Vec<f64> {
    let degree = coefficients.iter().rposition(|&c| c != 0.0).unwrap_or(0);
    if degree == 0 || high <= low {
        return Vec::new();
    }
    if degree == 1 {
        let root = -coefficients[0] / coefficients[1];
        return (low..=high)
            .contains(&root)
            .then_some(root)
            .into_iter()
            .collect();
    }
    let mut bounds = vec![low];
    bounds.extend(roots_in(&slope(coefficients), low, high));
    bounds.push(high);
    let at = |x: f64| polynomial_at(coefficients, x);
    let mut roots: Vec<f64> = (at(low) == 0.0).then_some(low).into_iter().collect();
    for pair in bounds.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (value_a, value_b) = (at(a), at(b));
        if value_b == 0.0 {
            roots.push(b);
        } else if value_a != 0.0 && (value_a < 0.0) != (value_b < 0.0) {
            roots.push(bisected_root(coefficients, a, value_a, b));
        }
    }
    roots
}

// Equation a gives h_b = (gamma_a - k_a h_a^2) / beta; put into equation b and scaled by beta^2.
fn eliminated_quartic(k_a: f64, gamma_a: f64, k_b: f64, gamma_b: f64, beta: f64) -> [f64; 5] {
    [
        k_b * gamma_a * gamma_a - beta * beta * gamma_b,
        beta * beta * beta,
        -2.0 * k_b * k_a * gamma_a,
        0.0,
        k_b * k_a * k_a,
    ]
}

fn newton_polished(segment: &Segment, targets: (f64, f64), lengths: (f64, f64)) -> (f64, f64) {
    let (beta, _, _) = segment.coefficients();
    let size = |(f0, f1): (f64, f64)| f0.hypot(f1);
    let mut lengths = lengths;
    let mut residual = size(segment.residuals(targets, lengths));
    for _ in 0..NEWTON_STEPS {
        let (f0, f1) = segment.residuals(targets, lengths);
        let (j00, j11) = (2.0 * targets.0 * lengths.0, 2.0 * targets.1 * lengths.1);
        let determinant = j00 * j11 - beta * beta;
        let next = (
            lengths.0 - (j11 * f0 - beta * f1) / determinant,
            lengths.1 - (j00 * f1 - beta * f0) / determinant,
        );
        let next_residual = size(segment.residuals(targets, next));
        if next_residual.is_nan() || next_residual >= residual {
            break;
        }
        (lengths, residual) = (next, next_residual);
    }
    lengths
}

// Both parabolas' intersections, found from either end's quartic, plus a guess for nearly parallel tangents.
fn candidate_lengths(
    segment: &Segment,
    targets: (f64, f64),
    max_lengths: (f64, f64),
    current: (f64, f64),
) -> Vec<(f64, f64)> {
    let (beta, gamma0, gamma1) = segment.coefficients();
    let (k0, k1) = targets;
    let mut candidates = Vec::new();
    if beta != 0.0 {
        let start_quartic = eliminated_quartic(k0, gamma0, k1, gamma1, beta);
        let end_quartic = eliminated_quartic(k1, gamma1, k0, gamma0, beta);
        let on_start = roots_in(&start_quartic, MIN_SOLVED_LENGTH, max_lengths.0)
            .into_iter()
            .map(|h0| (h0, (gamma0 - k0 * h0 * h0) / beta));
        let on_end = roots_in(&end_quartic, MIN_SOLVED_LENGTH, max_lengths.1)
            .into_iter()
            .map(|h1| ((gamma1 - k1 * h1 * h1) / beta, h1));
        candidates.extend(on_start.chain(on_end));
    }
    // Near-parallel tangents leave each equation almost alone; a length no equation sets stays.
    let alone = |k: f64, gamma: f64, current: f64| {
        let squared = gamma / k;
        if squared > 0.0 && squared.is_finite() {
            squared.sqrt()
        } else {
            current
        }
    };
    candidates.push((alone(k0, gamma0, current.0), alone(k1, gamma1, current.1)));
    candidates
        .into_iter()
        .map(|lengths| newton_polished(segment, targets, lengths))
        .collect()
}

// After https://acko.net/blog/curvature-beziers/: fades to zero towards a zero length, where the two
// equations meet with opposing gradients (a root about to vanish) and for handles long against the
// chord (loops). Damped by the distance from the current lengths, so a drag follows its solution.
fn solution_score(
    segment: &Segment,
    targets: (f64, f64),
    lengths: (f64, f64),
    current: (f64, f64),
) -> f64 {
    let (beta, _, _) = segment.coefficients();
    let start_gradient = V2::new(2.0 * targets.0 * lengths.0, beta);
    let end_gradient = V2::new(beta, 2.0 * targets.1 * lengths.1);
    let opening = start_gradient
        .cross(end_gradient)
        .abs()
        .atan2(-start_gradient.dot(end_gradient));
    let chord = (segment.end - segment.start)
        .length()
        .max(MIN_SOLVED_LENGTH);
    let (u0, u1) = (lengths.0 / chord, lengths.1 / chord);
    let shape = opening * u0 * u1 / (1.0 + u0 * u0 + u1 * u1).powi(2);
    let drift = (lengths.0 - current.0).hypot(lengths.1 - current.1) / chord;
    shape / (1.0 + drift * drift)
}

fn solve_lengths(
    segment: &Segment,
    targets: (f64, f64),
    max_lengths: (f64, f64),
    current: (f64, f64),
) -> Option<(f64, f64)> {
    if fits(segment, current, targets, max_lengths) {
        return Some(current);
    }
    let score = |lengths: &(f64, f64)| solution_score(segment, targets, *lengths, current);
    candidate_lengths(segment, targets, max_lengths, current)
        .into_iter()
        .filter(|&lengths| {
            lengths.0.min(lengths.1) >= MIN_SOLVED_LENGTH
                && fits(segment, lengths, targets, max_lengths)
        })
        .max_by(|a, b| score(a).total_cmp(&score(b)))
}

// The start curvature once the end length is re-solved, with the start length held, to keep the end curvature.
fn held_start_solution(
    segment: &Segment,
    start_length: f64,
    end_target: f64,
    end_max: f64,
    end_current: f64,
) -> Option<f64> {
    let meets_end = |end_length: f64| {
        let lengths = (start_length, end_length);
        (MIN_SOLVED_LENGTH..=end_max + STORAGE_ROUNDING).contains(&end_length)
            && segment
                .curvatures(lengths)
                .is_some_and(|k| meets(k.1, segment.rounding_slack(lengths, k).1, end_target))
    };
    let (beta, _, gamma1) = segment.coefficients();
    let squared = (gamma1 - beta * start_length) / end_target;
    let end_length = [end_current, squared.sqrt()]
        .into_iter()
        .find(|&length| meets_end(length))?;
    segment
        .curvatures((start_length, end_length))
        .map(|(k0, _)| k0)
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

// Travel direction along the circle through the point and its two nearest neighbours: inverting about
// the point maps that circle onto a line parallel to its tangent there.
fn circle_direction(positions: &[V2], index: usize) -> V2 {
    let here = positions[index];
    let last = positions.len() - 1;
    if last < 2 {
        return chord_direction(positions, index);
    }
    let inverted = |i: usize| {
        let offset = positions[i] - here;
        let squared = offset.dot(offset);
        (squared > f64::EPSILON).then(|| offset * (1.0 / squared))
    };
    let (towards, away) = match index {
        0 => (1, 2),
        i if i == last => (i - 2, i - 1),
        i => (i + 1, i - 1),
    };
    inverted(towards)
        .zip(inverted(away))
        .and_then(|(towards, away)| (towards - away).unit())
        .unwrap_or_else(|| chord_direction(positions, index))
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
    let fallback = circle_direction(positions, index);
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

struct Layout {
    positions: Vec<V2>,
    handles: Vec<[V2; 2]>,
    directions: Vec<[V2; 2]>,
    segments: Vec<Segment>,
    max_lengths: Vec<(f64, f64)>,
    current: Vec<(f64, f64)>,
}

impl Layout {
    fn new(points: &[ControlPoint], pinned: Option<(usize, usize)>) -> Self {
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
        let segment_count = points.len().saturating_sub(1);
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
        Self {
            positions,
            handles,
            directions,
            segments,
            max_lengths,
            current,
        }
    }

    fn handle_lengths(&self, segment: usize) -> (f64, f64) {
        (
            self.handles[segment][OUTGOING].length(),
            self.handles[segment + 1][INCOMING].length(),
        )
    }

    // Curvature and rounding slack of each side of a joint whose handles are not degenerate.
    fn joint_sides(&self, joint: usize) -> Vec<(f64, f64)> {
        let side = |segment: usize, is_end: bool| {
            let lengths = self.handle_lengths(segment);
            let segment = &self.segments[segment];
            segment.curvatures(lengths).map(|k| {
                let slack = segment.rounding_slack(lengths, k);
                if is_end {
                    (k.1, slack.1)
                } else {
                    (k.0, slack.0)
                }
            })
        };
        let arriving = (joint > 0).then(|| side(joint - 1, true)).flatten();
        let leaving = (joint < self.segments.len())
            .then(|| side(joint, false))
            .flatten();
        arriving.into_iter().chain(leaving).collect()
    }
}

// The segment a handle shapes, and whether the handle sits at that segment's start.
fn handle_segment(point_count: usize, index: usize, slot: usize) -> Option<(usize, bool)> {
    match slot {
        OUTGOING if index + 1 < point_count => Some((index, true)),
        INCOMING if index > 0 && index < point_count => Some((index - 1, false)),
        _ => None,
    }
}

fn stored_curvature(cp: &ControlPoint) -> Option<f64> {
    cp.curvature().map(f64::from).filter(|k| k.is_finite())
}

// A joint whose two sides disagree has no one curvature to keep, so it gets the circle's like a joint
// with degenerate handles.
fn measured_curvature(layout: &Layout, joint: usize) -> f64 {
    let measured = match layout.joint_sides(joint)[..] {
        [(k, _)] => Some(k),
        [(a, a_slack), (b, b_slack)] => meets(a, a_slack + b_slack, b).then_some(0.5 * (a + b)),
        _ => None,
    };
    let count = layout.positions.len();
    let first = joint.saturating_sub(1).min(count.saturating_sub(3));
    let circle = (count >= 3)
        .then(|| {
            let [a, b, c] = [first, first + 1, first + 2].map(|i| layout.positions[i]);
            circle_curvature(a, b, c)
        })
        .flatten();
    measured.or(circle).unwrap_or(0.0)
}

fn joint_segments(layout: &Layout, joint: usize) -> std::ops::Range<usize> {
    joint.saturating_sub(1)..(joint + 1).min(layout.segments.len())
}

// What the joint's curvature can be on one segment while the segment's far end keeps its curvature,
// as the spans swept between neighbouring samples of the far end's equation.
fn feasible_spans(
    layout: &Layout,
    targets: &[f64],
    joint: usize,
    segment: usize,
) -> Vec<(f64, f64)> {
    let (oriented, max_lengths, far_target, sign) = if segment == joint {
        let max_lengths = layout.max_lengths[segment];
        (
            layout.segments[segment],
            max_lengths,
            targets[segment + 1],
            1.0,
        )
    } else {
        let max_lengths = swapped(layout.max_lengths[segment]);
        (
            layout.segments[segment].reversed(),
            max_lengths,
            -targets[segment],
            -1.0,
        )
    };
    let (beta, _, gamma1) = oriented.coefficients();
    let joint_curvature = |lengths: (f64, f64)| {
        let is_valid = (MIN_SOLVED_LENGTH..=max_lengths.0).contains(&lengths.0)
            && (MIN_SOLVED_LENGTH..=max_lengths.1 + STORAGE_ROUNDING).contains(&lengths.1);
        is_valid
            .then(|| oriented.curvatures(lengths))
            .flatten()
            .map(|(k, _)| sign * k)
    };
    let sampled = |max: f64| {
        (0..=FEASIBILITY_SAMPLES).map(move |i| {
            let fraction = i as f64 / FEASIBILITY_SAMPLES as f64;
            MIN_SOLVED_LENGTH + (max - MIN_SOLVED_LENGTH) * fraction
        })
    };
    // The far equation is a parabola in the two lengths; walking it along either length covers the
    // stretches where the other one changes fast.
    let by_joint: Vec<Option<f64>> = sampled(max_lengths.0)
        .map(|joint| {
            let far = ((gamma1 - beta * joint) / far_target).sqrt();
            joint_curvature((joint, far))
        })
        .collect();
    let by_far: Vec<Option<f64>> = sampled(max_lengths.1)
        .map(|far| joint_curvature(((gamma1 - far_target * far * far) / beta, far)))
        .collect();
    [by_joint, by_far]
        .iter()
        .flat_map(|samples| samples.windows(2))
        .filter_map(|pair| match *pair {
            [Some(a), Some(b)] => Some((a.min(b), a.max(b))),
            _ => None,
        })
        .collect()
}

// The curvature nearest `preferred` that all the joint's segments can meet with their far ends held.
fn nearest_feasible_curvature(
    layout: &Layout,
    targets: &[f64],
    joint: usize,
    preferred: f64,
) -> Option<f64> {
    let mut feasible = joint_segments(layout, joint)
        .map(|segment| feasible_spans(layout, targets, joint, segment))
        .reduce(|a, b| {
            a.iter()
                .flat_map(|&(a_low, a_high)| {
                    b.iter().filter_map(move |&(b_low, b_high)| {
                        let span = (a_low.max(b_low), a_high.min(b_high));
                        (span.0 <= span.1).then_some(span)
                    })
                })
                .collect()
        })?;
    feasible.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (low, high) in feasible {
        match merged.last_mut() {
            Some(last) if low <= last.1 => last.1 = last.1.max(high),
            _ => merged.push((low, high)),
        }
    }
    // A quarter of the way into the nearest interval, so the next small move doesn't fall out again.
    merged
        .into_iter()
        .map(|(low, high)| {
            if (low..=high).contains(&preferred) {
                preferred
            } else if preferred < low {
                low + 0.25 * (high - low)
            } else {
                high - 0.25 * (high - low)
            }
        })
        .min_by(|a, b| (a - preferred).abs().total_cmp(&(b - preferred).abs()))
}

fn store_curvature(cp: &mut ControlPoint, k: f64) -> f64 {
    assert!(k.is_finite(), "{k}");
    *cp.curvature_mut() = Some(k as f32);
    f64::from(k as f32)
}

// Seeds start from the handles and move to the nearest value the joint's segments can meet.
fn seed_curvatures(points: &mut [ControlPoint], edit: CombEdit) {
    let layout = Layout::new(points, edit.dragged_handle);
    let is_missing: Vec<bool> = points
        .iter()
        .enumerate()
        .map(|(joint, cp)| {
            !stored_curvature(cp).is_some_and(|k| {
                !edit.is_reseed
                    || layout
                        .joint_sides(joint)
                        .iter()
                        .all(|&(side, slack)| meets(side, slack, k))
            })
        })
        .collect();
    let mut targets: Vec<f64> = points
        .iter()
        .enumerate()
        .map(|(joint, cp)| match stored_curvature(cp) {
            Some(k) if !is_missing[joint] => k,
            _ => measured_curvature(&layout, joint),
        })
        .collect();
    for joint in (0..points.len()).filter(|&joint| is_missing[joint]) {
        let preferred = targets[joint];
        let seeded = nearest_feasible_curvature(&layout, &targets, joint, preferred);
        targets[joint] = store_curvature(&mut points[joint], seeded.unwrap_or(preferred));
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CombEdit {
    pub dragged_point: Option<usize>,
    pub dragged_handle: Option<(usize, usize)>,
    pub is_reseed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CombPass {
    unsolved_segments: usize,
    is_changed: bool,
}

fn comb_pass(points: &mut [ControlPoint], edit: CombEdit) -> CombPass {
    let layout = Layout::new(points, edit.dragged_handle);
    let mut targets: Vec<f64> = points
        .iter()
        .map(|cp| stored_curvature(cp).expect("curvatures are seeded before a pass"))
        .collect();
    let solve = |targets: &[f64], i: usize| {
        solve_lengths(
            &layout.segments[i],
            (targets[i], targets[i + 1]),
            layout.max_lengths[i],
            layout.current[i],
        )
    };
    let mut is_changed = false;
    let edited_joint = edit
        .dragged_point
        .or(edit.dragged_handle.map(|(index, _)| index))
        .filter(|&joint| joint < points.len());
    if let Some(joint) = edited_joint {
        // The joint being dragged gives way when its segments can't keep its curvature.
        let is_stuck = joint_segments(&layout, joint).any(|i| solve(&targets, i).is_none());
        let feasible = is_stuck
            .then(|| nearest_feasible_curvature(&layout, &targets, joint, targets[joint]))
            .flatten();
        if let Some(k) = feasible {
            let before = points[joint].curvature();
            targets[joint] = store_curvature(&mut points[joint], k);
            is_changed |= points[joint].curvature() != before;
        }
    }
    let lengths: Vec<(f64, f64)> = (0..layout.segments.len())
        .map(|i| solve(&targets, i).unwrap_or(layout.current[i]))
        .collect();

    for (i, &(start_length, end_length)) in lengths.iter().enumerate() {
        is_changed |= set_handle(
            &mut points[i],
            OUTGOING,
            layout.handles[i][OUTGOING],
            layout.directions[i][OUTGOING],
            start_length,
        );
        is_changed |= set_handle(
            &mut points[i + 1],
            INCOMING,
            layout.handles[i + 1][INCOMING],
            layout.directions[i + 1][INCOMING],
            end_length,
        );
    }
    let unsolved_segments = (0..layout.segments.len())
        .filter(|&i| {
            !fits(
                &layout.segments[i],
                lengths[i],
                (targets[i], targets[i + 1]),
                layout.max_lengths[i],
            )
        })
        .count();
    CombPass {
        unsolved_segments,
        is_changed,
    }
}

// A handle that lands on the square's border can need a second pass to settle.
pub fn apply_auto_comb(points: &mut [ControlPoint], edit: CombEdit) -> usize {
    if points.len() < 2 {
        return 0;
    }
    seed_curvatures(points, edit);
    let mut pass = CombPass::default();
    for _ in 0..MAX_PASSES {
        pass = comb_pass(points, edit);
        if !pass.is_changed {
            break;
        }
    }
    pass.unsolved_segments
}

// The drag's component along the handle sets the joint's curvature (with the segment's far curvature
// held); the next pass turns the joint to the handle's new direction, holding curvatures.
fn set_curvature_from_drag(points: &mut [ControlPoint], index: usize, slot: usize, target: V2) {
    let Some((segment, is_at_start)) = handle_segment(points.len(), index, slot) else {
        return;
    };
    let far_joint = if is_at_start { segment + 1 } else { segment };
    let Some(far_target) = stored_curvature(&points[far_joint]) else {
        return;
    };
    let layout = Layout::new(points, Some((index, slot)));
    let handle = layout.handles[index][slot];
    if handle.length() < MIN_HANDLE_LENGTH {
        return;
    }
    let direction = layout.directions[index][slot];
    let stretch = (target - layout.positions[index] - handle).dot(direction);
    if stretch.abs() <= STORAGE_ROUNDING {
        return;
    }
    let (max_lengths, current) = (layout.max_lengths[segment], layout.current[segment]);
    let (oriented, max_lengths, current, sign) = if is_at_start {
        (layout.segments[segment], max_lengths, current, 1.0)
    } else {
        (
            layout.segments[segment].reversed(),
            swapped(max_lengths),
            swapped(current),
            -1.0,
        )
    };
    if max_lengths.0 < MIN_SOLVED_LENGTH {
        return;
    }
    let length = (handle.length() + stretch).clamp(MIN_SOLVED_LENGTH, max_lengths.0);
    let joint_curvature = held_start_solution(
        &oriented,
        length,
        sign * far_target,
        max_lengths.1,
        current.1,
    );
    if let Some(k) = joint_curvature {
        *points[index].curvature_mut() = Some((sign * k) as f32);
    }
}

pub fn drag_handle_to(
    points: &mut [ControlPoint],
    index: usize,
    slot: usize,
    target: ControlPointValue,
    is_auto_comb_running: bool,
) {
    if points[index].tangents()[slot].is_none() {
        return;
    }
    if is_auto_comb_running {
        set_curvature_from_drag(points, index, slot, sv(target));
    }
    let offset = target - *points[index].val();
    points[index].tangents_mut()[slot] = Some(offset);
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
            vec![
                point([0.2, 0.3], [0.0; 2], [0.1, 0.1]),
                untangented([0.7, 0.6]),
            ],
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

    const RESEED: CombEdit = CombEdit {
        dragged_point: None,
        dragged_handle: None,
        is_reseed: true,
    };

    fn handle_drag(index: usize, slot: usize) -> CombEdit {
        CombEdit {
            dragged_handle: Some((index, slot)),
            ..Default::default()
        }
    }

    fn run(mut points: Vec<ControlPoint>) -> (Vec<ControlPoint>, usize) {
        let unsolved = apply_auto_comb(&mut points, CombEdit::default());
        (points, unsolved)
    }

    fn combed(points: Vec<ControlPoint>) -> Vec<ControlPoint> {
        let (points, unsolved) = run(points);
        assert_eq!(unsolved, 0, "{points:?}");
        points
    }

    fn stored(points: &[ControlPoint]) -> Vec<f32> {
        points.iter().map(|cp| cp.curvature().unwrap()).collect()
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

    fn relative_gap(a: f64, b: f64) -> f64 {
        (a - b).abs() / a.abs().max(b.abs()).max(1.0)
    }

    fn joint_gap(points: &[ControlPoint], joint: usize) -> f64 {
        let arriving = derivative_curvature(cubic(points, joint - 1), true);
        let leaving = derivative_curvature(cubic(points, joint), false);
        relative_gap(arriving, leaving)
    }

    fn handle_direction(cp: &ControlPoint, slot: usize) -> V2 {
        (sv(bezier_handle(cp, slot)) - sv(*cp.val()))
            .unit()
            .unwrap()
    }

    fn handle_length(cp: &ControlPoint, slot: usize) -> f64 {
        (sv(bezier_handle(cp, slot)) - sv(*cp.val())).length()
    }

    fn rotated(v: V2, angle: f64) -> V2 {
        let (sin, cos) = angle.sin_cos();
        V2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
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

    fn assert_meets_stored(points: &[ControlPoint], segment: usize) {
        let cubic = cubic(points, segment);
        for (k, joint) in [
            (derivative_curvature(cubic, false), segment),
            (derivative_curvature(cubic, true), segment + 1),
        ] {
            let target = points[joint].curvature().unwrap() as f64;
            let gap = relative_gap(k, target);
            assert!(gap < 2.5e-3, "joint {joint}: {k} vs {target}: {points:?}");
        }
    }

    fn assert_finite(points: &[ControlPoint]) {
        for cp in points {
            let tangents = cp.tangents().iter().flatten();
            for value in std::iter::once(cp.val()).chain(tangents) {
                assert!(value.val.iter().all(|c| c.is_finite()), "{points:?}");
            }
            assert!(cp.curvature().is_none_or(f32::is_finite), "{points:?}");
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

    fn drag(points: &mut [ControlPoint], index: usize, slot: usize, offset: V2) {
        let mut target = *points[index].val();
        target[0] += offset.x as f32;
        target[1] += offset.y as f32;
        drag_handle_to(points, index, slot, target, true);
        apply_auto_comb(points, handle_drag(index, slot));
    }

    fn handle_offset(cp: &ControlPoint, slot: usize) -> V2 {
        sv(bezier_handle(cp, slot)) - sv(*cp.val())
    }

    #[test]
    fn random_configurations_settle_and_stay_finite() {
        for mut points in random_configurations(500) {
            let unsolved = apply_auto_comb(&mut points, CombEdit::default());
            assert_finite(&points);
            let settled = points.clone();
            assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), unsolved);
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
            let (r0, r1) = segment.reversed().curvatures((h1, h0)).unwrap();
            assert!((r0 + k1).abs() < 1e-9 && (r1 + k0).abs() < 1e-9);
        }
    }

    #[test]
    fn roots_in_finds_every_root_of_a_quartic_in_the_range() {
        let quartic = [0.021, -0.238, 0.917, -1.4, 0.7];
        let product = |x: f64| 0.7 * (x - 0.2) * (x - 0.3) * (x - 0.5) * (x - 1.0);
        for x in [0.0, 0.4, 0.8, 1.3] {
            assert!((polynomial_at(&quartic, x) - product(x)).abs() < 1e-12);
        }
        let roots = roots_in(&quartic, 0.25, 1.2);
        assert_eq!(roots.len(), 3, "{roots:?}");
        for (root, expected) in roots.iter().zip([0.3, 0.5, 1.0]) {
            assert!((root - expected).abs() < 1e-12, "{roots:?}");
        }
        assert_eq!(roots_in(&[0.0, 0.0, 1.0], 0.1, 1.0), Vec::<f64>::new());
        assert_eq!(roots_in(&[-0.5, 1.0], 0.1, 1.0), vec![0.5]);
    }

    #[test]
    fn solver_finds_the_lengths_a_curvature_pair_came_from() {
        let mut seed: u64 = 1234;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..2000 {
            let start = V2::new(0.2 + 0.6 * next(), 0.2 + 0.6 * next());
            let end = V2::new(0.2 + 0.6 * next(), 0.2 + 0.6 * next());
            let start_angle = next() * std::f64::consts::TAU;
            let end_angle = next() * std::f64::consts::TAU;
            let segment = Segment {
                start,
                end,
                start_tangent: V2::new(start_angle.cos(), start_angle.sin()),
                end_tangent: V2::new(end_angle.cos(), end_angle.sin()),
            };
            let lengths = (0.02 + 0.2 * next(), 0.02 + 0.2 * next());
            let Some(targets) = segment.curvatures(lengths) else {
                continue;
            };
            let found = candidate_lengths(&segment, targets, (1.0, 1.0), (0.1, 0.1))
                .into_iter()
                .any(|l| (l.0 - lengths.0).abs() < 1e-7 && (l.1 - lengths.1).abs() < 1e-7);
            assert!(found, "{segment:?} {lengths:?} {targets:?}");
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
        let lengths = solve_lengths(&segment, (1.0, -1.0), max_lengths, (0.1, 0.1)).unwrap();
        let (k0, k1) = segment.curvatures(lengths).unwrap();
        assert!(
            (k0 - 1.0).abs() < 1e-6 && (k1 + 1.0).abs() < 1e-6,
            "{lengths:?}"
        );
        assert_eq!(
            solve_lengths(&segment, (-1.0, -1.0), max_lengths, (0.1, 0.1)),
            None
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
        let lengths = solve_lengths(&segment, (0.0, 2.0), (0.8, 0.6), (0.2, 0.2)).unwrap();
        let (k0, k1) = segment.curvatures(lengths).unwrap();
        assert!(k0.abs() < 1e-6 && (k1 - 2.0).abs() < 1e-6, "{lengths:?}");
    }

    #[test]
    fn c_shape_becomes_continuous_without_turning_any_handle() {
        let before = c_shape();
        let after = combed(before.clone());
        assert_continuous(&after);
        for (old, new) in before.iter().zip(&after) {
            for slot in [INCOMING, OUTGOING] {
                assert_parallel(handle_direction(new, slot), handle_direction(old, slot));
            }
        }
    }

    #[test]
    fn s_shape_becomes_continuous() {
        assert_continuous(&combed(s_shape()));
    }

    #[test]
    fn points_without_tangents_get_continuous_handles() {
        assert_continuous(&combed(ellipse_without_tangents()));
    }

    #[test]
    fn every_segment_meets_the_stored_curvatures() {
        for points in [c_shape(), s_shape(), ellipse_without_tangents()] {
            let after = combed(points);
            for segment in 0..after.len() - 1 {
                assert_meets_stored(&after, segment);
            }
        }
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
    fn a_drag_settles_once_released() {
        let mut points = combed(c_shape());
        drag(&mut points, 2, OUTGOING, V2::new(0.05, 0.03));
        let dragged = points.clone();
        apply_auto_comb(&mut points, CombEdit::default());
        assert_eq!(points, dragged);
    }

    #[test]
    fn json_without_curvature_loads_and_gets_seeded() {
        let json = serde_json::to_string(&c_shape()).unwrap();
        assert!(!json.contains("curvature"));
        let mut points: Vec<ControlPoint> = serde_json::from_str(&json).unwrap();
        assert!(points.iter().all(|cp| cp.curvature().is_none()));
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 0);
        assert!(points.iter().all(|cp| cp.curvature().is_some()));
        assert_continuous(&points);
    }

    #[test]
    fn seed_comes_from_the_handles_or_the_circle_through_the_neighbours() {
        let radius = 0.3;
        let points = || {
            let lengths = [
                (200.0, [0.1, 0.1]),
                (240.0, [0.1, 0.1]),
                (280.0, [0.1, 0.1]),
            ];
            arc([0.5, 0.5], radius, &lengths)
        };
        let mut without_handles = points();
        *without_handles[1].tangents_mut() = [Some(ControlPointValue::new(0.0, 0.0, 0.0)); 2];
        let seed = measured_curvature(&Layout::new(&without_handles, None), 1);
        assert!(relative_gap(seed, 1.0 / radius as f64) < 1e-4, "{seed}");

        let with_handles = points();
        let seed = measured_curvature(&Layout::new(&with_handles, None), 0);
        let first = derivative_curvature(cubic(&with_handles, 0), false);
        assert!(relative_gap(seed, first) < 1e-6, "{seed} {first}");
    }

    #[test]
    fn moving_a_point_keeps_the_stored_curvatures() {
        let mut points = combed(c_shape());
        let before = stored(&points);
        points[2].val_mut()[0] += 0.005;
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 0);
        assert_eq!(stored(&points), before);
        for segment in 0..points.len() - 1 {
            assert_meets_stored(&points, segment);
        }
        assert_continuous(&points);
    }

    #[test]
    fn lengthening_a_handle_sets_its_curvature_and_the_other_side_matches() {
        let mut points = combed(c_shape());
        let before = stored(&points);
        let outgoing = handle_offset(&points[2], OUTGOING);
        drag(&mut points, 2, OUTGOING, outgoing * 1.15);
        let after = stored(&points);
        assert!(
            relative_gap(after[2] as f64, before[2] as f64) > 0.01,
            "{after:?}"
        );
        for joint in [0, 1, 3, 4] {
            assert_eq!(after[joint], before[joint]);
        }
        let length = handle_length(&points[2], OUTGOING);
        assert!((length - outgoing.length() * 1.15).abs() < 1e-4, "{length}");
        assert_meets_stored(&points, 1);
        assert_meets_stored(&points, 2);
        assert_continuous(&points);
    }

    #[test]
    fn turning_a_handle_turns_its_partner_by_the_same_angle() {
        let mut points = combed(s_shape());
        let before = stored(&points);
        let incoming = handle_direction(&points[2], INCOMING);
        let outgoing = handle_offset(&points[2], OUTGOING);
        let sideways = V2::new(-outgoing.y, outgoing.x) * 0.05;
        drag(&mut points, 2, OUTGOING, outgoing + sideways);
        let turn = sideways.length().atan2(outgoing.length());
        assert_parallel(
            handle_direction(&points[2], INCOMING),
            rotated(incoming, turn),
        );
        assert_eq!(stored(&points), before);
        assert_continuous(&points);
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
        apply_auto_comb(&mut points, handle_drag(1, INCOMING));
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
        apply_auto_comb(&mut points, CombEdit::default());
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
    fn unsolvable_segment_is_counted_and_left_unchanged() {
        let mut points = combed(c_shape());
        let before = points.clone();
        *points[2].curvature_mut() = Some(1.0e4);
        let unsolved = apply_auto_comb(&mut points, CombEdit::default());
        assert_eq!(unsolved, 2);
        assert_eq!(points[2].tangents(), before[2].tangents());
        assert_eq!(
            points[1].tangents()[OUTGOING],
            before[1].tangents()[OUTGOING]
        );
        assert_eq!(
            points[3].tangents()[INCOMING],
            before[3].tangents()[INCOMING]
        );
        assert_eq!(points[2].curvature(), Some(1.0e4));
        for joint in [0, 1, 3, 4] {
            assert_eq!(points[joint], before[joint]);
        }
    }

    #[test]
    fn point_outside_the_square_is_kept_and_counted() {
        let mut points = combed(c_shape());
        points[2].val_mut()[1] = -0.2;
        let outside = points[2].clone();
        let unsolved = apply_auto_comb(&mut points, CombEdit::default());
        assert_eq!(unsolved, 2);
        assert_eq!(points[2], outside);
    }

    #[test]
    fn reseed_replaces_only_curvatures_the_handles_no_longer_meet() {
        let points = combed(c_shape());
        let before = stored(&points);
        let mut stale = points.clone();
        *stale[2].curvature_mut() = Some(before[2] * 3.0);
        apply_auto_comb(&mut stale, RESEED);
        for (reseeded, original) in stale.iter().zip(&points) {
            assert_eq!(reseeded.tangents(), original.tangents());
        }
        let reseeded = stale[2].curvature().unwrap() as f64;
        assert!(
            relative_gap(reseeded, before[2] as f64) < 1e-3,
            "{reseeded}"
        );

        let mut edited = points.clone();
        edited[3].tangents_mut()[OUTGOING] = edited[3].tangents()[OUTGOING].map(|t| t * 2.0);
        *edited[3].curvature_mut() = Some(99.0);
        apply_auto_comb(&mut edited, RESEED);
        assert_eq!(stored(&edited)[..3], before[..3]);
        assert_ne!(stored(&edited)[3], 99.0);
    }

    #[test]
    fn a_flipped_curve_stays_settled() {
        let mut points = combed(s_shape());
        crate::ui_egui::content_windows::flip_control_points(&mut points, &mut None);
        let flipped = points.clone();
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 0);
        assert_eq!(points, flipped);
    }

    #[test]
    fn degenerate_inputs_stay_finite() {
        for points in degenerate_cases() {
            let (after, _) = run(points);
            assert_finite(&after);
        }
    }
}
