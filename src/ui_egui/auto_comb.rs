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
const SOLVED_TOLERANCE: f64 = 1.0e-7;
const UNCHANGED_LOG_LENGTH: f64 = 1.0e-6;
const STORAGE_ROUNDING: f64 = 1.0e-6;
const MAX_PASSES: usize = 4;
const MAX_ITERATIONS: usize = 60;
const MIN_DAMPING: f64 = 1.0e-9;
const INITIAL_DAMPING: f64 = 1.0e-3;
const MAX_DAMPING: f64 = 1.0e12;
const CONVERGED_DECREASE: f64 = 1.0e-14;
const GAP_WEIGHTS: [f64; 3] = [1.0e2, 1.0e4, 1.0e6];
const TARGET_WEIGHT: f64 = 1.0;
const LENGTH_WEIGHT: f64 = 0.1;
const DRAGGED_POINT_WEIGHT: f64 = 0.1;
const DRAGGED_HANDLE_WEIGHT: f64 = 100.0;

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

    // Each end's curvature with its slopes along the logs of the start and end lengths.
    fn curvature_slopes(&self, lengths: (f64, f64)) -> [(f64, [f64; 2]); 2] {
        let (beta, gamma0, gamma1) = self.coefficients();
        let (h0, h1) = lengths;
        let k0 = (gamma0 - beta * h1) / (h0 * h0);
        let k1 = (gamma1 - beta * h0) / (h1 * h1);
        [
            (k0, [-2.0 * k0, -beta * h1 / (h0 * h0)]),
            (k1, [-beta * h0 / (h1 * h1), -2.0 * k1]),
        ]
    }

    // Exact, as each end's numerator is linear in the far length: [start, end] over the length boxes.
    fn curvature_ranges(&self, boxes: [(f64, f64); 2]) -> [(f64, f64); 2] {
        let (beta, gamma0, gamma1) = self.coefficients();
        let range = |gamma: f64, near: (f64, f64), far: (f64, f64)| {
            let (a, b) = (gamma - beta * far.0, gamma - beta * far.1);
            let (low, high) = (a.min(b), a.max(b));
            let squared = |length: f64| length * length;
            (
                low / squared(if low < 0.0 { near.0 } else { near.1 }),
                high / squared(if high > 0.0 { near.0 } else { near.1 }),
            )
        };
        [
            range(gamma0, boxes[0], boxes[1]),
            range(gamma1, boxes[1], boxes[0]),
        ]
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

fn is_within(k: f64, target: f64, tolerance: f64, slack: f64) -> bool {
    (k - target).abs() <= tolerance * target.abs().max(k.abs()).max(1.0) + slack
}

fn meets(k: f64, slack: f64, target: f64) -> bool {
    is_within(k, target, CURVATURE_TOLERANCE, slack)
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

    fn length_boxes(&self, segment: usize) -> [(f64, f64); 2] {
        let (start, end) = self.max_lengths[segment];
        [start, end].map(|max_length| (MIN_SOLVED_LENGTH, max_length - STORAGE_ROUNDING))
    }

    // A point outside the square can't take handles, and a handle that points out of the square
    // from its border has no room.
    fn unfit_segments(&self) -> Vec<bool> {
        (0..self.segments.len())
            .map(|i| {
                !is_inside_square(self.positions[i])
                    || !is_inside_square(self.positions[i + 1])
                    || self.length_boxes(i).iter().any(|&(low, high)| high < low)
            })
            .collect()
    }

    fn chord(&self, segment: usize) -> f64 {
        (self.positions[segment + 1] - self.positions[segment]).length()
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

fn store_curvature(cp: &mut ControlPoint, k: f64) -> bool {
    assert!(k.is_finite(), "{k}");
    let before = cp.curvature();
    *cp.curvature_mut() = Some(k as f32);
    cp.curvature() != before
}

// A joint's two sides: the end of the segment arriving at it, the start of the one leaving it.
fn joint_side_ends(segment_count: usize, joint: usize) -> [Option<(usize, usize)>; 2] {
    [
        (joint > 0).then(|| (joint - 1, 1)),
        (joint < segment_count).then_some((joint, 0)),
    ]
}

// Where the directions force the two sides of a joint apart (an inflection, say), no lengths give G2:
// both its segments keep their lengths, which can leave a neighbouring joint stuck in turn.
fn freeze_infeasible_joints(layout: &Layout, frozen: &mut [bool]) {
    let segment_count = layout.segments.len();
    loop {
        let range = |frozen: &[bool], (segment, end): (usize, usize)| {
            let boxes = if frozen[segment] {
                let (start, finish) = layout.current[segment];
                if start.min(finish) < MIN_HANDLE_LENGTH {
                    return None;
                }
                [(start, start), (finish, finish)]
            } else {
                layout.length_boxes(segment)
            };
            Some(layout.segments[segment].curvature_ranges(boxes)[end])
        };
        let infeasible = (1..segment_count).find(|&joint| {
            let [arriving, leaving] = joint_side_ends(segment_count, joint).map(Option::unwrap);
            if frozen[arriving.0] && frozen[leaving.0] {
                return false;
            }
            match (range(frozen, arriving), range(frozen, leaving)) {
                (Some(a), Some(b)) => {
                    let (low, high) = (a.0.max(b.0), a.1.min(b.1));
                    low > high && !is_within(low, high, SOLVED_TOLERANCE, 0.0)
                }
                _ => false,
            }
        });
        match infeasible {
            Some(joint) => {
                frozen[joint - 1] = true;
                frozen[joint] = true;
            }
            None => return,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct JointGoal {
    target: f64,
    is_stored: bool,
    weight: f64,
    scale: f64,
}

// One side's curvature, with its slopes along the solver variables it depends on.
struct Side {
    k: f64,
    slopes: Vec<(usize, f64)>,
}

#[derive(Default)]
struct Residuals {
    values: Vec<f64>,
    rows: Vec<Vec<(usize, f64)>>,
}

impl Residuals {
    fn push(&mut self, value: f64, row: Vec<(usize, f64)>) {
        self.values.push(value);
        self.rows.push(row);
    }

    fn cost(&self) -> f64 {
        self.values.iter().map(|value| value * value).sum()
    }

    fn band_width(&self) -> usize {
        self.rows
            .iter()
            .filter_map(|row| {
                let low = row.iter().map(|&(variable, _)| variable).min()?;
                let high = row.iter().map(|&(variable, _)| variable).max()?;
                Some(high - low)
            })
            .max()
            .unwrap_or(0)
    }
}

// Solves for the logs of the free handle lengths: G2 at every joint with a free side is a hard
// constraint; each joint's stored curvature is a soft target, as is each length's current value.
struct Chain<'a> {
    layout: &'a Layout,
    frozen: &'a [bool],
    goals: Vec<JointGoal>,
    variables: Vec<Option<[usize; 2]>>,
    bounds: Vec<(f64, f64)>,
    start: Vec<f64>,
    start_lengths: Vec<f64>,
    length_weights: Vec<f64>,
    constrained: Vec<usize>,
}

impl<'a> Chain<'a> {
    fn new(
        layout: &'a Layout,
        frozen: &'a [bool],
        targets: &[Option<f64>],
        edit: CombEdit,
    ) -> Self {
        let segment_count = layout.segments.len();
        let mut variables = Vec::new();
        let mut bounds = Vec::new();
        let mut start = Vec::new();
        let mut start_lengths = Vec::new();
        let mut length_weights = Vec::new();
        let dragged_handle = edit
            .dragged_handle
            .and_then(|(index, slot)| handle_segment(targets.len(), index, slot))
            .map(|(segment, is_at_start)| (segment, usize::from(!is_at_start)));
        for (segment, &is_frozen) in frozen.iter().enumerate() {
            if is_frozen {
                variables.push(None);
                continue;
            }
            let current = layout.current[segment];
            let boxes = layout.length_boxes(segment);
            let first = bounds.len();
            for (end, length) in [current.0, current.1].into_iter().enumerate() {
                let (low, high) = boxes[end];
                let length = length.clamp(low, high);
                bounds.push((low.ln(), high.ln()));
                start_lengths.push(length);
                start.push(length.ln());
                let is_dragged = dragged_handle == Some((segment, end));
                length_weights.push(if is_dragged {
                    LENGTH_WEIGHT * DRAGGED_HANDLE_WEIGHT
                } else {
                    LENGTH_WEIGHT
                });
            }
            variables.push(Some([first, first + 1]));
        }
        let mut chain = Self {
            layout,
            frozen,
            goals: Vec::new(),
            variables,
            bounds,
            start,
            start_lengths,
            length_weights,
            constrained: Vec::new(),
        };
        let start_sides = chain.sides(&chain.start);
        chain.goals = (0..targets.len())
            .map(|joint| {
                let weight = if edit.dragged_handle.map(|(index, _)| index) == Some(joint) {
                    DRAGGED_HANDLE_WEIGHT
                } else if edit.dragged_point == Some(joint) {
                    DRAGGED_POINT_WEIGHT
                } else {
                    TARGET_WEIGHT
                };
                let chords: Vec<f64> = joint_side_ends(segment_count, joint)
                    .into_iter()
                    .flatten()
                    .map(|(segment, _)| layout.chord(segment))
                    .collect();
                let reach = (chords.iter().sum::<f64>() / chords.len().max(1) as f64)
                    .max(MIN_SOLVED_LENGTH);
                let target = targets[joint].unwrap_or_else(|| measured_curvature(layout, joint));
                JointGoal {
                    target,
                    is_stored: targets[joint].is_some(),
                    weight,
                    // Misses count relative to the target, or to the chords' scale near zero.
                    scale: reach / (1.0 + target.abs() * reach),
                }
            })
            .collect();
        chain.constrained = (1..segment_count)
            .filter(|&joint| {
                let sides = &start_sides[joint];
                sides.len() == 2 && sides.iter().any(|side| !side.slopes.is_empty())
            })
            .collect();
        chain
    }

    fn lengths(&self, x: &[f64]) -> Vec<(f64, f64)> {
        let length = |variable: usize| {
            if x[variable] == self.start[variable] {
                self.start_lengths[variable]
            } else {
                x[variable].exp()
            }
        };
        self.variables
            .iter()
            .enumerate()
            .map(|(segment, variables)| match variables {
                Some([start, end]) => (length(*start), length(*end)),
                None => self.layout.current[segment],
            })
            .collect()
    }

    fn sides(&self, x: &[f64]) -> Vec<Vec<Side>> {
        let lengths = self.lengths(x);
        let segment_count = self.layout.segments.len();
        (0..=segment_count)
            .map(|joint| {
                joint_side_ends(segment_count, joint)
                    .into_iter()
                    .flatten()
                    .filter_map(|(segment, end)| {
                        let lengths = lengths[segment];
                        let geometry = &self.layout.segments[segment];
                        geometry.curvatures(lengths)?;
                        let (k, slopes) = geometry.curvature_slopes(lengths)[end];
                        let slopes = match self.variables[segment] {
                            Some(variables) => variables.into_iter().zip(slopes).collect(),
                            None => Vec::new(),
                        };
                        Some(Side { k, slopes })
                    })
                    .collect()
            })
            .collect()
    }

    // Weighted target misses and length moves, then the G2 gaps times `gap_weight`; without the
    // objective, only the G2 gaps.
    fn residuals(&self, x: &[f64], with_objective: bool, gap_weight: f64) -> Residuals {
        let row = |terms: &[(usize, f64)], factor: f64| {
            terms
                .iter()
                .map(|&(variable, slope)| (variable, factor * slope))
                .collect()
        };
        let sides = self.sides(x);
        let mut residuals = Residuals::default();
        if with_objective {
            for (goal, sides) in self.goals.iter().zip(&sides) {
                if sides.iter().all(|side| side.slopes.is_empty()) {
                    continue;
                }
                let factor = goal.weight.sqrt() * goal.scale / sides.len() as f64;
                let k: f64 = sides.iter().map(|side| side.k).sum();
                let terms: Vec<(usize, f64)> = sides
                    .iter()
                    .flat_map(|side| side.slopes.iter().copied())
                    .collect();
                residuals.push(
                    factor * (k - sides.len() as f64 * goal.target),
                    row(&terms, factor),
                );
            }
            for (variable, weight) in self.length_weights.iter().enumerate() {
                let factor = weight.sqrt();
                residuals.push(
                    factor * (x[variable] - self.start[variable]),
                    row(&[(variable, 1.0)], factor),
                );
            }
        }
        for &joint in &self.constrained {
            let factor = gap_weight.sqrt() * self.goals[joint].scale;
            let [arriving, leaving] = [&sides[joint][0], &sides[joint][1]];
            let mut terms = arriving.slopes.clone();
            terms.extend(leaving.slopes.iter().map(|&(v, slope)| (v, -slope)));
            residuals.push(factor * (arriving.k - leaving.k), row(&terms, factor));
        }
        residuals
    }

    // Levenberg-Marquardt inside the length bounds: a variable on a bound that the gradient pushes
    // outwards is held for that step.
    fn least_squares(&self, mut x: Vec<f64>, residuals: impl Fn(&[f64]) -> Residuals) -> Vec<f64> {
        let count = x.len();
        let mut damping = INITIAL_DAMPING;
        let mut current = residuals(&x);
        for _ in 0..MAX_ITERATIONS {
            let cost = current.cost();
            if cost == 0.0 {
                break;
            }
            let width = current.band_width();
            let stride = width + 1;
            let mut gradient = vec![0.0; count];
            let mut normal = vec![0.0; count * stride];
            for (value, row) in current.values.iter().zip(&current.rows) {
                for &(i, a) in row {
                    gradient[i] += value * a;
                    for &(j, b) in row.iter().filter(|&&(j, _)| j >= i) {
                        normal[i * stride + j - i] += a * b;
                    }
                }
            }
            let held: Vec<bool> = (0..count)
                .map(|i| {
                    let (low, high) = self.bounds[i];
                    (x[i] <= low && gradient[i] > 0.0) || (x[i] >= high && gradient[i] < 0.0)
                })
                .collect();
            let step = |damping: f64| {
                let mut matrix = normal.clone();
                let mut rhs: Vec<f64> = gradient.iter().map(|g| -g).collect();
                for i in 0..count {
                    if held[i] {
                        matrix[i * stride..(i + 1) * stride].fill(0.0);
                        for k in i.saturating_sub(width)..i {
                            matrix[k * stride + i - k] = 0.0;
                        }
                        matrix[i * stride] = 1.0;
                        rhs[i] = 0.0;
                    } else {
                        matrix[i * stride] += damping * normal[i * stride].max(f64::EPSILON);
                    }
                }
                solve_band(matrix, width, rhs)
            };
            let accepted = loop {
                if damping > MAX_DAMPING {
                    break None;
                }
                let trial: Option<Vec<f64>> = step(damping).map(|step| {
                    (0..count)
                        .map(|i| (x[i] + step[i]).clamp(self.bounds[i].0, self.bounds[i].1))
                        .collect()
                });
                match trial.map(|trial| {
                    let at_trial = residuals(&trial);
                    (trial, at_trial)
                }) {
                    Some((trial, at_trial)) if at_trial.cost() < cost => {
                        damping = (damping / 3.0).max(MIN_DAMPING);
                        break Some((trial, at_trial));
                    }
                    _ => damping *= 4.0,
                }
            };
            let Some((trial, at_trial)) = accepted else {
                break;
            };
            let is_stalled = cost - at_trial.cost() <= CONVERGED_DECREASE * cost;
            (x, current) = (trial, at_trial);
            if is_stalled {
                break;
            }
        }
        x
    }

    // The G2 gaps enter as a penalty of rising weight, which keeps the solve well conditioned where
    // a gap can't close; the last solve closes the gaps alone.
    fn solve(&self) -> Vec<f64> {
        let x = GAP_WEIGHTS.iter().fold(self.start.clone(), |x, &weight| {
            self.least_squares(x, |x| self.residuals(x, true, weight))
        });
        self.least_squares(x, |x| self.residuals(x, false, 1.0))
    }

    // A move below f32 resolution would only flip rounding bits of a handle that is already solved.
    fn without_rounding_moves(&self, x: Vec<f64>) -> Vec<f64> {
        x.into_iter()
            .zip(&self.start)
            .map(|(x, &start)| {
                if (x - start).abs() < UNCHANGED_LOG_LENGTH {
                    start
                } else {
                    x
                }
            })
            .collect()
    }

    // Only the worst: a gap that can't close stalls the last solve short of closing the others.
    fn worst_open_joint(&self, x: &[f64]) -> Option<usize> {
        let sides = self.sides(x);
        let gap = |joint: usize| {
            let [arriving, leaving] = [sides[joint][0].k, sides[joint][1].k];
            (arriving - leaving).abs() / arriving.abs().max(leaving.abs()).max(1.0)
        };
        self.constrained
            .iter()
            .copied()
            .filter(|&joint| gap(joint) > SOLVED_TOLERANCE)
            .max_by(|&a, &b| gap(a).total_cmp(&gap(b)))
    }

    // The current handles already meet every constraint and target, so a solve would only move
    // them by rounding.
    fn is_settled(&self) -> bool {
        let layout = self.layout;
        let free_segments = (0..layout.segments.len()).filter(|&i| !self.frozen[i]);
        let handles_fit = free_segments.clone().all(|segment| {
            let boxes = layout.length_boxes(segment);
            [(segment, OUTGOING, 0), (segment + 1, INCOMING, 1)]
                .into_iter()
                .all(|(index, slot, end)| {
                    let handle = layout.handles[index][slot];
                    let (low, high) = boxes[end];
                    let length = handle.length();
                    (low - STORAGE_ROUNDING..=high + STORAGE_ROUNDING).contains(&length)
                        && handle
                            .unit()
                            .is_some_and(|u| is_aligned(u, layout.directions[index][slot]))
                })
        });
        handles_fit
            && (0..self.goals.len())
                .filter(|&joint| self.is_free_joint(joint))
                .all(|joint| {
                    let sides = layout.joint_sides(joint);
                    let target = self.goals[joint].target;
                    let meets_target = sides.iter().all(|&(k, slack)| meets(k, slack, target));
                    let meets_other_side = match sides[..] {
                        [(a, a_slack), (b, b_slack)] => meets(a, a_slack + b_slack, b),
                        _ => true,
                    };
                    meets_target && meets_other_side
                })
    }

    fn is_free_joint(&self, joint: usize) -> bool {
        joint_side_ends(self.layout.segments.len(), joint)
            .into_iter()
            .flatten()
            .any(|(segment, _)| !self.frozen[segment])
    }

    fn unstored_seeds(&self) -> Vec<Option<f64>> {
        self.goals
            .iter()
            .enumerate()
            .map(|(joint, goal)| {
                (self.is_free_joint(joint) && !goal.is_stored).then_some(goal.target)
            })
            .collect()
    }

    fn achieved_curvatures(&self, x: &[f64]) -> Vec<Option<f64>> {
        self.sides(x)
            .iter()
            .enumerate()
            .map(|(joint, sides)| {
                (self.is_free_joint(joint) && !sides.is_empty())
                    .then(|| sides.iter().map(|side| side.k).sum::<f64>() / sides.len() as f64)
            })
            .collect()
    }
}

// Cholesky on a symmetric positive definite band matrix, stored as each row's diagonal and the
// `width` entries right of it.
fn solve_band(mut band: Vec<f64>, width: usize, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let size = rhs.len();
    let stride = width + 1;
    let at = |i: usize, j: usize| i * stride + j - i;
    for i in 0..size {
        let end = size.min(i + stride);
        let pivot = band[at(i, i)];
        if pivot <= 0.0 || pivot.is_nan() {
            return None;
        }
        let root = pivot.sqrt();
        for j in i..end {
            band[at(i, j)] /= root;
        }
        for k in i + 1..end {
            for j in k..end {
                band[at(k, j)] -= band[at(i, k)] * band[at(i, j)];
            }
        }
        rhs[i] /= root;
        for j in i + 1..end {
            rhs[j] -= band[at(i, j)] * rhs[i];
        }
    }
    for i in (0..size).rev() {
        let end = size.min(i + stride);
        let known: f64 = (i + 1..end).map(|j| band[at(i, j)] * rhs[j]).sum();
        rhs[i] = (rhs[i] - known) / band[at(i, i)];
    }
    rhs.iter().all(|x| x.is_finite()).then_some(rhs)
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

fn circle_curvature(a: V2, b: V2, c: V2) -> Option<f64> {
    let denominator = (b - a).length() * (c - b).length() * (c - a).length();
    let k = 2.0 * (b - a).cross(c - a) / denominator;
    (denominator > f64::EPSILON && k.is_finite()).then_some(k)
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

// A reseed drops stored curvatures the handles no longer meet.
fn kept_curvatures(points: &[ControlPoint], layout: &Layout, is_reseed: bool) -> Vec<Option<f64>> {
    points
        .iter()
        .enumerate()
        .map(|(joint, cp)| {
            stored_curvature(cp).filter(|&k| {
                !is_reseed
                    || layout
                        .joint_sides(joint)
                        .iter()
                        .all(|&(side, slack)| meets(side, slack, k))
            })
        })
        .collect()
}

fn comb_pass(points: &mut [ControlPoint], edit: CombEdit) -> CombPass {
    let layout = Layout::new(points, edit.dragged_handle);
    let targets = kept_curvatures(points, &layout, edit.is_reseed);
    let mut frozen = layout.unfit_segments();
    let (lengths, curvatures) = loop {
        freeze_infeasible_joints(&layout, &mut frozen);
        let chain = Chain::new(&layout, &frozen, &targets, edit);
        if chain.is_settled() {
            break (layout.current.clone(), chain.unstored_seeds());
        }
        let x = chain.solve();
        let Some(joint) = chain.worst_open_joint(&x) else {
            let x = chain.without_rounding_moves(x);
            break (chain.lengths(&x), chain.achieved_curvatures(&x));
        };
        frozen[joint - 1] = true;
        frozen[joint] = true;
    };
    let mut is_changed = false;
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
    for (cp, k) in points.iter_mut().zip(curvatures) {
        if let Some(k) = k {
            is_changed |= store_curvature(cp, k);
        }
    }
    CombPass {
        unsolved_segments: frozen.iter().filter(|&&is_frozen| is_frozen).count(),
        is_changed,
    }
}

// A handle that lands on the square's border can need a second pass to settle.
pub fn apply_auto_comb(points: &mut [ControlPoint], edit: CombEdit) -> usize {
    if points.len() < 2 {
        return 0;
    }
    let mut pass = CombPass::default();
    for _ in 0..MAX_PASSES {
        pass = comb_pass(points, edit);
        if !pass.is_changed {
            break;
        }
    }
    pass.unsolved_segments
}

// The drag's component along the handle sets the joint's curvature target, as stretching the handle
// along its current direction would with the rest held; the next pass turns the joint to the
// handle's new direction and weights that target above the others.
fn set_curvature_from_drag(points: &mut [ControlPoint], index: usize, slot: usize, target: V2) {
    let Some((segment, is_at_start)) = handle_segment(points.len(), index, slot) else {
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
    if let Some((k, _)) = oriented.curvatures((length, current.1)) {
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
    let offset = target - *points[index].val();
    drag_handle_by(points, index, slot, offset, is_auto_comb_running);
}

pub fn drag_handle_by(
    points: &mut [ControlPoint],
    index: usize,
    slot: usize,
    offset: ControlPointValue,
    is_auto_comb_running: bool,
) {
    if points[index].tangents()[slot].is_none() {
        return;
    }
    if is_auto_comb_running {
        set_curvature_from_drag(points, index, slot, sv(*points[index].val() + offset));
    }
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
    fn lengthening_a_handle_sets_its_curvature_and_the_curve_follows() {
        let mut points = combed(c_shape());
        let before = stored(&points);
        let outgoing = handle_offset(&points[2], OUTGOING);
        let mut target = *points[2].val();
        target[0] += (outgoing.x * 1.15) as f32;
        target[1] += (outgoing.y * 1.15) as f32;
        drag_handle_to(&mut points, 2, OUTGOING, target, true);
        let dragged_target = points[2].curvature().unwrap() as f64;
        assert!(
            relative_gap(dragged_target, before[2] as f64) > 0.01,
            "{dragged_target} {before:?}"
        );

        assert_eq!(apply_auto_comb(&mut points, handle_drag(2, OUTGOING)), 0);
        let after = stored(&points);
        assert!(
            relative_gap(after[2] as f64, dragged_target) < 0.01,
            "{dragged_target} {after:?}"
        );
        let length = handle_length(&points[2], OUTGOING);
        assert!(
            relative_gap(length, outgoing.length() * 1.15) < 0.01,
            "{length}"
        );
        for segment in 0..points.len() - 1 {
            assert_meets_stored(&points, segment);
        }
        assert_g2(&points, 1.0e-4);
    }

    #[test]
    fn turning_a_handle_turns_its_partner_and_keeps_its_curvature() {
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
        let after = stored(&points);
        assert!(
            relative_gap(after[2] as f64, before[2] as f64) < 0.05,
            "{before:?} {after:?}"
        );
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
    fn an_inflection_the_directions_force_is_counted_and_left_unchanged() {
        let mut points = vec![
            point([0.2, 0.5], [0.0, -0.1], [0.0, 0.1]),
            point([0.5, 0.5], [-0.1, 0.0], [0.1, 0.0]),
            point([0.8, 0.5], [0.0, -0.1], [0.0, 0.1]),
        ];
        for cp in &mut points {
            *cp.curvature_mut() = Some(2.0);
        }
        let before = points.clone();
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 2);
        assert_eq!(points, before);
    }

    #[test]
    fn a_far_curvature_target_is_approached_without_breaking_g2() {
        let mut points = combed(c_shape());
        let before = stored(&points);
        *points[2].curvature_mut() = Some(before[2] * 1.5);
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 0);
        assert_g2(&points, 1.0e-4);
        let after = stored(&points)[2];
        assert!(after > before[2] && after < before[2] * 1.5, "{after}");
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
        let mut not_reseeded = stale.clone();
        apply_auto_comb(&mut stale, RESEED);
        for (reseeded, original) in stale.iter().zip(&points) {
            assert_eq!(reseeded.tangents(), original.tangents());
        }
        let reseeded = stale[2].curvature().unwrap() as f64;
        assert!(
            relative_gap(reseeded, before[2] as f64) < 1e-3,
            "{reseeded}"
        );
        apply_auto_comb(&mut not_reseeded, CombEdit::default());
        assert_ne!(not_reseeded[2].tangents(), points[2].tangents());

        let mut edited = points.clone();
        edited[3].tangents_mut()[OUTGOING] = edited[3].tangents()[OUTGOING].map(|t| t * 2.0);
        *edited[3].curvature_mut() = Some(99.0);
        apply_auto_comb(&mut edited, RESEED);
        let after = stored(&edited);
        let kept = relative_gap(after[0] as f64, before[0] as f64);
        assert!(kept < 0.05, "{before:?} {after:?}");
        assert!(relative_gap(after[3] as f64, 99.0) > 0.5, "{after:?}");
        assert_g2(&edited, 1.0e-4);
    }

    #[test]
    fn curvatures_missing_from_a_g2_curve_are_stored_without_moving_a_handle() {
        let combed = combed(c_shape());
        let mut points = combed.clone();
        for cp in &mut points {
            *cp.curvature_mut() = None;
        }
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 0);
        for (seeded, original) in points.iter().zip(&combed) {
            assert_eq!(seeded.tangents(), original.tangents());
            let gap = relative_gap(
                seeded.curvature().unwrap() as f64,
                original.curvature().unwrap() as f64,
            );
            assert!(gap < 1e-3, "{points:?}");
        }
    }

    #[test]
    fn a_flipped_curve_stays_settled() {
        let mut points = combed(s_shape());
        crate::ui_egui::content_windows::flip_control_points(&mut points, &mut None);
        let flipped = points.clone();
        assert_eq!(apply_auto_comb(&mut points, CombEdit::default()), 0);
        assert_eq!(points, flipped);
    }

    fn random_smooth_curves(count: usize) -> Vec<Vec<ControlPoint>> {
        let mut seed: u64 = 4321;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        (0..count)
            .map(|_| {
                let len = 3 + (next() * 5.0) as usize;
                let positions: Vec<[f32; 2]> = (0..len)
                    .map(|_| [0.05 + 0.9 * next(), 0.05 + 0.9 * next()])
                    .collect();
                (0..len)
                    .map(|i| {
                        let [previous, here, after] =
                            [i.saturating_sub(1), i, (i + 1).min(len - 1)].map(|j| positions[j]);
                        let chord = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
                        let travel = [after[0] - previous[0], after[1] - previous[1]];
                        let outgoing = chord(here, after).max(0.05) * (0.15 + 0.35 * next());
                        let incoming = chord(previous, here).max(0.05) * (0.15 + 0.35 * next());
                        mirrored(here, along(travel, outgoing), incoming)
                    })
                    .collect()
            })
            .collect()
    }

    fn assert_g2(points: &[ControlPoint], tolerance: f64) {
        for joint in 1..points.len() - 1 {
            let gap = joint_gap(points, joint);
            assert!(gap < tolerance, "joint {joint} gap {gap}: {points:?}");
        }
    }

    fn drag_point(points: &mut [ControlPoint], index: usize, offset: V2) -> usize {
        points[index].val_mut()[0] += offset.x as f32;
        points[index].val_mut()[1] += offset.y as f32;
        let edit = CombEdit {
            dragged_point: Some(index),
            ..Default::default()
        };
        apply_auto_comb(points, edit)
    }

    fn assert_g2_to_storage_precision(points: &[ControlPoint]) {
        let layout = Layout::new(points, None);
        let precision = f64::from(f32::EPSILON) / STORAGE_ROUNDING;
        for joint in 1..points.len() - 1 {
            let [(a, a_slack), (b, b_slack)] = layout.joint_sides(joint)[..] else {
                panic!("joint {joint} has a degenerate side: {points:?}");
            };
            let slack = precision * (a_slack + b_slack) / a.abs().max(b.abs()).max(1.0);
            let gap = joint_gap(points, joint);
            assert!(gap < 1.0e-6 + slack, "joint {joint} gap {gap}: {points:?}");
        }
    }

    #[test]
    fn random_smooth_curves_get_g2_at_every_interior_joint() {
        let mut checked = 0;
        for points in random_smooth_curves(300) {
            let (after, unsolved) = run(points);
            if unsolved == 0 {
                assert_g2_to_storage_precision(&after);
                checked += 1;
            }
        }
        assert!(checked > 250, "{checked}");
    }

    #[test]
    fn few_segments_of_random_smooth_curves_are_unsolved() {
        let curves = random_smooth_curves(300);
        let segments: usize = curves.iter().map(|points| points.len() - 1).sum();
        let mut moved = 0;
        let mut seed = 0usize;
        let enabled: usize = curves
            .into_iter()
            .map(|points| {
                let (mut points, unsolved) = run(points);
                seed = (seed * 7 + 3) % 1009;
                let angle = seed as f64;
                let index = seed % points.len();
                let offset = V2::new(angle.cos(), angle.sin()) * 0.01;
                moved += drag_point(&mut points, index, offset);
                unsolved
            })
            .sum();
        assert!(enabled * 25 <= segments, "{enabled} of {segments}");
        assert!(moved * 25 <= segments, "{moved} of {segments}");
    }

    #[test]
    fn dragging_a_point_keeps_g2_and_the_other_curvatures_close() {
        let mut points = combed(c_shape());
        let before = stored(&points);
        assert_eq!(drag_point(&mut points, 2, V2::new(0.01, 0.005)), 0);
        assert_g2(&points, 1.0e-4);
        let after = stored(&points);
        for joint in [0, 1, 3, 4] {
            let gap = relative_gap(after[joint] as f64, before[joint] as f64);
            assert!(gap < 0.1, "joint {joint}: {before:?} -> {after:?}");
        }
        let released = points.clone();
        apply_auto_comb(&mut points, CombEdit::default());
        assert_eq!(points, released);
    }

    #[test]
    fn degenerate_inputs_stay_finite() {
        for points in degenerate_cases() {
            let (after, _) = run(points);
            assert_finite(&after);
        }
    }

    #[test]
    fn band_solve_matches_the_full_system() {
        let (size, width) = (7, 2);
        let full = |i: usize, j: usize| match i.abs_diff(j) {
            0 => 4.0 + i as f64,
            1 => -1.0 + 0.1 * i.min(j) as f64,
            2 => 0.5,
            _ => 0.0,
        };
        let band: Vec<f64> = (0..size)
            .flat_map(|i| (i..=i + width).map(move |j| if j < size { full(i, j) } else { 0.0 }))
            .collect();
        let expected: Vec<f64> = (0..size).map(|i| i as f64 - 2.5).collect();
        let rhs: Vec<f64> = (0..size)
            .map(|i| (0..size).map(|j| full(i, j) * expected[j]).sum())
            .collect();
        let solution = solve_band(band, width, rhs).unwrap();
        for (x, e) in solution.iter().zip(&expected) {
            assert!((x - e).abs() < 1.0e-12, "{solution:?}");
        }
    }

    #[test]
    fn band_solve_rejects_a_matrix_that_is_not_positive_definite() {
        assert_eq!(
            solve_band(vec![1.0, 2.0, 1.0, 0.0], 1, vec![1.0, 1.0]),
            None
        );
    }
}
