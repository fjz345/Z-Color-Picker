use std::f32::consts::TAU;

use eframe::{
    egui::{
        self, Align2, FontId, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2,
    },
    epaint::{CircleShape, Color32, Rgba},
};
use serde::{Deserialize, Serialize};

use crate::{
    color_space::{
        linear_to_oklab, linear_to_srgb, srgb_to_hsl, srgb_to_hsv, srgb_to_linear, PickerSpace,
    },
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::ControlPoint,
    ui_egui::{
        color_picker::format_color_as,
        control_points::{
            control_point_outline, control_points_to_spline, flatten_control_points,
            sample_spline_normalized,
        },
    },
};

const CURVE_SAMPLES: usize = 128;

fn gradient_linear_samples(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    picker_space: PickerSpace,
) -> Vec<[f32; 3]> {
    let spline = control_points_to_spline(&flatten_control_points(control_points), spline_mode);
    (0..CURVE_SAMPLES)
        .filter_map(|i| {
            let x = i as f32 / (CURVE_SAMPLES - 1) as f32;
            sample_spline_normalized(&spline, spline_mode, x)
        })
        .map(|sample| linear_of(sample.color(picker_space)))
        .collect()
}

fn linear_of(color: Color32) -> [f32; 3] {
    let rgba = Rgba::from(color);
    [rgba.r(), rgba.g(), rgba.b()]
}

fn display_color([r, g, b]: [f32; 3]) -> Color32 {
    Rgba::from_rgb(r, g, b).into()
}

fn midpoint(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| 0.5 * (a[i] + b[i]))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum View3dSpace {
    #[default]
    Srgb,
    LinearRgb,
    Hsv,
    Hsl,
    Oklab,
}

const CYLINDER_RADIUS: f32 = 0.5;

impl View3dSpace {
    const ALL: [View3dSpace; 5] = [
        View3dSpace::Srgb,
        View3dSpace::LinearRgb,
        View3dSpace::Hsv,
        View3dSpace::Hsl,
        View3dSpace::Oklab,
    ];

    fn label(self) -> &'static str {
        match self {
            View3dSpace::Srgb => "sRGB cube",
            View3dSpace::LinearRgb => "Linear RGB cube",
            View3dSpace::Hsv => "HSV cylinder",
            View3dSpace::Hsl => "HSL cylinder",
            View3dSpace::Oklab => "Oklab",
        }
    }

    fn position(self, linear: [f32; 3]) -> [f32; 3] {
        match self {
            View3dSpace::Srgb => centered_cube(linear_to_srgb(linear)),
            View3dSpace::LinearRgb => centered_cube(linear),
            View3dSpace::Hsv => {
                let [h, s, v] = srgb_to_hsv(linear_to_srgb(linear));
                cylinder(h, s, v)
            }
            View3dSpace::Hsl => {
                let [h, s, l] = srgb_to_hsl(linear_to_srgb(linear));
                cylinder(h, s, l)
            }
            View3dSpace::Oklab => {
                let [l, a, b] = linear_to_oklab(linear);
                [a, l - 0.5, -b]
            }
        }
    }
}

fn centered_cube(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| c - 0.5)
}

fn cylinder(hue_turns: f32, radius: f32, height: f32) -> [f32; 3] {
    let (sin, cos) = (hue_turns * TAU).sin_cos();
    let r = radius * CYLINDER_RADIUS;
    [r * cos, height - 0.5, -r * sin]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.45,
            distance: 2.6,
        }
    }
}

const MAX_PITCH: f32 = 1.48;
const MIN_DISTANCE: f32 = 1.2;
const MAX_DISTANCE: f32 = 8.0;
const ORBIT_RADIANS_PER_POINT: f32 = 0.01;
const ZOOM_PER_SCROLL_POINT: f32 = 0.002;
const FOCAL_LENGTH_PER_SIZE: f32 = 1.2;
const NEAR_DEPTH: f32 = 0.05;
// Half the unit cube's diagonal: every reference shape fits inside.
const SCENE_RADIUS: f32 = 0.866_025_4;
const FAR_FADE: f32 = 0.45;

