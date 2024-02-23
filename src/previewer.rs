use ecolor::HsvaGamma;
use eframe::{
    egui::{
        self, Layout, Painter, PointerButton, Response, Sense, Ui, Vec2, WidgetInfo, WidgetType,
    },
    epaint::{Color32, Rect},
};

use crate::{
    color_picker::{response_copy_color_on_click, ColorStringCopy, PreviewerData, SplineMode},
    curves::control_points_to_spline,
    gradient::color_function_gradient,
    ui_common::{background_checkers, color_button},
    CONTROL_POINT_TYPE,
};

fn draw_ui_previewer_control_points(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[CONTROL_POINT_TYPE],
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
) {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_control_points =
        ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));

    previewer_ui_control_points.spacing_mut().item_spacing = Vec2::ZERO;

    let ui_size: Vec2 = previewer_ui_control_points.available_size();

    let spline = control_points_to_spline(control_points, spline_mode);
    let num_spline_points = spline.len();
    let size_per_color_x = ui_size.x / (num_spline_points as f32);
    let size_per_color_y = ui_size.y;
    let previewer_sizes_sum: f32 = previewer_data.points_preview_sizes.iter().sum();

    let mut points: Vec<Vec2> = Vec::with_capacity(num_spline_points);
    for cp in control_points {
        points.push(Vec2::new(cp[0], cp[1]));
    }

    for i in 0..num_spline_points {
        if points.len() <= i {
            break;
        }
        let color_data = &points[i];
        let color_data_hue = spline.get(i).unwrap().value[2];
        let mut color_at_point: HsvaGamma = HsvaGamma {
            h: color_data_hue,
            s: color_data.x,
            v: color_data.y,
            a: 1.0,
        };

        let size_weight: f32 =
            previewer_data.points_preview_sizes[i] * num_spline_points as f32 / previewer_sizes_sum;
        let response: Response = color_button(
            &mut previewer_ui_control_points,
            Vec2 {
                x: size_weight * size_per_color_x,
                y: size_per_color_y,
            },
            color_at_point.into(),
            true,
        );

        response_copy_color_on_click(
            ui,
            &response,
            color_at_point,
            color_copy_format,
            PointerButton::Middle,
        );

        if response.dragged_by(PointerButton::Primary) {
            const PREVIEWER_DRAG_SENSITIVITY: f32 = 0.6;
            previewer_data.points_preview_sizes[i] +=
                response.drag_delta().x * PREVIEWER_DRAG_SENSITIVITY;
            previewer_data.points_preview_sizes[i] =
                previewer_data.points_preview_sizes[i].max(0.0);

            let min_percentage_x = 0.5 * (1.0 / num_spline_points as f32);
            let min_preview_size: f32 = min_percentage_x * previewer_sizes_sum;

            // TODO: loop over all and set min_preview_size
            previewer_data.enforce_min_size(min_preview_size);
        }

        let color_response_rect = response.ctx.screen_rect();
    }
}

fn draw_ui_previewer_curve(
    ui: &mut Ui,
    size: Vec2,
    control_points: &[CONTROL_POINT_TYPE],
    spline_mode: SplineMode,
    previewer_data: &PreviewerData,
    color_copy_format: ColorStringCopy,
) {
    let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
    ui.allocate_rect(rect, Sense::click_and_drag());
    let mut previewer_ui_curve = ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));
    previewer_ui_curve.spacing_mut().item_spacing = Vec2::ZERO;

    let spline = control_points_to_spline(control_points, spline_mode);
    let colors: Vec<Color32> = spline
        .keys()
        .iter()
        .map(|a| {
            HsvaGamma {
                h: a.value[2],
                s: a.value[0],
                v: a.value[1],
                a: 1.0,
            }
            .into()
        })
        .collect();

    color_function_gradient(&mut previewer_ui_curve, rect.size(), |x| {
        if spline.len() <= 0 {
            return HsvaGamma {
                h: 0.0,
                s: 0.0,
                v: 0.0,
                a: 0.0,
            }
            .into();
        }

        let sample = spline
            .clamped_sample(x * spline.get(spline.len() - 1).unwrap().t as f32)
            .unwrap_or_default();

        HsvaGamma {
            h: sample[2],
            s: sample[0],
            v: sample[1],
            a: 1.0,
        }
        .into()
    });
}

pub fn draw_ui_previewer(
    ui: &mut Ui,
    control_points: &[CONTROL_POINT_TYPE],
    spline_mode: SplineMode,
    previewer_data: &mut PreviewerData,
    color_copy_format: ColorStringCopy,
) {
    let previewer_rect = ui.available_rect_before_wrap();

    ui.vertical(|ui| {
        draw_ui_previewer_control_points(
            ui,
            previewer_rect.size() * Vec2::new(1.0, 0.5),
            control_points,
            spline_mode,
            previewer_data,
            color_copy_format,
        );
        draw_ui_previewer_curve(
            ui,
            previewer_rect.size() * Vec2::new(1.0, 0.5),
            control_points,
            spline_mode,
            previewer_data,
            color_copy_format,
        );

        let reset_button = egui::Button::new("❌").small().wrap(true).frame(true);
        let reset_button_size: Vec2 = Vec2::new(25.0, 25.0);
        let mut reset_button_rect: Rect = Rect {
            min: previewer_rect.min,
            max: previewer_rect.min + reset_button_size,
        };

        if ui.put(reset_button_rect, reset_button).clicked() {
            previewer_data.reset_preview_sizes();
        }
    });
}
