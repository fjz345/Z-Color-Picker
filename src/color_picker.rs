use std::{
    borrow::{Borrow, BorrowMut},
    default,
    fmt::format,
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
use splines::{Interpolation, Spline};

use crate::{
    curves::{self, Bezier, PaintCurve},
    hsv_key_value::HsvKeyValue,
    ui_common::{
        color_slider_1d, color_slider_2d, color_text_ui, contrast_color,
        response_copy_color_on_click,
    },
    CONTROL_POINT_TYPE,
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

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SplineMode {
    Linear,
    Bezier,
    HermiteBezier,
    Polynomial,
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
    pub paint_curve: PaintCurve,
    pub dragging_bezier_index: Option<usize>,
    pub control_point_right_clicked: Option<usize>,
    pub last_modifying_point_index: Option<usize>,
    pub is_curve_locked: bool,
    pub is_hue_middle_interpolated: bool,
    pub is_insert_right: bool,
    pub is_window_lock: bool,
}

pub fn main_color_picker(
    ui: &mut Ui,
    control_points: &mut [CONTROL_POINT_TYPE],
    spline_mode: SplineMode,
    data: &mut MainColorPickerData,
    color_copy_format: &mut ColorStringCopy,
) -> Response {
    let num_spline_points = control_points.len();
    if let Some(last_modified_index) = data.last_modifying_point_index {
        if num_spline_points == 0 {
            data.last_modifying_point_index = None;
        } else {
            data.last_modifying_point_index =
                Some(last_modified_index.clamp(0, num_spline_points - 1));
        }
    }

    let main_color_picker_response = ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let desired_size_slider_2d = Vec2::splat(ui.spacing().slider_width);

        let is_modifying_index = data
            .dragging_bezier_index
            .or(data.last_modifying_point_index);

        let modifying_control_point = match is_modifying_index {
            Some(index) => control_points.get_mut(index),
            None => None,
        };

        let mut dummy_color = HsvaGamma {
            h: 0.0,
            s: 0.0,
            v: 0.0,
            a: 1.0,
        };
        let mut color_to_show = match modifying_control_point.as_ref() {
            Some(CP) => HsvaGamma {
                h: CP[2],
                s: CP[0],
                v: CP[1],
                a: 1.0,
            },
            None => dummy_color,
        };

        let mut delta_hue = None;
        {
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
                    color_slider_1d(ui, Some(a), |a| HsvaGamma { a, ..opaque }.into())
                        .on_hover_text("Alpha");
                } else if !additive {
                    color_slider_1d(ui, Some(a), |a| HsvaGamma { a, ..opaque }.into())
                        .on_hover_text("Alpha");
                }
            }

            let prev_hue = color_to_show.h;
            let hue_optional_value: Option<&mut f32> = match modifying_control_point {
                Some(CP) => Some(&mut CP[2]),
                None => None,
            };
            let hue_response = color_slider_1d(ui, hue_optional_value, |h| {
                HsvaGamma {
                    h,
                    s: 1.0,
                    v: 1.0,
                    a: 1.0,
                }
                .into()
            })
            .on_hover_text("Hue");
            delta_hue = match hue_response.changed() {
                true => Some(color_to_show.h - prev_hue),
                false => None,
            };
        }

        if let Some(h) = delta_hue {
            // Move all other points
            for i in 0..num_spline_points {
                if is_modifying_index.is_some() {
                    if i == is_modifying_index.unwrap() {
                        continue;
                    }
                }
                let hue_ref = &mut control_points[i][2];
                *hue_ref += h;
            }
        }

        let slider_2d_reponse: Response = color_slider_2d(
            ui,
            desired_size_slider_2d,
            &mut color_to_show.s,
            &mut color_to_show.v,
            main_color_picker_color_at_function(color_to_show.h, 1.0),
        );

        if is_modifying_index.is_some() {
            let mut control_point = match is_modifying_index {
                Some(a) => Some(control_points[a]),
                _ => None,
            };
            let unwrapped = &mut control_point.unwrap();
            unwrapped[2] = color_to_show.h;
        }

        if data.dragging_bezier_index.is_some() {
            let mut control_point = match is_modifying_index {
                Some(a) => Some(control_points[a]),
                _ => None,
            };
            let unwrapped = &mut control_point.unwrap();
            unwrapped[0] = color_to_show.s;
            unwrapped[1] = color_to_show.v;
        }

        let (dragged_points_response, selected_index, hovering_control_point) =
            data.paint_curve.ui_content(
                ui,
                control_points,
                spline_mode,
                data.is_hue_middle_interpolated,
                &slider_2d_reponse,
            );
        data.control_point_right_clicked = match hovering_control_point {
            Some(a) => {
                if a.0.clicked_by(PointerButton::Secondary) {
                    Some(a.1)
                } else {
                    None
                }
            }
            _ => None,
        };

        data.dragging_bezier_index = selected_index;
        match selected_index {
            Some(index) => data.last_modifying_point_index = Some(index),
            _ => {}
        }

        match dragged_points_response {
            Some(R) => {
                if R.dragged_by(PointerButton::Primary) {
                    match is_modifying_index {
                        Some(index) => {
                            {
                                let point_x_ref = &mut control_points[index][0];

                                *point_x_ref += R.drag_delta().x / slider_2d_reponse.rect.size().x;
                            }
                            {
                                let point_y_ref = &mut control_points[index][1];
                                *point_y_ref -= R.drag_delta().y / slider_2d_reponse.rect.size().y;
                            }
                        }
                        _ => {}
                    }

                    if data.is_curve_locked {
                        // Move all other points
                        for i in 0..num_spline_points {
                            if i == is_modifying_index.unwrap_or(0) {
                                continue;
                            }

                            {
                                let point_x_ref = &mut control_points[i][0];

                                *point_x_ref += R.drag_delta().x / slider_2d_reponse.rect.size().x;
                            }
                            {
                                let point_y_ref = &mut control_points[i][1];
                                *point_y_ref -= R.drag_delta().y / slider_2d_reponse.rect.size().y;
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        slider_2d_reponse
    });

    return main_color_picker_response.inner;
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
