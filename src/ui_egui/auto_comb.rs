use crate::datatypes::control_point::{create_tangent_for_control_point, ControlPoint};

const INCOMING: usize = 0;
const OUTGOING: usize = 1;
const MIN_CHORD: f64 = 1.0e-6;

type V2 = [f64; 2];

fn position(cp: &ControlPoint) -> V2 {
    let value = cp.val();
    [value[0] as f64, value[1] as f64]
}

fn add(a: V2, b: V2) -> V2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn scale(a: V2, factor: f64) -> V2 {
    [a[0] * factor, a[1] * factor]
}

fn chords(positions: &[V2]) -> Vec<f64> {
    positions
        .windows(2)
        .map(|pair| {
            let [x, y] = sub(pair[1], pair[0]);
            x.hypot(y).max(MIN_CHORD)
        })
        .collect()
}

// Derivatives at the knots of the C2 cubic through the points, with knots at cumulative chord
// length and zero second derivative at both ends, by the Thomas algorithm (diagonally dominant).
fn knot_derivatives(positions: &[V2], chords: &[f64]) -> Vec<V2> {
    let n = positions.len();
    let slope = |i: usize| scale(sub(positions[i + 1], positions[i]), 1.0 / chords[i]);
    let mut lower = vec![0.0; n];
    let mut diagonal = vec![2.0; n];
    let mut upper = vec![0.0; n];
    let mut rhs = vec![[0.0; 2]; n];
    upper[0] = 1.0;
    rhs[0] = scale(slope(0), 3.0);
    lower[n - 1] = 1.0;
    rhs[n - 1] = scale(slope(n - 2), 3.0);
    for i in 1..n - 1 {
        let (before, after) = (chords[i - 1], chords[i]);
        lower[i] = after;
        diagonal[i] = 2.0 * (before + after);
        upper[i] = before;
        rhs[i] = scale(
            add(scale(slope(i - 1), after), scale(slope(i), before)),
            3.0,
        );
    }
    for i in 1..n {
        let factor = lower[i] / diagonal[i - 1];
        diagonal[i] -= factor * upper[i - 1];
        rhs[i] = sub(rhs[i], scale(rhs[i - 1], factor));
    }
    let mut derivatives = vec![[0.0; 2]; n];
    derivatives[n - 1] = scale(rhs[n - 1], 1.0 / diagonal[n - 1]);
    for i in (0..n - 1).rev() {
        derivatives[i] = scale(
            sub(rhs[i], scale(derivatives[i + 1], upper[i])),
            1.0 / diagonal[i],
        );
    }
    derivatives
}

fn is_inside_square([x, y]: V2) -> bool {
    (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)
}

fn fraction_inside_square(point: V2, offset: V2) -> f64 {
    (0..2).fold(1.0, |fraction: f64, axis| {
        let end = point[axis] + offset[axis];
        if end > 1.0 {
            fraction.min((1.0 - point[axis]) / offset[axis])
        } else if end < 0.0 {
            fraction.min(-point[axis] / offset[axis])
        } else {
            fraction
        }
    })
}

fn set_handle(cp: &mut ControlPoint, slot: usize, offset: V2) {
    let point = position(cp);
    if !is_inside_square(point) {
        return;
    }
    let [s, v] = scale(offset, fraction_inside_square(point, offset).max(0.0));
    let tangent = cp.tangents_mut()[slot].get_or_insert_with(create_tangent_for_control_point);
    tangent[0] = s as f32;
    tangent[1] = v as f32;
}

