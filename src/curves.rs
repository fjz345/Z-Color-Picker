//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use crate::common::SplineMode;
use crate::control_point::{ControlPoint, ControlPointType};
#[allow(unused_imports)]
use crate::error::Result;
use ecolor::{Color32, HsvaGamma};
use eframe::egui::{self, lerp, Sense, Ui};
use eframe::emath;
use eframe::epaint::{Pos2, Rect, Shape, Stroke, Vec2};
use egui::epaint::PathShape;
use splines::{Interpolation, Key, Spline};

use crate::math::{add_array_array, mul_array};

pub fn ui_ordered_control_points(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    marked_control_point_index: &Option<usize>,
    _is_middle_interpolated: bool,
    parent_response: &egui::Response,
    show_bezier_tangents: bool,
) -> (
    /*
        control_point_dragged_point_response,
        control_point_selected_index,
        control_point_hovering_point_option,
        tangent_selected_index
        tangent_dragged_response
    */
    Option<egui::Response>,
    Option<usize>,
    Option<(egui::Response, usize)>,
    Option<usize>,
    Option<egui::Response>,
) {
    const SHOW_LINEAR_LINE: bool = false;
    let num_control_points = control_points.len();
    if num_control_points <= 0 {
        return (None, None, None, None, None);
    }
    let to_screen = emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0)),
        parent_response.rect,
    );

    let mut dragged_point_response = None;
    let mut dragged_tangent_response = None;

    let control_point_radius = 8.0;

    // Fill Circle
    let mut selected_index = *marked_control_point_index;
    let mut tangent_selected_index = None;
    let mut hovering_control_point = None;

    let control_point_draw_size: Vec2 = Vec2::splat(2.0 * control_point_radius);
    let control_point_shapes_fill: Vec<Shape> = control_points
        .into_iter()
        .enumerate()
        .take(num_control_points)
        .map(|(i, key)| {
            let is_control_point_first = i == 0;
            let is_control_point_last = i == control_points.len() - 1;

            let is_selected_index: bool = if let Some(index) = selected_index {
                index == i
            } else {
                false
            };
            let control_point = &key;
            let mut point_xy: Pos2 = Pos2::new(
                control_point.val()[0].clamp(0.0, 1.0),
                1.0 - control_point.val()[1].clamp(0.0, 1.0),
            );

            let point_in_screen: Pos2 = to_screen.transform_pos(point_xy);
            let control_point_ui_rect =
                Rect::from_center_size(point_in_screen, control_point_draw_size);
            let control_point_response = ui.interact(
                control_point_ui_rect,
                parent_response.id.with(i),
                Sense::click_and_drag(),
            );

            // TODO: CHECK THIS LOGIC (is_inactive, is_inactive_click_or_drag)
            let is_inactive: bool = false;
            let mut is_inactive_click_or_drag: bool = false;

            // if is_middle_interpolated {
            //     if !(i == first_index || i == last_index) {
            //         is_inactive = true;
            //     }
            // }

            if control_point_response.dragged_by(egui::PointerButton::Primary) {
                is_inactive_click_or_drag = is_inactive;

                if !is_inactive {
                    point_xy += control_point_response.drag_delta() / parent_response.rect.size();
                    selected_index = Some(i);
                    dragged_point_response = Some(control_point_response.clone());
                }
            }

            if control_point_response.hovered() {
                hovering_control_point = Some((control_point_response, i));
            }

            let point_as_color = HsvaGamma {
                h: control_point.val()[2],
                s: control_point.val()[0],
                v: control_point.val()[1],
                a: 1.0,
            };
            let color_to_show = if !is_inactive_click_or_drag {
                point_as_color
            } else {
                HsvaGamma {
                    h: point_as_color.h,
                    s: point_as_color.s * 0.75,
                    v: point_as_color.v * 0.6,
                    a: point_as_color.a,
                }
            };

            let mut stroke: Stroke = ui.style().noninteractive().fg_stroke;
            if is_inactive {
                stroke.color = Color32::LIGHT_GRAY;
                stroke.width *= 6.0;
                ui.painter().add(Shape::circle_stroke(
                    point_in_screen,
                    1.8 * control_point_radius,
                    stroke,
                ));
            }

            if show_bezier_tangents {
                for (tangent_index, tangent) in key.tangents().iter().enumerate() {
                    if tangent_index == 0 && is_control_point_first {
                        // Skip
                        continue;
                    }
                    if tangent_index == 1 && is_control_point_last {
                        // Skip
                        continue;
                    }
                    if let Some(tang) = tangent {
                        let tang_xy = [key.val()[0] + tang.val[0], key.val()[1] + tang.val[1]];
                        let tangent_point_xy: Pos2 =
                            Pos2::new(tang_xy[0].clamp(0.0, 1.0), 1.0 - tang_xy[1].clamp(0.0, 1.0));
                        let mut tangent_in_screen: Pos2 = to_screen.transform_pos(tangent_point_xy);
                        let tangent_draw_scale = 0.7;
                        let _tangent_draw_size = tangent_draw_scale * control_point_draw_size;

                        if is_selected_index {
                            let tangent_ui_rect =
                                Rect::from_center_size(tangent_in_screen, control_point_draw_size);
                            let tangent_response = ui.interact(
                                tangent_ui_rect,
                                parent_response.id.with(tangent_index + 1000),
                                Sense::drag(),
                            );

                            if tangent_response.dragged_by(egui::PointerButton::Primary) {
                                tangent_in_screen +=
                                    tangent_response.drag_delta() / parent_response.rect.size();
                                tangent_selected_index = Some(tangent_index);
                                dragged_tangent_response = Some(tangent_response.clone());
                                selected_index = Some(i);
                            }

                            let active_tangent_shape = Shape::circle_stroke(
                                tangent_in_screen,
                                tangent_draw_scale * control_point_radius,
                                stroke,
                            );

                            let active_aux_stroke =
                                Stroke::new(1.0, Color32::WHITE.linear_multiply(0.25));
                            // Draw lines to the Cp
                            ui.painter().add(PathShape::line(
                                vec![point_in_screen, tangent_in_screen],
                                active_aux_stroke,
                            ));

                            ui.painter().add(active_tangent_shape);
                        } else {
                            let inactive_aux_stroke =
                                Stroke::new(1.0, Color32::WHITE.linear_multiply(0.002));
                            // Draw lines to the Cp
                            ui.painter().add(PathShape::line(
                                vec![point_in_screen, tangent_in_screen],
                                inactive_aux_stroke,
                            ));

                            let inactive_tangent_shape = Shape::circle_stroke(
                                tangent_in_screen,
                                (0.2 / 0.7) * tangent_draw_scale * control_point_radius,
                                stroke,
                            );
                            ui.painter().add(inactive_tangent_shape);
                        }
                    };
                }
            }

            Shape::circle_filled(point_in_screen, 1.8 * control_point_radius, color_to_show)
        })
        .collect();

    // Circle Stroke
    let control_point_shapes: Vec<Shape> = control_points
        .into_iter()
        .enumerate()
        .take(num_control_points)
        .map(|(i, key)| {
            let mut point = Pos2::new(key.val()[0], 1.0 - key.val()[1]);
            point = to_screen.from().clamp(point);

            let point_in_screen = to_screen.transform_pos(point);
            let stroke: Stroke = ui.style().interact(parent_response).fg_stroke;

            let control_point_is_first_or_last = i == 0 || i == num_control_points - 1;
            let control_point_shape: Shape = if control_point_is_first_or_last {
                Shape::rect_stroke(
                    Rect::from_center_size(
                        point_in_screen,
                        Vec2::new(control_point_radius, control_point_radius),
                    ),
                    0.0,
                    stroke,
                    eframe::egui::StrokeKind::Middle,
                )
            } else {
                Shape::circle_stroke(point_in_screen, control_point_radius, stroke)
            };

            control_point_shape
        })
        .collect();

    if SHOW_LINEAR_LINE {
        let points_in_screen: Vec<Pos2> = control_points
            .into_iter()
            .take(num_control_points)
            .map(|key| {
                let point = Pos2::new(key.val()[0], 1.0 - key.val()[1]);
                to_screen * point
            })
            .collect();

        let aux_stroke = Stroke::new(1.0, Color32::RED.linear_multiply(0.25));
        ui.painter()
            .add(PathShape::line(points_in_screen, aux_stroke));
    }

    ui.painter().extend(control_point_shapes_fill);
    ui.painter().extend(control_point_shapes);
    if let Some(marked_index) = marked_control_point_index {
        let key = &control_points[*marked_index];
        let mut point = Pos2::new(key.val()[0], 1.0 - key.val()[1]);
        point = to_screen.from().clamp(point);
        let stroke: Stroke = ui.style().interact(parent_response).fg_stroke;

        let point_in_screen = to_screen.transform_pos(point);
        let shape = Shape::rect_stroke(
            Rect::from_center_size(
                point_in_screen,
                Vec2::new(control_point_radius * 0.5, control_point_radius * 0.5),
            ),
            0.0,
            stroke,
            eframe::egui::StrokeKind::Middle,
        );
        ui.painter().add(shape);
    }

    (
        dragged_point_response,
        selected_index,
        hovering_control_point,
        tangent_selected_index,
        dragged_tangent_response,
    )
}