#[derive(Clone, Copy, Debug)]
struct Projected {
    pos: Pos2,
    depth: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl OrbitCamera {
    fn orbited(self, drag: Vec2) -> Self {
        Self {
            yaw: self.yaw - drag.x * ORBIT_RADIANS_PER_POINT,
            pitch: (self.pitch + drag.y * ORBIT_RADIANS_PER_POINT).clamp(-MAX_PITCH, MAX_PITCH),
            ..self
        }
    }

    fn zoomed(self, scroll: f32) -> Self {
        let distance = self.distance * (-scroll * ZOOM_PER_SCROLL_POINT).exp();
        Self {
            distance: distance.clamp(MIN_DISTANCE, MAX_DISTANCE),
            ..self
        }
    }

    fn project(self, p: [f32; 3], rect: Rect) -> Option<Projected> {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let right = [cos_yaw, 0.0, -sin_yaw];
        let up = [-sin_pitch * sin_yaw, cos_pitch, -sin_pitch * cos_yaw];
        let toward_camera = [cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw];
        let depth = self.distance - dot(p, toward_camera);
        if depth < NEAR_DEPTH {
            return None;
        }
        let scale = FOCAL_LENGTH_PER_SIZE * rect.size().min_elem() / depth;
        let offset = Vec2::new(dot(p, right), -dot(p, up));
        Some(Projected {
            pos: rect.center() + scale * offset,
            depth,
        })
    }

    fn size_scale(self, depth: f32) -> f32 {
        self.distance / depth
    }

    fn depth_fade(self, depth: f32) -> f32 {
        let nearest = self.distance - SCENE_RADIUS;
        let far_fraction = ((depth - nearest) / (2.0 * SCENE_RADIUS)).clamp(0.0, 1.0);
        1.0 - FAR_FADE * far_fraction
    }
}

struct WorldSegment {
    ends: [[f32; 3]; 2],
    color: Color32,
    width: f32,
}

const CURVE_WIDTH: f32 = 4.0;
const REFERENCE_WIDTH: f32 = 1.5;
const REFERENCE_OPACITY: f32 = 0.5;
const GUIDE_WIDTH: f32 = 1.0;
const EDGE_STEPS: usize = 16;
const RING_STEPS: usize = 48;
const SPOKES: usize = 6;
const OKLAB_FLOOR_AXIS: f32 = 0.35;

fn colored_polyline(
    space: View3dSpace,
    linear: &[[f32; 3]],
    width: f32,
    opacity: f32,
) -> Vec<WorldSegment> {
    linear
        .windows(2)
        .map(|pair| WorldSegment {
            ends: [space.position(pair[0]), space.position(pair[1])],
            color: display_color(midpoint(pair[0], pair[1])).gamma_multiply(opacity),
            width,
        })
        .collect()
}

fn guide_polyline(points: &[[f32; 3]], color: Color32) -> Vec<WorldSegment> {
    points
        .windows(2)
        .map(|pair| WorldSegment {
            ends: [pair[0], pair[1]],
            color,
            width: GUIDE_WIDTH,
        })
        .collect()
}

fn line_points(start: [f32; 3], end: [f32; 3]) -> Vec<[f32; 3]> {
    (0..=EDGE_STEPS)
        .map(|step| {
            let t = step as f32 / EDGE_STEPS as f32;
            std::array::from_fn(|i| start[i] + t * (end[i] - start[i]))
        })
        .collect()
}

fn srgb_cube_edges() -> impl Iterator<Item = Vec<[f32; 3]>> {
    (0..3).flat_map(|axis| {
        (0..4).map(move |corner| {
            let mut start = [0.0; 3];
            start[(axis + 1) % 3] = (corner & 1) as f32;
            start[(axis + 2) % 3] = (corner >> 1) as f32;
            let mut end = start;
            end[axis] = 1.0;
            line_points(start, end)
                .into_iter()
                .map(srgb_to_linear)
                .collect()
        })
    })
}

fn gamut_outline(space: View3dSpace) -> Vec<WorldSegment> {
    srgb_cube_edges()
        .flat_map(|edge| colored_polyline(space, &edge, REFERENCE_WIDTH, REFERENCE_OPACITY))
        .collect()
}

fn cylinder_guides(guide: Color32) -> Vec<WorldSegment> {
    let mut lines = Vec::new();
    for height in [0.0, 1.0] {
        lines.push(
            (0..=RING_STEPS)
                .map(|step| cylinder(step as f32 / RING_STEPS as f32, 1.0, height))
                .collect(),
        );
        lines.extend((0..SPOKES).map(|spoke| {
            let hue = spoke as f32 / SPOKES as f32;
            line_points(cylinder(hue, 0.0, height), cylinder(hue, 1.0, height))
        }));
    }
    lines.push(line_points(
        cylinder(0.0, 0.0, 0.0),
        cylinder(0.0, 0.0, 1.0),
    ));
    lines
        .iter()
        .flat_map(|line| guide_polyline(line, guide))
        .collect()
}

fn oklab_axes(guide: Color32) -> Vec<WorldSegment> {
    [
        ([0.0, -0.5, 0.0], [0.0, 0.5, 0.0]),
        (
            [-OKLAB_FLOOR_AXIS, -0.5, 0.0],
            [OKLAB_FLOOR_AXIS, -0.5, 0.0],
        ),
        (
            [0.0, -0.5, -OKLAB_FLOOR_AXIS],
            [0.0, -0.5, OKLAB_FLOOR_AXIS],
        ),
    ]
    .iter()
    .flat_map(|&(start, end)| guide_polyline(&line_points(start, end), guide))
    .collect()
}

fn reference_segments(space: View3dSpace, guide: Color32) -> Vec<WorldSegment> {
    match space {
        View3dSpace::Srgb | View3dSpace::LinearRgb => gamut_outline(space),
        View3dSpace::Hsv | View3dSpace::Hsl => cylinder_guides(guide),
        View3dSpace::Oklab => {
            let mut segments = gamut_outline(space);
            segments.extend(oklab_axes(guide));
            segments
        }
    }
}

const LABEL_EXTENT: f32 = 0.62;
const OKLAB_LABEL_EXTENT: f32 = 0.42;
const LABEL_SIZE: f32 = 12.0;

fn axis_labels(space: View3dSpace) -> &'static [(&'static str, [f32; 3])] {
    match space {
        View3dSpace::Srgb | View3dSpace::LinearRgb => &[
            ("R", [LABEL_EXTENT, -0.5, -0.5]),
            ("G", [-0.5, LABEL_EXTENT, -0.5]),
            ("B", [-0.5, -0.5, LABEL_EXTENT]),
        ],
        View3dSpace::Hsv => &[
            ("V", [0.0, LABEL_EXTENT, 0.0]),
            ("S", [LABEL_EXTENT, 0.5, 0.0]),
        ],
        View3dSpace::Hsl => &[
            ("L", [0.0, LABEL_EXTENT, 0.0]),
            ("S", [LABEL_EXTENT, 0.5, 0.0]),
        ],
        View3dSpace::Oklab => &[
            ("L", [0.0, LABEL_EXTENT, 0.0]),
            ("+a", [OKLAB_LABEL_EXTENT, -0.5, 0.0]),
            ("+b", [0.0, -0.5, -OKLAB_LABEL_EXTENT]),
        ],
    }
}