pub fn apply_auto_comb(points: &mut [ControlPoint]) {
    if points.len() < 2 {
        return;
    }
    let positions: Vec<V2> = points.iter().map(position).collect();
    let chords = chords(&positions);
    let derivatives = knot_derivatives(&positions, &chords);
    for (segment, &chord) in chords.iter().enumerate() {
        let third = chord / 3.0;
        set_handle(
            &mut points[segment],
            OUTGOING,
            scale(derivatives[segment], third),
        );
        set_handle(
            &mut points[segment + 1],
            INCOMING,
            scale(derivatives[segment + 1], -third),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;
    use crate::ui_egui::control_points::bezier_handle;

    fn points(positions: &[[f32; 2]]) -> Vec<ControlPoint> {
        positions
            .iter()
            .map(|&[s, v]| ControlPoint::new_simple(ControlPointValue::new(s, v, 0.3), 0.0))
            .collect()
    }

    fn combed(positions: &[[f32; 2]]) -> Vec<ControlPoint> {
        let mut points = points(positions);
        apply_auto_comb(&mut points);
        points
    }

    fn cross(a: V2, b: V2) -> f64 {
        a[0] * b[1] - a[1] * b[0]
    }

    fn length(a: V2) -> f64 {
        a[0].hypot(a[1])
    }

    fn sv(value: ControlPointValue) -> V2 {
        [value[0] as f64, value[1] as f64]
    }

    fn end_curvatures(points: &[ControlPoint], segment: usize) -> (f64, f64) {
        let [p0, p1, p2, p3] = [
            sv(*points[segment].val()),
            sv(bezier_handle(&points[segment], OUTGOING)),
            sv(bezier_handle(&points[segment + 1], INCOMING)),
            sv(*points[segment + 1].val()),
        ];
        let start = 2.0 / 3.0 * cross(sub(p1, p0), sub(p2, p1)) / length(sub(p1, p0)).powi(3);
        let end = 2.0 / 3.0 * cross(sub(p2, p1), sub(p3, p2)) / length(sub(p3, p2)).powi(3);
        (start, end)
    }

    const SHAPES: [&[[f32; 2]]; 4] = [
        &[[0.2, 0.3], [0.4, 0.45], [0.5, 0.5], [0.6, 0.55], [0.8, 0.7]],
        &[
            [0.3, 0.3],
            [0.45, 0.35],
            [0.6, 0.5],
            [0.55, 0.65],
            [0.4, 0.7],
        ],
        &[[0.25, 0.33], [0.44, 0.38], [0.6, 0.5], [0.7, 0.65]],
        &[
            [0.3, 0.4],
            [0.32, 0.41],
            [0.6, 0.6],
            [0.65, 0.4],
            [0.7, 0.45],
        ],
    ];

    #[test]
    fn curvature_is_continuous_at_every_point_and_zero_at_the_ends() {
        for shape in SHAPES {
            let points = combed(shape);
            let curvatures: Vec<(f64, f64)> = (0..points.len() - 1)
                .map(|segment| end_curvatures(&points, segment))
                .collect();
            let scale = curvatures
                .iter()
                .fold(0.0f64, |m, &(a, b)| m.max(a.abs()).max(b.abs()));
            for joint in 1..curvatures.len() {
                let (arriving, leaving) = (curvatures[joint - 1].1, curvatures[joint].0);
                assert!(
                    (arriving - leaving).abs() <= 1e-3 * scale,
                    "{shape:?} joint {joint}: {arriving} vs {leaving}"
                );
            }
            let (first, last) = (curvatures[0].0, curvatures[curvatures.len() - 1].1);
            assert!(
                first.abs() <= 1e-3 * scale && last.abs() <= 1e-3 * scale,
                "{shape:?}"
            );
        }
    }

    #[test]
    fn handles_depend_on_the_points_alone() {
        for shape in SHAPES {
            let expected = combed(shape);
            let mut runaway = points(shape);
            for cp in &mut runaway {
                *cp.tangents_mut() = [Some(ControlPointValue::new(0.9, -0.7, 0.0)); 2];
            }
            apply_auto_comb(&mut runaway);
            for (actual, expected) in runaway.iter().zip(&expected).skip(1).take(shape.len() - 2) {
                assert_eq!(actual.tangents(), expected.tangents(), "{shape:?}");
            }

            let mut dragged = expected.clone();
            for step in [0.012f32; 12].into_iter().chain([-0.012; 12]) {
                dragged[2].val_mut()[1] += step;
                apply_auto_comb(&mut dragged);
            }
            dragged[2].val_mut()[1] = expected[2].val()[1];
            apply_auto_comb(&mut dragged);
            assert_eq!(dragged, expected, "{shape:?}");
        }
    }

    #[test]
    fn a_second_pass_changes_nothing() {
        for shape in SHAPES {
            let once = combed(shape);
            let mut twice = once.clone();
            apply_auto_comb(&mut twice);
            assert_eq!(twice, once, "{shape:?}");
        }
    }

    #[test]
    fn handles_keep_their_hue_offsets_and_the_outer_handles_stay() {
        let mut points = points(SHAPES[0]);
        for (i, cp) in points.iter_mut().enumerate() {
            let hue = 0.1 * i as f32;
            *cp.tangents_mut() = [
                Some(ControlPointValue::new(-0.01, 0.0, -hue)),
                Some(ControlPointValue::new(0.01, 0.0, hue)),
            ];
        }
        let before = points.clone();
        apply_auto_comb(&mut points);
        for (i, (cp, before)) in points.iter().zip(&before).enumerate() {
            for slot in [INCOMING, OUTGOING] {
                let [after, before] = [cp, before].map(|cp| cp.tangents()[slot].unwrap());
                assert_eq!(after.h(), before.h(), "point {i} slot {slot}");
            }
        }
        assert_eq!(
            points[0].tangents()[INCOMING],
            before[0].tangents()[INCOMING]
        );
        let last = points.len() - 1;
        assert_eq!(
            points[last].tangents()[OUTGOING],
            before[last].tangents()[OUTGOING]
        );
    }

    #[test]
    fn a_handle_that_would_leave_the_square_is_shortened_along_its_direction() {
        let shape = [[0.05, 0.1], [0.02, 0.6], [0.5, 0.97], [0.95, 0.6]];
        let positions: Vec<V2> = shape.iter().map(|&[s, v]| [s as f64, v as f64]).collect();
        let chords = chords(&positions);
        let derivatives = knot_derivatives(&positions, &chords);
        let points = combed(&shape);
        let mut shortened = 0;
        for (i, cp) in points.iter().enumerate() {
            for slot in [INCOMING, OUTGOING] {
                let Some(offset) = cp.tangents()[slot] else {
                    continue;
                };
                let offset = [offset[0] as f64, offset[1] as f64];
                let end = add(position(cp), offset);
                assert!(
                    end.iter().all(|c| (-1e-6..=1.0 + 1e-6).contains(c)),
                    "{end:?}"
                );
                let free = derivatives[i];
                assert!(
                    cross(offset, free).abs() <= 1e-6 * length(free),
                    "point {i} slot {slot}"
                );
                let unclipped_length = length(free) * chords[i.min(chords.len() - 1)] / 3.0;
                shortened += usize::from(length(offset) < unclipped_length * 0.999);
            }
        }
        assert!(
            shortened > 0,
            "the fixture should push a handle out of the square"
        );
    }

    #[test]
    fn two_points_get_a_straight_segment_with_third_length_handles() {
        let points = combed(&[[0.2, 0.3], [0.8, 0.6]]);
        let chord = [0.6, 0.3];
        let third = scale(chord, 1.0 / 3.0);
        let out = points[0].tangents()[OUTGOING].unwrap();
        let incoming = points[1].tangents()[INCOMING].unwrap();
        for (actual, expected) in [(out, third), (incoming, scale(third, -1.0))] {
            assert!((actual[0] as f64 - expected[0]).abs() < 1e-6, "{actual:?}");
            assert!((actual[1] as f64 - expected[1]).abs() < 1e-6, "{actual:?}");
        }
    }

    #[test]
    fn coincident_points_keep_finite_handles() {
        let points = combed(&[[0.3, 0.3], [0.3, 0.3], [0.6, 0.5], [0.6, 0.5]]);
        for cp in &points {
            for tangent in cp.tangents().iter().flatten() {
                assert!(tangent.val.iter().all(|c| c.is_finite()), "{points:?}");
            }
        }
    }
}