pub fn flatten_control_points(control_points: &[ControlPoint]) -> Vec<ControlPoint> {
    let mut control_points_flattened: Vec<ControlPoint> = Vec::new();

    let inc_all_prev_hue_values = |vec: &mut Vec<ControlPoint>, val: f32| {
        for a in &mut vec.iter_mut() {
            a.val_mut()[2] += val;
        }
    };

    for (i, cp) in control_points.iter().enumerate() {
        if i == 0 {
            control_points_flattened.push(cp.clone());
            continue;
        }

        let prev = &mut control_points_flattened[i - 1];
        let hue_diff = cp.val().h() - prev.val().h();
        if hue_diff.abs() <= 0.5 {
            control_points_flattened.push(cp.clone());
        } else {
            if hue_diff > 0.0 {
                inc_all_prev_hue_values(&mut control_points_flattened, 1.0);
                control_points_flattened.push(cp.clone());
            } else {
                inc_all_prev_hue_values(&mut control_points_flattened, -1.0);
                control_points_flattened.push(cp.clone());
            }
        }
    }

    control_points_flattened
}

pub fn find_spline_max_t(spline: &Spline<f32, ControlPointType>) -> f32 {
    let vec_of_t_values: Vec<f32> = spline.into_iter().map(|k| k.t).collect();
    let max_t = vec_of_t_values
        .into_iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);
    max_t
}

