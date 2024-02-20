use std::{
    borrow::{Borrow, BorrowMut},
    sync::Arc,
};

use eframe::{
    egui::{
        self,
        color_picker::{show_color, Alpha},
        ComboBox, InnerResponse, Label, Layout, Painter, PointerButton, Response, Sense, TextStyle,
        Ui, Widget,
    },
    emath::{lerp, remap_clamp},
    epaint::{self, pos2, vec2, Color32, HsvaGamma, Mesh, Pos2, Rect, Rgba, Shape, Stroke, Vec2},
};

use crate::{
    curves::{self, Bezier, PaintCurve},
    ui_common::{color_slider_2d, contrast_color},
};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ColorStringCopy {
    HEX,
    HEXNOA,
    SRGBHEX,
    HSV,
    HSVA,
    INT,
    FLOAT,
    RGB,
    SRGB,
    RGBA,
    SRGBA,
}

pub fn format_color_as(
    color: Color32,
    format_type: ColorStringCopy,
    no_alpha: Option<bool>,
) -> String {
    let formatted = match format_type {
        ColorStringCopy::HEX => match no_alpha {
            Some(no_alpha) => {
                if no_alpha {
                    format!("{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
                } else {
                    format!(
                        "{:02x}{:02x}{:02x}{:02x}",
                        color.a(),
                        color.r(),
                        color.g(),
                        color.b()
                    )
                }
            }
            _ => {
                format!(
                    "{:02x}{:02x}{:02x}{:02x}",
                    color.a(),
                    color.r(),
                    color.g(),
                    color.b()
                )
            }
        },
        ColorStringCopy::HEXNOA => {
            format!("{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
        }
        _ => {
            println!("Not Implemented {:?}", format_type);
            format!("rgb({}, {}, {})", color.r(), color.g(), color.b())
        }
    };
    formatted.to_uppercase()
}

const PREVIEWER_DEFAULT_VALUE: f32 = 100.0;
pub struct PreviewerData {
    pub points_preview_sizes: Vec<f32>,
}

impl PreviewerData {
    pub fn new(num: usize) -> Self {
        Self {
            points_preview_sizes: vec![PREVIEWER_DEFAULT_VALUE; num],
        }
    }
    pub fn reset_preview_sizes(&mut self) {
        for val in self.points_preview_sizes.iter_mut() {
            *val = PREVIEWER_DEFAULT_VALUE;
        }
    }

    pub fn enforce_min_size(&mut self, min_size: f32) {
        for point_ref in &mut self.points_preview_sizes {
            *point_ref = point_ref.max(min_size);
        }
    }
}

pub struct MainColorPickerData {
    pub hsva: HsvaGamma,
    pub alpha: Alpha,
    pub paint_curve: PaintCurve<f32, [f32; 3]>,
    pub dragging_bezier_index: Option<usize>,
    pub bezier_right_clicked: Option<usize>,
    pub last_modifying_bezier_index: Option<usize>,
    pub is_curve_locked: bool,
    pub is_hue_middle_interpolated: bool,
}

pub fn xyz_to_hsva(x: f32, y: f32, z: f32) -> HsvaGamma {
    HsvaGamma {
        h: x,
        s: y,
        v: 1.0 - z.clamp(0.0, 1.0),
        a: 1.0,
    }
}

pub fn slice_to_hsva(xyz: &[f32]) -> HsvaGamma {
    if (xyz.len() < 3) {
        panic!("dim need to be larger than 3");
    }
    let alpha = if xyz.len() >= 4 { xyz[3] } else { 1.0 };
    HsvaGamma {
        h: xyz[0],
        s: xyz[1],
        v: 1.0 - xyz[2].clamp(0.0, 1.0),
        a: alpha,
    }
}

pub fn main_color_picker(
    ui: &mut Ui,
    data: &mut MainColorPickerData,
    color_copy_format: &mut ColorStringCopy,
) -> (Response, Vec2) {
    let num_spline_points = data.paint_curve.spline.len();
    match data.last_modifying_bezier_index {
        Some(a) => {
            data.last_modifying_bezier_index = None;
        }
        _ => {}
    }

    let mut bezier_response_size = Vec2::default();
    let main_color_picker_response =
        ui.with_layout(Layout::top_down(egui::Align::Min), |mut ui| {
            let desired_size_slider_2d = Vec2::splat(ui.spacing().slider_width);

            let bezier_index = data
                .dragging_bezier_index
                .or(data.last_modifying_bezier_index);

            let color_data_bezier_index = match bezier_index {
                Some(a) => data.paint_curve.control_points().get(a).unwrap().value,
                _ => [0.0; 3],
            };

            let color_data_x = color_data_bezier_index[0];
            let color_data_y = color_data_bezier_index[1];
            let color_data_hue = color_data_bezier_index[2];
            let mut color_to_show: HsvaGamma = xyz_to_hsva(
                color_data_hue,
                color_data_x / desired_size_slider_2d.x,
                color_data_y / desired_size_slider_2d.y,
            )
            .into();

            let current_color_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
            let response =
                show_color(ui, color_to_show, current_color_size).on_hover_text("Selected color");
            response_copy_color_on_click(
                ui,
                &response,
                color_to_show,
                *color_copy_format,
                PointerButton::Middle,
            );

            color_text_ui(ui, color_to_show, data.alpha, *color_copy_format);

            if data.alpha == Alpha::BlendOrAdditive {
                // We signal additive blending by storing a negative alpha (a bit ironic).
                let a = &mut color_to_show.a;
                let mut additive = *a < 0.0;
                ui.horizontal(|ui| {
                    ui.label("Blending:");
                    ui.radio_value(&mut additive, false, "Normal");
                    ui.radio_value(&mut additive, true, "Additive");

                    if additive {
                        *a = -a.abs();
                    }

                    if !additive {
                        *a = a.abs();
                    }
                });
            }

            let additive = color_to_show.a < 0.0;

            let opaque = HsvaGamma {
                a: 1.0,
                ..color_to_show
            };

            if data.alpha == Alpha::Opaque {
                color_to_show.a = 1.0;
            } else {
                let a = &mut color_to_show.a;

                if data.alpha == Alpha::OnlyBlend {
                    if *a < 0.0 {
                        *a = 0.5; // was additive, but isn't allowed to be
                    }
                    color_slider_1d(ui, a, |a| HsvaGamma { a, ..opaque }.into())
                        .on_hover_text("Alpha");
                } else if !additive {
                    color_slider_1d(ui, a, |a| HsvaGamma { a, ..opaque }.into())
                        .on_hover_text("Alpha");
                }
            }

            let mut delta_hue = None;
            {
                let mut hue_mut = color_data_hue;
                let prev_hue = color_data_hue;
                let hue_response = color_slider_1d(ui, &mut hue_mut, |h| {
                    HsvaGamma {
                        h,
                        s: 1.0,
                        v: 1.0,
                        a: 1.0,
                    }
                    .into()
                })
                .on_hover_text("Hue");

                if data.is_curve_locked {
                    if bezier_index.is_some() {
                        delta_hue = if let Some(_) = hue_response.interact_pointer_pos() {
                            let hue_diff = hue_mut - prev_hue;
                            Some(hue_diff)
                        } else {
                            None
                        };

                        if delta_hue.is_some() {
                            let color_data_hue_mut = &mut data
                                .paint_curve
                                .control_points_mut()
                                .get_mut(bezier_index.unwrap())
                                .unwrap()
                                .value[2];

                            *color_data_hue_mut += delta_hue.unwrap();
                        }
                    }
                }
            }

            if let Some(h) = delta_hue {
                // Move all other points
                for i in 0..num_spline_points {
                    if (i == bezier_index.unwrap_or(0)) {
                        continue;
                    }
                    let hue_ref = &mut data.paint_curve.spline.get_mut(i).unwrap().value[2];
                    *hue_ref += h;
                }
            }

            let HsvaGamma { h, s, v, a: _ } = &mut color_to_show;

            if false {
                color_slider_1d(ui, s, |s| HsvaGamma { s, ..opaque }.into())
                    .on_hover_text("Saturation");
            }

            if false {
                color_slider_1d(ui, v, |v| HsvaGamma { v, ..opaque }.into()).on_hover_text("Value");
            }

            let slider_2d_reponse: Response = main_color_slider_2d(
                ui,
                desired_size_slider_2d,
                s,
                v,
                main_color_picker_color_at_function(*h, 1.0),
            );

            if bezier_index.is_some() {
                let color_data_x_mut = &mut data
                    .paint_curve
                    .control_points_mut()
                    .get_mut(bezier_index.unwrap())
                    .unwrap()
                    .value[0];
                *color_data_x_mut = *s / desired_size_slider_2d.x;

                let color_data_y_mut = &mut data
                    .paint_curve
                    .control_points_mut()
                    .get_mut(bezier_index.unwrap())
                    .unwrap()
                    .value[1];
                *color_data_y_mut = *v / desired_size_slider_2d.y;
            }

            let (bezier_response, dragged_points_response, selected_index, hovering_bezier_option) =
                data.paint_curve.ui_content(
                    &mut ui,
                    data.is_hue_middle_interpolated,
                    &slider_2d_reponse,
                );
            data.bezier_right_clicked = match hovering_bezier_option {
                Some(a) => {
                    if a.0.secondary_clicked() {
                        Some(a.1)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            data.dragging_bezier_index = selected_index;
            match selected_index {
                Some(a) => data.last_modifying_bezier_index = Some(a),
                _ => {}
            }

            bezier_response_size = bezier_response.rect.size();

            match dragged_points_response {
                Some(R) => {
                    if R.dragged() {
                        if data.is_curve_locked {
                            // Move all other points
                            for i in 0..num_spline_points {
                                if i == bezier_index.unwrap_or(0) {
                                    continue;
                                }

                                {
                                    let point_x_ref =
                                        &mut data.paint_curve.spline.get_mut(i).unwrap().value[0];

                                    *point_x_ref += R.drag_delta().x;
                                }
                                {
                                    let point_y_ref =
                                        &mut data.paint_curve.spline.get_mut(i).unwrap().value[1];
                                    *point_y_ref += R.drag_delta().y;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            ui.horizontal(|ui| {
                ui.checkbox(&mut data.is_curve_locked, "🔒");
                ui.checkbox(&mut data.is_hue_middle_interpolated, "🎨");

                egui::ComboBox::from_label("Color Copy Format")
                    .selected_text(format!("{color_copy_format:?}"))
                    .show_ui(ui, |ui| {
                        ui.style_mut().wrap = Some(false);
                        ui.set_min_width(60.0);
                        ui.selectable_value(color_copy_format, ColorStringCopy::HEX, "Hex");
                        ui.selectable_value(
                            color_copy_format,
                            ColorStringCopy::HEXNOA,
                            "Hex(no A)",
                        );
                    });
            });

            if data.is_hue_middle_interpolated {
                let num_points = data.paint_curve.spline.len();
                if (num_points >= 2) {
                    let points = data.paint_curve.control_points_mut();
                    for i in 0..num_points {
                        let point = points.get_mut(i).unwrap();
                        point.value[0] /= bezier_response_size.x;
                        point.value[1] /= bezier_response_size.y;
                    }

                    let first_index = 0;
                    let last_index = points.len() - 1;
                    let first_point = points.get(0).unwrap().value[2];
                    let last_point = points.get(last_index).unwrap().value[2];
                    let first_hue = points.get(first_index).unwrap().value[2];
                    let last_hue = points.get(last_index).unwrap().value[2];
                    for i in 1..(last_index) {
                        let t = i as f32 / points.len() as f32;
                        let hue = lerp((first_hue..=last_hue), t);
                        points.get_mut(i).unwrap().value[2] = hue;
                    }
                }
            }

            slider_2d_reponse
        });

    return (main_color_picker_response.inner, bezier_response_size);
}

fn main_color_picker_color_at_function(hue: f32, alpha: f32) -> impl Fn(f32, f32) -> Color32 {
    let color = HsvaGamma {
        h: hue,
        s: 0.0,
        v: 0.0,
        a: alpha,
    };

    return move |s, v| HsvaGamma { s, v, ..color }.into();
}

fn color_slider_1d(ui: &mut Ui, value: &mut f32, color_at: impl Fn(f32) -> Color32) -> Response {
    #![allow(clippy::identity_op)]

    let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);

        // background_checkers(ui.painter(), rect); // for alpha:

        {
            // fill color:
            let mut mesh = Mesh::default();
            for i in 0..=N {
                let t = i as f32 / (N as f32);
                let color = color_at(t);
                let x = lerp(rect.left()..=rect.right(), t);
                mesh.colored_vertex(pos2(x, rect.top()), color);
                mesh.colored_vertex(pos2(x, rect.bottom()), color);
                if i < N {
                    mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                    mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
                }
            }
            ui.painter().add(Shape::mesh(mesh));
        }

        ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

        {
            // Show where the slider is at:
            let x = lerp(rect.left()..=rect.right(), *value);
            let r = rect.height() / 4.0;
            let picked_color = color_at(*value);
            ui.painter().add(Shape::convex_polygon(
                vec![
                    pos2(x, rect.center().y),   // tip
                    pos2(x + r, rect.bottom()), // right bottom
                    pos2(x - r, rect.bottom()), // left bottom
                ],
                picked_color,
                Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
            ));
        }
    }

    response
}

/// Number of vertices per dimension in the color sliders.
/// We need at least 6 for hues, and more for smooth 2D areas.
/// Should always be a multiple of 6 to hit the peak hues in HSV/HSL (every 60°).
const N: u32 = 6 * 6;
/// # Arguments
/// * `x_value` - X axis, either saturation or value (0.0-1.0).
/// * `y_value` - Y axis, either saturation or value (0.0-1.0).
/// * `color_at` - A function that dictates how the mix of saturation and value will be displayed in the 2d slider.
/// E.g.: `|x_value, y_value| HsvaGamma { h: 1.0, s: x_value, v: y_value, a: 1.0 }.into()` displays the colors as follows: top-left: white \[s: 0.0, v: 1.0], top-right: fully saturated color \[s: 1.0, v: 1.0], bottom-right: black \[s: 0.0, v: 1.0].
///
fn main_color_slider_2d(
    ui: &mut Ui,
    desiered_size: Vec2,
    x_value: &mut f32,
    y_value: &mut f32,
    color_at: impl Fn(f32, f32) -> Color32,
) -> Response {
    let (rect, response) = ui.allocate_at_least(desiered_size, Sense::click());

    if let Some(mpos) = response.interact_pointer_pos() {
        *x_value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
        *y_value = remap_clamp(mpos.y, rect.bottom()..=rect.top(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let mut mesh = Mesh::default();

        for xi in 0..=N {
            for yi in 0..=N {
                let xt = xi as f32 / (N as f32);
                let yt: f32 = yi as f32 / (N as f32);
                let color = color_at(xt, yt);
                let x = lerp(rect.left()..=rect.right(), xt);
                let y = lerp(rect.bottom()..=rect.top(), yt);
                mesh.colored_vertex(pos2(x, y), color);

                if xi < N && yi < N {
                    let x_offset = 1;
                    let y_offset = N + 1;
                    let tl = yi * y_offset + xi;
                    mesh.add_triangle(tl, tl + x_offset, tl + y_offset);
                    mesh.add_triangle(tl + x_offset, tl + y_offset, tl + y_offset + x_offset);
                }
            }
        }
        ui.painter().add(Shape::mesh(mesh)); // fill

        ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

        // Show where the slider is at:
        let x = lerp(rect.left()..=rect.right(), *x_value);
        let y = lerp(rect.bottom()..=rect.top(), *y_value);
        let picked_color = color_at(*x_value, *y_value);
        ui.painter().add(epaint::CircleShape {
            center: pos2(x, y),
            radius: rect.width() / 12.0,
            fill: picked_color,
            stroke: Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        });
    }

    response
}

pub fn color_button_copy(
    ui: &mut Ui,
    color: impl Into<Color32>,
    alpha: Alpha,
    color_copy_format: ColorStringCopy,
) {
    let button_response = ui.button("📋").on_hover_text("Copy (middle mouse click)");
    if button_response.clicked() {
        ui.output_mut(|o| {
            o.copied_text = format_color_as(color.into(), color_copy_format, None);
        });
    }
}

pub fn response_copy_color_on_click(
    ui: &mut Ui,
    response: &Response,
    color: impl Into<Color32>,
    color_copy_format: ColorStringCopy,
    button_click_type: PointerButton,
) {
    if response.clicked_by(button_click_type) {
        ui.output_mut(|o| {
            o.copied_text = format_color_as(color.into(), color_copy_format, None);
        });
    }
}

fn color_text_ui(
    ui: &mut Ui,
    color: impl Into<Color32>,
    alpha: Alpha,
    color_copy_format: ColorStringCopy,
) {
    let color = color.into();
    let [r, g, b, a] = color.to_array();

    ui.horizontal(|ui| {
        color_button_copy(ui, color, alpha, color_copy_format);

        let old_style = Arc::as_ref(ui.style()).clone();

        ui.style_mut()
            .text_styles
            .get_mut(&TextStyle::Body)
            .unwrap()
            .size = 8.0;

        if alpha == Alpha::Opaque {
            ui.label(format!("rgb({}, {}, {})", r, g, b))
                .on_hover_text("Red Green Blue");
        } else {
            ui.label(format!("rgba({}, {}, {}, {})", r, g, b, a))
                .on_hover_text("Red Green Blue with premultiplied Alpha");
        }

        *ui.style_mut() = old_style;
    });
}