const POINT_RADIUS: f32 = 6.0;
const SELECTED_POINT_RADIUS: f32 = 8.0;
// Points share their samples' depth; the bias draws them over the curve.
const POINT_DEPTH_BIAS: f32 = 0.05;
const HOVER_RADIUS: f32 = 10.0;
const VIEW_3D_HINT: &str = "Drag: orbit. Scroll: zoom. Double-click: reset";

fn segment_shape(camera: OrbitCamera, rect: Rect, segment: &WorldSegment) -> Option<(f32, Shape)> {
    let [Some(a), Some(b)] = segment.ends.map(|p| camera.project(p, rect)) else {
        return None;
    };
    let depth = 0.5 * (a.depth + b.depth);
    let stroke = Stroke::new(
        segment.width * camera.size_scale(depth),
        segment.color.gamma_multiply(camera.depth_fade(depth)),
    );
    Some((depth, Shape::line_segment([a.pos, b.pos], stroke)))
}

fn point_shape(
    camera: OrbitCamera,
    projected: Projected,
    fill: Color32,
    selected: bool,
) -> (f32, Shape) {
    let radius = if selected {
        SELECTED_POINT_RADIUS
    } else {
        POINT_RADIUS
    };
    let fade = camera.depth_fade(projected.depth);
    let outline = control_point_outline(fill, selected);
    let circle = CircleShape {
        center: projected.pos,
        radius: radius * camera.size_scale(projected.depth),
        fill: fill.gamma_multiply(fade),
        stroke: Stroke::new(outline.width, outline.color.gamma_multiply(fade)),
    };
    (projected.depth - POINT_DEPTH_BIAS, Shape::Circle(circle))
}