pub fn generate_spline_points_with_distance(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    t_distance: f32,
) -> Vec<ControlPointType> {
    let mut spline_samples = Vec::new();

    if control_points.len() <= 1 {
        return spline_samples;
    }

    let spline = control_points_to_spline(&control_points, spline_mode);
    let spline_max_t = find_spline_max_t(&spline) as f32;
    let mut curr_t = 0.0;
    while curr_t <= spline_max_t {
        let spline_sample = spline.clamped_sample(curr_t);

        match spline_sample {
            Some(key) => spline_samples.push(key),
            None => {}
        }

        curr_t += t_distance;
    }

    let last_spline_sample = spline.clamped_sample(spline_max_t);
    match last_spline_sample {
        Some(key) => spline_samples.push(key),
        None => todo!(),
    }

    spline_samples
}

pub fn sub_divide_control_points(
    control_points: &[ControlPoint],
    distance_per_point: f32,
) -> Vec<ControlPoint> {
    let capacity: usize = control_points.len() * 4;
    let mut sub_divided: Vec<ControlPoint> = Vec::with_capacity(capacity);

    for i in 1..control_points.len() {
        sub_divided.push(control_points[i - 1].clone());
        let hue_to_use = control_points[i - 1].val()[2];
        let first = control_points[i - 1].val().pos2();
        let last = control_points[i].val().pos2();
        let dir = (last - first).normalized();
        let mut sub_div_start = first;
        let mut distance_to_end = (last - first).dot(last - first).sqrt();
        while distance_to_end > distance_per_point {
            let new: Pos2 = sub_div_start + distance_per_point * dir;
            distance_to_end -= distance_per_point;

            let mut new_cp = ControlPoint::default();
            *new_cp.val_mut() = ControlPointType::new(new.x, new.y, hue_to_use);
            *new_cp.t_mut() = lerp(*control_points[i - 1].t()..=*control_points[i].t(), 0.5);

            sub_divided.push(new_cp);
            sub_div_start = new;
        }
        let last_new = sub_div_start + distance_to_end.max(0.0) * dir;

        let mut new_cp = ControlPoint::default();
        *new_cp.val_mut() = ControlPointType::new(last_new.x, last_new.y, hue_to_use);
        *new_cp.t_mut() = *control_points[i].t();

        let last_cp = new_cp;
        sub_divided.push(last_cp);
    }
    if control_points.last().is_some() {
        sub_divided.push(control_points.last().unwrap().clone());
    }

    sub_divided
}

pub fn ui_ordered_spline_gradient(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    parent_response: &egui::Response,
) -> Option<egui::Response> {
    let num_control_points = control_points.len();
    if num_control_points <= 1 {
        return None;
    }

    let response: egui::Response = ui.interact(
        parent_response.rect,
        parent_response.id.with(190124502),
        Sense::focusable_noninteractive(),
    );

    let to_screen = emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0)),
        response.rect,
    );

    // let sub_divided_control_points = sub_divide_control_points(control_points, 0.01);
    let flattened_points = flatten_control_points(control_points);
    let spline_points =
        generate_spline_points_with_distance(&flattened_points[..], spline_mode, 0.01);

    for i in 1..spline_points.len() {
        let first = spline_points[i - 1];
        let next = spline_points[i];

        // let spline = control_points_to_spline(&sub_divided_control_points, spline_mode);
        let segment_color = first.color();

        let control_point_radius = 8.0;

        let point_first = first.pos2();
        let point_next = next.pos2();
        let mut points_in_screen: Vec<Pos2> = Vec::with_capacity(2);
        let point_in_screen_first = to_screen * Pos2::new(point_first.x, 1.0 - point_first.y);
        let point_in_screen_next = to_screen * Pos2::new(point_next.x, 1.0 - point_next.y);
        points_in_screen.push(point_in_screen_first);
        points_in_screen.push(point_in_screen_next);

        let shape = PathShape::line(
            points_in_screen,
            Stroke::new(control_point_radius * 1.6, segment_color),
        );

        ui.painter().add(shape);
    }

    Some(response)
}

pub struct Bezier<const D: usize, const N: usize> {
    pub control_points: [[f32; D]; N],
}

impl<const D: usize, const N: usize> Bezier<D, N> {
    pub fn new() -> Self {
        Self {
            control_points: [[0.0; D]; N],
        }
    }

    pub fn get_at(&self, t: f32) -> [f32; D] {
        // https://en.wikipedia.org/wiki/B%C3%A9zier_curve
        let mut outer_sum: [f32; D] = [0.0; D];

        for i in 0..N {
            let inner_prod = num_integer::binomial(N as u64, i as u64) as f32
                * (1.0 - t).powi(N as i32 - i as i32)
                * t.powi(i as i32);
            let inner = mul_array(self.control_points[i].clone(), inner_prod);
            outer_sum = add_array_array(outer_sum, inner);
        }

        outer_sum
    }
}

pub fn control_points_to_spline(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> Spline<f32, ControlPointType> {
    match spline_mode {
        SplineMode::Linear => Spline::from_vec(
            control_points
                .iter()
                .enumerate()
                .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::Linear))
                .collect(),
        ),
        SplineMode::Bezier => Spline::from_vec(
            control_points
                .iter()
                .enumerate()
                .map(|(index, e)| {
                    Key::new(
                        index as f32,
                        *e.val(),
                        Interpolation::StrokeBezier(
                            *control_points[index].val()
                                + control_points[index].tangents()[0].unwrap_or_default(),
                            *control_points[index].val()
                                + control_points[index].tangents()[1].unwrap_or_default(),
                        ),
                    )
                })
                .collect(),
        ),
        SplineMode::HermiteBezier => {
            let mut catmul_rom_spline_vec = control_points.to_vec();
            if control_points.len() >= 1 {
                catmul_rom_spline_vec.insert(0, control_points.first().unwrap().clone());
            }

            if control_points.len() >= 1 {
                catmul_rom_spline_vec.push(control_points.last().unwrap().clone());
            }

            let new_spline = Spline::from_vec(
                catmul_rom_spline_vec
                    .iter()
                    .enumerate()
                    .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::CatmullRom))
                    .collect(),
            );

            new_spline
        }
        SplineMode::Polynomial => todo!(),
        _ => {
            log::info!("Not Implemented...");
            Spline::from_vec(
                control_points
                    .iter()
                    .enumerate()
                    .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::Linear))
                    .collect(),
            )
        }
    }
}