fn paint_axis_labels(
    painter: &Painter,
    camera: OrbitCamera,
    rect: Rect,
    space: View3dSpace,
    color: Color32,
) {
    for &(text, position) in axis_labels(space) {
        if let Some(projected) = camera.project(position, rect) {
            painter.text(
                projected.pos,
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(LABEL_SIZE),
                color,
            );
        }
    }
}

fn nearest_color(pointer: Pos2, targets: &[(Pos2, Color32)]) -> Option<Color32> {
    targets
        .iter()
        .map(|&(pos, color)| (pos.distance_sq(pointer), color))
        .filter(|&(distance_sq, _)| distance_sq <= HOVER_RADIUS * HOVER_RADIUS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, color)| color)
}

fn camera_after_input(camera: OrbitCamera, ui: &Ui, response: &egui::Response) -> OrbitCamera {
    if response.double_clicked() {
        return OrbitCamera::default();
    }
    let mut camera = camera;
    if response.dragged_by(PointerButton::Primary) {
        camera = camera.orbited(response.drag_delta());
    }
    if response.hovered() {
        camera = camera.zoomed(ui.input(|i| i.smooth_scroll_delta.y));
    }
    camera
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct View3d {
    #[serde(default)]
    space: View3dSpace,
    #[serde(skip)]
    camera: OrbitCamera,
}

impl View3d {
    pub fn draw_ui(
        &mut self,
        ui: &mut Ui,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
        picker_space: PickerSpace,
        selected_index: Option<usize>,
    ) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("view_3d_space")
                .selected_text(self.space.label())
                .show_ui(ui, |ui| {
                    for space in View3dSpace::ALL {
                        ui.selectable_value(&mut self.space, space, space.label());
                    }
                });
            ui.weak(VIEW_3D_HINT);
        });
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.camera = camera_after_input(self.camera, ui, &response);
        let (camera, space) = (self.camera, self.space);
        let guide = ui.visuals().weak_text_color();

        let samples = gradient_linear_samples(control_points, spline_mode, picker_space);
        let mut segments = reference_segments(space, guide.gamma_multiply(REFERENCE_OPACITY));
        segments.extend(colored_polyline(space, &samples, CURVE_WIDTH, 1.0));
        let mut shapes: Vec<(f32, Shape)> = segments
            .iter()
            .filter_map(|segment| segment_shape(camera, rect, segment))
            .collect();

        let mut point_targets = Vec::new();
        for (index, cp) in control_points.iter().enumerate() {
            let fill = cp.val().color(picker_space);
            if let Some(projected) = camera.project(space.position(linear_of(fill)), rect) {
                shapes.push(point_shape(
                    camera,
                    projected,
                    fill,
                    selected_index == Some(index),
                ));
                point_targets.push((projected.pos, fill));
            }
        }
        let sample_targets: Vec<(Pos2, Color32)> = samples
            .iter()
            .filter_map(|&linear| {
                let projected = camera.project(space.position(linear), rect)?;
                Some((projected.pos, display_color(linear)))
            })
            .collect();

        shapes.sort_by(|a, b| b.0.total_cmp(&a.0));
        let painter = ui.painter_at(rect);
        painter.extend(shapes.into_iter().map(|(_, shape)| shape));
        paint_axis_labels(&painter, camera, rect, space, guide);

        let hovered = response.hover_pos().and_then(|pointer| {
            nearest_color(pointer, &point_targets)
                .or_else(|| nearest_color(pointer, &sample_targets))
        });
        if let Some(color) = hovered {
            let hex = format_color_as(color, ColorStringCopy::HEXNOA, None);
            response.on_hover_text_at_pointer(format!("#{hex}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, f32::consts::FRAC_PI_2, rc::Rc};

    use crate::{
        datatypes::control_point::ControlPointValue,
        ui_egui::{
            app::ZColorPickerAppContext,
            panes::{View3dPane, ZAppPane},
        },
    };

    const RECT: Rect = Rect {
        min: Pos2::ZERO,
        max: Pos2::new(200.0, 100.0),
    };

    fn assert_close(actual: [f32; 3], expected: [f32; 3]) {
        for i in 0..3 {
            assert!(
                (actual[i] - expected[i]).abs() < 1e-4,
                "{actual:?} != {expected:?}"
            );
        }
    }

    fn assert_pos_close(actual: Pos2, expected: Pos2) {
        assert!(
            actual.distance(expected) < 1e-3,
            "{actual:?} != {expected:?}"
        );
    }

    fn front_camera(distance: f32) -> OrbitCamera {
        OrbitCamera {
            yaw: 0.0,
            pitch: 0.0,
            distance,
        }
    }

    #[test]
    fn scene_center_projects_to_the_rect_center_at_the_camera_distance() {
        let camera = OrbitCamera::default();
        let projected = camera.project([0.0; 3], RECT).unwrap();
        assert_pos_close(projected.pos, RECT.center());
        assert!((projected.depth - camera.distance).abs() < 1e-6);
    }

    #[test]
    fn front_camera_offsets_points_by_focal_length_over_depth() {
        let camera = front_camera(2.0);
        let offset = 0.5 * FOCAL_LENGTH_PER_SIZE * RECT.height() / 2.0;
        let right = camera.project([0.5, 0.0, 0.0], RECT).unwrap();
        assert_pos_close(right.pos, RECT.center() + Vec2::new(offset, 0.0));
        let up = camera.project([0.0, 0.5, 0.0], RECT).unwrap();
        assert_pos_close(up.pos, RECT.center() - Vec2::new(0.0, offset));
    }

    #[test]
    fn side_camera_sees_the_point_toward_it_at_the_center_and_nearer() {
        let camera = OrbitCamera {
            yaw: FRAC_PI_2,
            ..front_camera(2.0)
        };
        let projected = camera.project([0.5, 0.0, 0.0], RECT).unwrap();
        assert_pos_close(projected.pos, RECT.center());
        assert!((projected.depth - 1.5).abs() < 1e-5, "{projected:?}");
    }

    #[test]
    fn points_behind_the_camera_are_not_projected() {
        assert!(front_camera(2.0).project([0.0, 0.0, 3.0], RECT).is_none());
    }

    #[test]
    fn orbit_clamps_pitch() {
        let camera = OrbitCamera::default();
        assert_eq!(camera.orbited(Vec2::new(0.0, 1e6)).pitch, MAX_PITCH);
        assert_eq!(camera.orbited(Vec2::new(0.0, -1e6)).pitch, -MAX_PITCH);
    }

    #[test]
    fn zoom_clamps_distance() {
        let camera = OrbitCamera::default();
        assert!(camera.zoomed(50.0).distance < camera.distance);
        assert_eq!(camera.zoomed(1e6).distance, MIN_DISTANCE);
        assert_eq!(camera.zoomed(-1e6).distance, MAX_DISTANCE);
    }

    #[test]
    fn white_and_black_sit_on_the_hsv_and_hsl_axis_ends() {
        for space in [View3dSpace::Hsv, View3dSpace::Hsl] {
            assert_close(space.position([1.0; 3]), [0.0, 0.5, 0.0]);
            assert_close(space.position([0.0; 3]), [0.0, -0.5, 0.0]);
        }
        assert_close(View3dSpace::Hsv.position([1.0, 0.0, 0.0]), [0.5, 0.5, 0.0]);
    }

    #[test]
    fn srgb_red_sits_at_its_cube_corner() {
        for space in [View3dSpace::Srgb, View3dSpace::LinearRgb] {
            assert_close(space.position([1.0, 0.0, 0.0]), [0.5, -0.5, -0.5]);
        }
    }

    #[test]
    fn oklab_white_sits_at_the_top_of_the_l_axis() {
        assert_close(View3dSpace::Oklab.position([1.0; 3]), [0.0, 0.5, 0.0]);
    }

    fn pane_frame(
        egui_ctx: &egui::Context,
        pane: &mut View3dPane,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            events,
            ..Default::default()
        };
        egui_ctx.run(input, |egui_ctx| {
            egui::CentralPanel::default().show(egui_ctx, |ui| pane.ui(ui));
        })
    }

    fn painted_texts(output: &egui::FullOutput) -> Vec<String> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    fn circle_centers(output: &egui::FullOutput) -> Vec<Pos2> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Circle(circle) => Some(circle.center),
                _ => None,
            })
            .collect()
    }

    fn primary(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    #[test]
    fn pane_shows_hex_on_hover_and_moves_only_its_camera() {
        let points: Vec<ControlPoint> = [
            (0.2, 0.9, 0.0, 0.0),
            (0.8, 0.6, 0.4, 0.5),
            (0.5, 0.3, 0.7, 1.0),
        ]
        .map(|(s, v, h, t)| ControlPoint::new_simple(ControlPointValue::new(s, v, h), t))
        .to_vec();
        let ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        ctx.borrow_mut().control_points = points.clone();
        ctx.borrow_mut().spline_mode = SplineMode::Bezier;
        ctx.borrow()
            .z_color_picker
            .borrow_mut()
            .last_modifying_point_index = Some(1);
        let mut pane = View3dPane {
            title: None,
            ctx: ctx.clone(),
            view: View3d::default(),
        };
        let egui_ctx = egui::Context::default();
        egui_ctx.style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });

        let output = pane_frame(&egui_ctx, &mut pane, Vec::new());
        let point = circle_centers(&output)[0];
        pane_frame(&egui_ctx, &mut pane, vec![egui::Event::PointerMoved(point)]);
        let hovered = painted_texts(&pane_frame(&egui_ctx, &mut pane, Vec::new()));
        let hexes: Vec<String> = points
            .iter()
            .map(|cp| {
                format!(
                    "#{}",
                    format_color_as(
                        cp.val().color(PickerSpace::Hsv),
                        ColorStringCopy::HEXNOA,
                        None
                    )
                )
            })
            .collect();
        assert!(
            hovered.iter().any(|text| hexes.contains(text)),
            "{hovered:?}"
        );

        let end = point + Vec2::new(60.0, 30.0);
        pane_frame(&egui_ctx, &mut pane, vec![primary(point, true)]);
        for step in 1..=3 {
            let pos = point.lerp(end, step as f32 / 3.0);
            pane_frame(&egui_ctx, &mut pane, vec![egui::Event::PointerMoved(pos)]);
        }
        pane_frame(&egui_ctx, &mut pane, vec![primary(end, false)]);
        let orbited = pane.view.camera;
        let start = OrbitCamera::default();
        assert!(
            orbited.yaw < start.yaw && orbited.pitch > start.pitch,
            "{orbited:?}"
        );

        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 50.0),
            modifiers: egui::Modifiers::default(),
        };
        pane_frame(&egui_ctx, &mut pane, vec![wheel]);
        for _ in 0..10 {
            pane_frame(&egui_ctx, &mut pane, Vec::new());
        }
        assert!(
            pane.view.camera.distance < orbited.distance,
            "{:?}",
            pane.view.camera
        );

        for pressed in [true, false, true, false] {
            pane_frame(&egui_ctx, &mut pane, vec![primary(end, pressed)]);
        }
        assert_eq!(pane.view.camera, OrbitCamera::default());
        assert_eq!(ctx.borrow().control_points, points);
        assert_eq!(ctx.borrow().spline_mode, SplineMode::Bezier);
    }
}
