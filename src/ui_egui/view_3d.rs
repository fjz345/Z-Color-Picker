use std::f32::consts::TAU;

use eframe::{
    egui::{
        self, lerp, Align2, CursorIcon, FontId, Painter, PointerButton, Pos2, Rect, Sense, Shape,
        Stroke, Ui, Vec2,
    },
    epaint::{CircleShape, Color32, Mesh},
};
use serde::{Deserialize, Serialize};

use crate::{
    color_space::{
        hsl_to_srgb, linear_to_color32, linear_to_oklab, linear_to_picker, linear_to_srgb,
        map_into_srgb_gamut, oklab_to_linear, srgb_to_hsl, srgb_to_hsv, srgb_to_linear, BlendSpace,
        PickerSpace,
    },
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::{ControlPoint, ControlPointValue},
    math::shortest_hue_delta,
    ui_egui::{
        auto_comb::drag_handle_to,
        blend::Gradient,
        color_picker::{
            apply_group_delta_sv, format_color_as, limit_group_delta, srgb_slide_fractions,
        },
        control_points::{
            bezier_handle, clip_points_to_srgb, clip_value_to_srgb, control_point_outline,
        },
    },
};

const CURVE_SAMPLES: usize = 128;

fn gradient_linear_samples(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    picker_space: PickerSpace,
    blend_space: BlendSpace,
) -> Vec<[f32; 3]> {
    let gradient = Gradient::new(control_points, spline_mode, picker_space, blend_space);
    (0..CURVE_SAMPLES)
        .filter_map(|i| gradient.linear_at(i as f32 / (CURVE_SAMPLES - 1) as f32))
        .collect()
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

    fn linear_at(self, position: [f32; 3]) -> [f32; 3] {
        match self {
            View3dSpace::Srgb => srgb_to_linear(cube_coordinates(position)),
            View3dSpace::LinearRgb => cube_coordinates(position),
            View3dSpace::Hsv | View3dSpace::Hsl => {
                self.cylinder_linear(cylinder_coordinates(position))
            }
            View3dSpace::Oklab => {
                let [a, height, minus_b] = position;
                map_into_srgb_gamut(oklab_to_linear([height + 0.5, a, -minus_b]))
            }
        }
    }

    fn cylinder_linear(self, [hue, saturation, height]: [f32; 3]) -> [f32; 3] {
        let srgb = match self {
            View3dSpace::Hsv => ecolor::rgb_from_hsv((hue, saturation, height)),
            View3dSpace::Hsl => hsl_to_srgb([hue, saturation, height]),
            other => unreachable!("{other:?} is not a cylinder"),
        };
        srgb_to_linear(srgb)
    }
}

fn centered_cube(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| c - 0.5)
}

fn cube_coordinates(position: [f32; 3]) -> [f32; 3] {
    position.map(|c| (c + 0.5).clamp(0.0, 1.0))
}

fn cylinder(hue_turns: f32, radius: f32, height: f32) -> [f32; 3] {
    let (sin, cos) = (hue_turns * TAU).sin_cos();
    let r = radius * CYLINDER_RADIUS;
    [r * cos, height - 0.5, -r * sin]
}

fn cylinder_coordinates([x, y, z]: [f32; 3]) -> [f32; 3] {
    let hue = ((-z).atan2(x) / TAU).rem_euclid(1.0);
    let radius = (x.hypot(z) / CYLINDER_RADIUS).min(1.0);
    [hue, radius, (y + 0.5).clamp(0.0, 1.0)]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    scene_radius: f32,
}

const MAX_PITCH: f32 = 1.48;
const MIN_DISTANCE: f32 = 1.2;
const MAX_DISTANCE: f32 = 8.0;
const ORBIT_RADIANS_PER_POINT: f32 = 0.01;
const ZOOM_PER_SCROLL_POINT: f32 = 0.002;
const FOCAL_LENGTH_PER_SIZE: f32 = 1.2;
const NEAR_DEPTH: f32 = 0.05;
const FAR_FADE: f32 = 0.45;
const FIT_FILL: f32 = 0.9;

fn gamut_radius(space: View3dSpace) -> f32 {
    gamut_quads(space)
        .iter()
        .flatten()
        .map(|vertex| dot(vertex.position, vertex.position))
        .fold(0.0, f32::max)
        .sqrt()
}

#[derive(Clone, Copy, Debug)]
struct Projected {
    pos: Pos2,
    depth: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl OrbitCamera {
    fn fitted(space: View3dSpace) -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.45,
            distance: 0.0,
            scene_radius: 0.0,
        }
        .refitted(space)
    }

    fn refitted(self, space: View3dSpace) -> Self {
        // A sphere of radius r at distance d projects to a circle of radius focal * r / sqrt(d^2 - r^2).
        let focal_per_filled_radius = 2.0 * FOCAL_LENGTH_PER_SIZE / FIT_FILL;
        let scene_radius = gamut_radius(space);
        let gamut_distance = scene_radius * (1.0 + focal_per_filled_radius.powi(2)).sqrt();
        let [right, up, toward_camera] = self.basis();
        let guide_ends = reference_segments(space, Color32::TRANSPARENT)
            .into_iter()
            .flat_map(|segment| segment.ends);
        let labels = axis_labels(space).iter().map(|&(_, position)| position);
        // Each guide end and label fits within FIT_FILL of a centered square of the shorter side.
        let distance = guide_ends
            .chain(labels)
            .map(|p| {
                let screen_offset = dot(p, right).abs().max(dot(p, up).abs());
                dot(p, toward_camera) + focal_per_filled_radius * screen_offset
            })
            .fold(gamut_distance, f32::max);
        Self {
            distance,
            scene_radius,
            ..self
        }
    }

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

    fn basis(self) -> [[f32; 3]; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let right = [cos_yaw, 0.0, -sin_yaw];
        let up = [-sin_pitch * sin_yaw, cos_pitch, -sin_pitch * cos_yaw];
        let toward_camera = [cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw];
        [right, up, toward_camera]
    }

    fn eye(self) -> [f32; 3] {
        self.basis()[2].map(|c| self.distance * c)
    }

    fn pixels_per_unit(depth: f32, rect: Rect) -> f32 {
        FOCAL_LENGTH_PER_SIZE * rect.size().min_elem() / depth
    }

    fn depth(self, p: [f32; 3]) -> f32 {
        self.distance - dot(p, self.basis()[2])
    }

    fn project(self, p: [f32; 3], rect: Rect) -> Option<Projected> {
        let [right, up, _] = self.basis();
        let depth = self.depth(p);
        if depth < NEAR_DEPTH {
            return None;
        }
        let offset = Vec2::new(dot(p, right), -dot(p, up));
        Some(Projected {
            pos: rect.center() + Self::pixels_per_unit(depth, rect) * offset,
            depth,
        })
    }

    fn unproject(self, pos: Pos2, depth: f32, rect: Rect) -> [f32; 3] {
        let [right, up, toward_camera] = self.basis();
        let offset = (pos - rect.center()) / Self::pixels_per_unit(depth, rect);
        std::array::from_fn(|i| {
            (self.distance - depth) * toward_camera[i] + offset.x * right[i] - offset.y * up[i]
        })
    }

    fn dragged_on_view_plane(
        self,
        position: [f32; 3],
        [from, to]: [Pos2; 2],
        rect: Rect,
    ) -> [f32; 3] {
        let depth = self.depth(position);
        let [from, to] = [from, to].map(|pos| self.unproject(pos, depth, rect));
        std::array::from_fn(|i| position[i] + (to[i] - from[i]))
    }

    fn size_scale(self, depth: f32) -> f32 {
        self.distance / depth
    }

    fn depth_fade(self, depth: f32) -> f32 {
        let nearest = self.distance - self.scene_radius;
        let far_fraction = ((depth - nearest) / (2.0 * self.scene_radius)).clamp(0.0, 1.0);
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
const OKLAB_FLOOR_AXIS: f32 = 0.28;

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
            color: linear_to_color32(midpoint(pair[0], pair[1])).gamma_multiply(opacity),
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
const OKLAB_LABEL_EXTENT: f32 = 0.35;
// Above white, the label would set the fitted distance and shrink the gamut.
const OKLAB_L_LABEL_SIDE: f32 = 0.08;
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
            ("L", [OKLAB_L_LABEL_SIDE, 0.5, 0.0]),
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
const HANDLE_RADIUS: f32 = 4.5;
const HANDLE_LINE_WIDTH: f32 = 1.5;
const HANDLE_MIN_SCREEN_LENGTH: f32 = 2.0 * SELECTED_POINT_RADIUS;
const VIEW_3D_HINT: &str =
    "Drag a point or handle: move it. Drag elsewhere: orbit. Scroll: zoom. Double-click elsewhere: reset";

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

fn nearest<T: Copy>(pointer: Pos2, targets: &[(Pos2, T)]) -> Option<(Pos2, T)> {
    targets
        .iter()
        .copied()
        .filter(|&(pos, _)| pos.distance_sq(pointer) <= HOVER_RADIUS * HOVER_RADIUS)
        .min_by(|a, b| {
            a.0.distance_sq(pointer)
                .total_cmp(&b.0.distance_sq(pointer))
        })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View3dItem {
    Point(usize),
    Handle(usize, usize),
}

impl View3dItem {
    fn point_index(self) -> usize {
        match self {
            View3dItem::Point(index) | View3dItem::Handle(index, _) => index,
        }
    }

    fn value(self, cp: &ControlPoint) -> ControlPointValue {
        match self {
            View3dItem::Point(_) => *cp.val(),
            View3dItem::Handle(_, side) => bezier_handle(cp, side),
        }
    }
}

struct SceneItem {
    item: View3dItem,
    linear: [f32; 3],
    projected: Projected,
    screen: Pos2,
}

fn shown_handle_sides(control_points: &[ControlPoint], index: usize) -> Vec<usize> {
    let Some(cp) = control_points.get(index) else {
        return Vec::new();
    };
    let is_last = index + 1 == control_points.len();
    (0..2)
        .filter(|&side| {
            let is_outer_end = (side == 0 && index == 0) || (side == 1 && is_last);
            cp.tangents()[side].is_some() && !is_outer_end
        })
        .collect()
}

fn handle_marker_pos(point: Pos2, handle: Pos2, side: usize) -> Pos2 {
    let offset = handle - point;
    if offset.length() >= HANDLE_MIN_SCREEN_LENGTH {
        return handle;
    }
    let direction = match (offset == Vec2::ZERO, side) {
        (false, _) => offset.normalized(),
        (true, 0) => -Vec2::X,
        (true, _) => Vec2::X,
    };
    point + HANDLE_MIN_SCREEN_LENGTH * direction
}

fn scene_items(
    camera: OrbitCamera,
    rect: Rect,
    space: View3dSpace,
    control_points: &[ControlPoint],
    picker_space: PickerSpace,
    handles_of: Option<usize>,
) -> Vec<SceneItem> {
    let project = |item: View3dItem| {
        let linear = item
            .value(control_points.get(item.point_index())?)
            .linear(picker_space);
        let projected = camera.project(space.position(linear), rect)?;
        Some(SceneItem {
            item,
            linear,
            projected,
            screen: projected.pos,
        })
    };
    let mut items: Vec<SceneItem> = (0..control_points.len())
        .filter_map(|index| project(View3dItem::Point(index)))
        .collect();
    let Some(index) = handles_of else {
        return items;
    };
    let Some(point_screen) = items
        .iter()
        .find(|scene_item| scene_item.item == View3dItem::Point(index))
        .map(|scene_item| scene_item.screen)
    else {
        return items;
    };
    for side in shown_handle_sides(control_points, index) {
        if let Some(handle) = project(View3dItem::Handle(index, side)) {
            items.push(SceneItem {
                screen: handle_marker_pos(point_screen, handle.screen, side),
                ..handle
            });
        }
    }
    items
}

fn handle_shapes(
    camera: OrbitCamera,
    point: &SceneItem,
    handle: &SceneItem,
    line_color: Color32,
) -> [(f32, Shape); 2] {
    let depth = handle.projected.depth;
    let fade = camera.depth_fade(depth);
    let scale = camera.size_scale(depth);
    let line = Shape::line_segment(
        [point.screen, handle.screen],
        Stroke::new(HANDLE_LINE_WIDTH * scale, line_color.gamma_multiply(fade)),
    );
    let fill = linear_to_color32(handle.linear);
    let outline = control_point_outline(fill, false);
    let marker = CircleShape {
        center: handle.screen,
        radius: HANDLE_RADIUS * scale,
        fill: fill.gamma_multiply(fade),
        stroke: Stroke::new(outline.width, outline.color.gamma_multiply(fade)),
    };
    [
        (depth.max(point.projected.depth), line),
        (depth - POINT_DEPTH_BIAS, Shape::Circle(marker)),
    ]
}

fn item_shapes(
    camera: OrbitCamera,
    items: &[SceneItem],
    selected_index: Option<usize>,
    handle_line_color: Color32,
) -> Vec<(f32, Shape)> {
    let mut shapes = Vec::new();
    for scene_item in items {
        match scene_item.item {
            View3dItem::Point(index) => shapes.push(point_shape(
                camera,
                scene_item.projected,
                linear_to_color32(scene_item.linear),
                selected_index == Some(index),
            )),
            View3dItem::Handle(index, _) => {
                if let Some(point) = items.iter().find(|p| p.item == View3dItem::Point(index)) {
                    shapes.extend(handle_shapes(camera, point, scene_item, handle_line_color));
                }
            }
        }
    }
    shapes
}

fn picker_value_from_linear(
    picker_space: PickerSpace,
    linear: [f32; 3],
    previous_z: f32,
) -> ControlPointValue {
    let [z, s, y] = linear_to_picker(picker_space, linear);
    assert!(
        [z, s, y].iter().all(|c| c.is_finite()),
        "{linear:?} in {picker_space:?}: {z}, {s}, {y}"
    );
    let is_achromatic = s <= 0.0;
    let z = if !picker_space.is_z_hue() {
        picker_space.z_on_bar(z)
    } else if is_achromatic {
        previous_z
    } else {
        previous_z + shortest_hue_delta(previous_z, z)
    };
    ControlPointValue::new(s.clamp(0.0, 1.0), y.clamp(0.0, 1.0), z)
}

// A value outside sRGB is shown at its gamut-mapped color, so it moves by the target's offset from there.
fn picker_value_after_drag(
    value: ControlPointValue,
    [shown, target]: [[f32; 3]; 2],
    picker_space: PickerSpace,
) -> ControlPointValue {
    let [shown, moved] =
        [shown, target].map(|linear| picker_value_from_linear(picker_space, linear, value.h()));
    // The linear round trip is not exact, so without motion it would still rewrite the value.
    if moved == shown {
        return value;
    }
    if value.is_in_srgb(picker_space) {
        return moved;
    }
    let dragged = value + (moved - shown);
    let z = if picker_space.is_z_hue() {
        dragged.h()
    } else {
        picker_space.z_on_bar(dragged.h())
    };
    ControlPointValue::new(dragged.s().clamp(0.0, 1.0), dragged.v().clamp(0.0, 1.0), z)
}

pub fn move_point_by_linear(
    control_points: &mut [ControlPoint],
    index: usize,
    start: ControlPointValue,
    shown_and_target: [[f32; 3]; 2],
    curve: CurveSettings,
) {
    let moved = picker_value_after_drag(start, shown_and_target, curve.picker_space);
    move_point_to(control_points, index, moved, curve);
}

fn move_point_to(
    control_points: &mut [ControlPoint],
    index: usize,
    moved: ControlPointValue,
    curve: CurveSettings,
) {
    let picker_space = curve.picker_space;
    let value = *control_points[index].val();
    if !curve.is_curve_locked {
        *control_points[index].val_mut() = moved;
        clip_points_to_srgb(&mut control_points[index..=index], picker_space);
        return;
    }
    // Like the square and the inspector: Lock moves every point's S/V, z only the dragged point's.
    let delta_sv = Vec2::new(moved.s() - value.s(), moved.v() - value.v());
    let delta_sv = if curve.is_window_lock {
        limit_group_delta(control_points, delta_sv)
    } else {
        delta_sv
    };
    let move_group = |points: &mut [ControlPoint], [x, y, z]: [f32; 3]| {
        apply_group_delta_sv(points, Vec2::new(x * delta_sv.x, y * delta_sv.y));
        points[index].val_mut()[2] = lerp(value.h()..=moved.h(), z);
    };
    let fractions = srgb_slide_fractions(control_points, picker_space, move_group);
    move_group(control_points, fractions);
    clip_points_to_srgb(control_points, picker_space);
}

const SCREEN_FIT_MOVES: usize = 48;
const SCREEN_FIT_PIXELS: f32 = 0.01;
const SCREEN_FIT_HALVINGS: i32 = 6;
const SCREEN_FIT_DIFFERENCE: f32 = 1e-3;
const SCREEN_FIT_DAMPING: f32 = 1e-3;
const SCREEN_FIT_FIRST_PATTERN_SIZE: f32 = 1.0 / 16.0;
const SCREEN_FIT_LAST_PATTERN_SIZE: f32 = 1.0 / 4096.0;
const SCREEN_FIT_ACCEPTED_PIXELS: f32 = 0.5;
const SCREEN_FIT_SEED_STEPS: usize = 8;
const SCREEN_FIT_SEEDS_SEARCHED: usize = 8;
const SCREEN_FIT_PATTERN: [[f32; 2]; 8] = [
    [1.0, 0.0],
    [-1.0, 0.0],
    [0.0, 1.0],
    [0.0, -1.0],
    [1.0, 1.0],
    [1.0, -1.0],
    [-1.0, 1.0],
    [-1.0, -1.0],
];

fn gauss_newton_step(
    at: [f32; 2],
    target: Pos2,
    screen_of: &impl Fn([f32; 2]) -> Option<Pos2>,
) -> Option<[f32; 2]> {
    let column = |axis: usize| {
        let (mut low, mut high) = (at, at);
        low[axis] = (at[axis] - SCREEN_FIT_DIFFERENCE).max(0.0);
        high[axis] = (at[axis] + SCREEN_FIT_DIFFERENCE).min(1.0);
        Some((screen_of(high)? - screen_of(low)?) / (high[axis] - low[axis]))
    };
    let [dx, dy] = [column(0)?, column(1)?];
    let damping = SCREEN_FIT_DAMPING * (dx.length_sq() + dy.length_sq());
    let [xx, xy, yy] = [
        dx.length_sq() + damping,
        dx.dot(dy),
        dy.length_sq() + damping,
    ];
    let determinant = xx * yy - xy * xy;
    if determinant <= 0.0 {
        return None;
    }
    let miss = screen_of(at)? - target;
    let [gx, gy] = [dx.dot(miss), dy.dot(miss)];
    Some([
        (xy * gy - yy * gx) / determinant,
        (xy * gx - xx * gy) / determinant,
    ])
}

fn first_nearer(
    target: Pos2,
    best: f32,
    candidates: impl IntoIterator<Item = [f32; 2]>,
    screen_of: &impl Fn([f32; 2]) -> Option<Pos2>,
) -> Option<([f32; 2], f32)> {
    candidates.into_iter().find_map(|candidate| {
        let candidate = candidate.map(|c| c.clamp(0.0, 1.0));
        let distance_sq = screen_of(candidate)?.distance_sq(target);
        (distance_sq < best).then_some((candidate, distance_sq))
    })
}

// The pattern gets past what stalls Gauss-Newton: kinks (HSL at l = 0.5), the square's edges
// and the flat run of chroma that gamut mapping puts on the sRGB edge.
fn locally_nearest_on_screen(
    start: [f32; 2],
    target: Pos2,
    screen_of: &impl Fn([f32; 2]) -> Option<Pos2>,
) -> Option<([f32; 2], f32)> {
    let mut xy = start.map(|c| c.clamp(0.0, 1.0));
    let mut best = screen_of(xy)?.distance_sq(target);
    let mut pattern_size = SCREEN_FIT_FIRST_PATTERN_SIZE;
    for _ in 0..SCREEN_FIT_MOVES {
        if best <= SCREEN_FIT_PIXELS * SCREEN_FIT_PIXELS {
            break;
        }
        let gauss_newton = gauss_newton_step(xy, target, screen_of)
            .into_iter()
            .flat_map(move |step| {
                (0..SCREEN_FIT_HALVINGS).map(move |halving| {
                    let scale = 0.5_f32.powi(halving);
                    [xy[0] + scale * step[0], xy[1] + scale * step[1]]
                })
            });
        let pattern = SCREEN_FIT_PATTERN
            .map(|[dx, dy]| [xy[0] + pattern_size * dx, xy[1] + pattern_size * dy]);
        match first_nearer(target, best, gauss_newton.chain(pattern), screen_of) {
            Some(nearer) => (xy, best) = nearer,
            None if pattern_size > SCREEN_FIT_LAST_PATTERN_SIZE => pattern_size *= 0.5,
            None => break,
        }
    }
    Some((xy, best))
}

fn grid_nearest_first_on_screen(
    target: Pos2,
    screen_of: &impl Fn([f32; 2]) -> Option<Pos2>,
) -> Vec<[f32; 2]> {
    let steps = SCREEN_FIT_SEED_STEPS;
    let mut seeds: Vec<(f32, [f32; 2])> = (0..=steps)
        .flat_map(|i| (0..=steps).map(move |j| [i, j].map(|k| k as f32 / steps as f32)))
        .filter_map(|xy| Some((screen_of(xy)?.distance_sq(target), xy)))
        .collect();
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    seeds.into_iter().map(|(_, xy)| xy).collect()
}

// From the start alone the search can stall in a local minimum, often tens of pixels off.
fn square_xy_nearest_on_screen(
    start: [f32; 2],
    target: Pos2,
    screen_of: impl Fn([f32; 2]) -> Option<Pos2>,
) -> [f32; 2] {
    let is_on_target = |(_, distance_sq): ([f32; 2], f32)| {
        distance_sq <= SCREEN_FIT_ACCEPTED_PIXELS * SCREEN_FIT_ACCEPTED_PIXELS
    };
    let Some(mut nearest) = locally_nearest_on_screen(start, target, &screen_of) else {
        return start;
    };
    if is_on_target(nearest) {
        return nearest.0;
    }
    let seeds = grid_nearest_first_on_screen(target, &screen_of);
    for seed in seeds.into_iter().take(SCREEN_FIT_SEEDS_SEARCHED) {
        let Some(found) = locally_nearest_on_screen(seed, target, &screen_of) else {
            continue;
        };
        if found.1 < nearest.1 {
            nearest = found;
        }
        if is_on_target(nearest) {
            break;
        }
    }
    nearest.0
}

// Outside sRGB it moves by the shown color's offset, like picker_value_after_drag;
// clipping the solution keeps chroma past the sRGB edge from piling onto that offset.
fn auto_hue_drag_value(
    value: ControlPointValue,
    target: Pos2,
    picker_space: PickerSpace,
    screen_of: impl Fn(ControlPointValue) -> Option<Pos2>,
) -> ControlPointValue {
    let z = value.h();
    let with_z = |[x, y]: [f32; 2]| ControlPointValue::new(x, y, z);
    let in_srgb = |xy: [f32; 2]| {
        if picker_space.can_leave_srgb() {
            with_z(xy).clipped_to_srgb(picker_space)
        } else {
            with_z(xy)
        }
    };
    let shown = in_srgb([value.s(), value.v()]);
    let solved = in_srgb(square_xy_nearest_on_screen(
        [shown.s(), shown.v()],
        target,
        |xy| screen_of(with_z(xy)),
    ));
    with_z([0, 1].map(|i| (value[i] + (solved[i] - shown[i])).clamp(0.0, 1.0)))
}

pub fn move_handle_to_linear(
    control_points: &mut [ControlPoint],
    index: usize,
    side: usize,
    start: &ControlPoint,
    shown_and_target: [[f32; 3]; 2],
    picker_space: PickerSpace,
    is_auto_comb_running: bool,
) {
    let start_handle = bezier_handle(start, side);
    let handle = picker_value_after_drag(start_handle, shown_and_target, picker_space);
    let handle = clip_value_to_srgb(handle, picker_space);
    if handle == start_handle {
        // handle - point is not the stored tangent bit for bit, nor a runaway one bezier_handle clamped.
        control_points[index].tangents_mut()[side] = start.tangents()[side];
    } else {
        drag_handle_to(control_points, index, side, handle, is_auto_comb_running);
    }
}

const FACE_STEPS: usize = 12;

#[derive(Clone, Copy)]
struct SurfaceVertex {
    position: [f32; 3],
    linear: [f32; 3],
}

fn surface_quads(
    [u_steps, v_steps]: [usize; 2],
    vertex: impl Fn(f32, f32) -> SurfaceVertex,
) -> Vec<[SurfaceVertex; 4]> {
    let grid: Vec<SurfaceVertex> = (0..=v_steps)
        .flat_map(|j| (0..=u_steps).map(move |i| (i, j)))
        .map(|(i, j)| vertex(i as f32 / u_steps as f32, j as f32 / v_steps as f32))
        .collect();
    let at = |i: usize, j: usize| grid[j * (u_steps + 1) + i];
    (0..v_steps)
        .flat_map(|j| (0..u_steps).map(move |i| (i, j)))
        .map(|(i, j)| [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)])
        .collect()
}

fn gamut_quads(space: View3dSpace) -> Vec<[SurfaceVertex; 4]> {
    match space {
        View3dSpace::Srgb | View3dSpace::LinearRgb | View3dSpace::Oklab => (0..3)
            .flat_map(|axis| [0.0, 1.0].map(|side| (axis, side)))
            .flat_map(|(axis, side)| {
                surface_quads([FACE_STEPS; 2], |u, v| {
                    let mut srgb = [side; 3];
                    srgb[(axis + 1) % 3] = u;
                    srgb[(axis + 2) % 3] = v;
                    let linear = srgb_to_linear(srgb);
                    SurfaceVertex {
                        position: space.position(linear),
                        linear,
                    }
                })
            })
            .collect(),
        View3dSpace::Hsv | View3dSpace::Hsl => {
            let vertex = |hue, saturation, height| SurfaceVertex {
                position: cylinder(hue, saturation, height),
                linear: space.cylinder_linear([hue, saturation, height]),
            };
            let mut quads = surface_quads([RING_STEPS, FACE_STEPS], |hue, height| {
                vertex(hue, 1.0, height)
            });
            for height in [0.0, 1.0] {
                quads.extend(surface_quads(
                    [RING_STEPS, FACE_STEPS],
                    |hue, saturation| vertex(hue, saturation, height),
                ));
            }
            quads
        }
    }
}

struct FaceQuad {
    depth: f32,
    corners: [Pos2; 4],
    colors: [Color32; 4],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn gamut_faces_back_and_front(
    camera: OrbitCamera,
    rect: Rect,
    space: View3dSpace,
    opacity: f32,
) -> [Vec<FaceQuad>; 2] {
    let (mut back, mut front) = (Vec::new(), Vec::new());
    if opacity <= 0.0 {
        return [back, front];
    }
    let interior = space.position(srgb_to_linear([0.5; 3]));
    let eye = camera.eye();
    for quad in gamut_quads(space) {
        let positions = quad.map(|vertex| vertex.position);
        let [Some(a), Some(b), Some(c), Some(d)] = positions.map(|p| camera.project(p, rect))
        else {
            continue;
        };
        let center: [f32; 3] =
            std::array::from_fn(|i| 0.25 * positions.iter().map(|p| p[i]).sum::<f32>());
        let normal = cross(
            sub(positions[2], positions[0]),
            sub(positions[3], positions[1]),
        );
        let faces_camera = dot(normal, sub(center, interior)) * dot(normal, sub(eye, center)) > 0.0;
        let face = FaceQuad {
            depth: 0.25 * (a.depth + b.depth + c.depth + d.depth),
            corners: [a.pos, b.pos, c.pos, d.pos],
            colors: quad.map(|vertex| linear_to_color32(vertex.linear).gamma_multiply(opacity)),
        };
        if faces_camera {
            front.push(face);
        } else {
            back.push(face);
        }
    }
    [back, front]
}

fn faces_mesh(mut faces: Vec<FaceQuad>) -> Option<Shape> {
    if faces.is_empty() {
        return None;
    }
    faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    let mut mesh = Mesh::default();
    for face in faces {
        let first = mesh.vertices.len() as u32;
        for (pos, color) in face.corners.into_iter().zip(face.colors) {
            mesh.colored_vertex(pos, color);
        }
        mesh.add_triangle(first, first + 1, first + 2);
        mesh.add_triangle(first, first + 2, first + 3);
    }
    Some(Shape::mesh(mesh))
}

fn item_at(items: &[SceneItem], pointer: Pos2) -> Option<(Pos2, View3dItem)> {
    let targets: Vec<(Pos2, View3dItem)> = items
        .iter()
        .map(|scene_item| (scene_item.screen, scene_item.item))
        .collect();
    nearest(pointer, &targets)
}

fn camera_after_input(
    camera: OrbitCamera,
    space: View3dSpace,
    ui: &Ui,
    response: &egui::Response,
    items: &[SceneItem],
    is_editing: bool,
) -> OrbitCamera {
    let is_on_item = response
        .interact_pointer_pos()
        .and_then(|pointer| item_at(items, pointer))
        .is_some();
    if response.double_clicked() && !is_on_item {
        return OrbitCamera::fitted(space);
    }
    let mut camera = camera;
    if response.dragged_by(PointerButton::Primary) && !is_editing {
        camera = camera.orbited(response.drag_delta());
    }
    if response.hovered() {
        camera = camera.zoomed(ui.input(|i| i.smooth_scroll_delta.y));
    }
    camera
}

const DEFAULT_GAMUT_OPACITY: f32 = 0.2;

fn default_gamut_opacity() -> f32 {
    DEFAULT_GAMUT_OPACITY
}

#[derive(Clone, Debug)]
struct ItemDrag {
    item: View3dItem,
    start: ControlPoint,
    start_position: [f32; 3],
    press: Pos2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct View3d {
    #[serde(default)]
    space: View3dSpace,
    #[serde(default = "default_gamut_opacity")]
    gamut_opacity: f32,
    #[serde(skip)]
    camera: Option<OrbitCamera>,
    #[serde(skip)]
    drag: Option<ItemDrag>,
}

impl Default for View3d {
    fn default() -> Self {
        Self {
            space: View3dSpace::default(),
            gamut_opacity: DEFAULT_GAMUT_OPACITY,
            camera: None,
            drag: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct CurveSettings {
    pub spline_mode: SplineMode,
    pub picker_space: PickerSpace,
    pub blend_space: BlendSpace,
    pub is_auto_comb_running: bool,
    pub is_curve_locked: bool,
    pub is_window_lock: bool,
    pub is_auto_hue: bool,
}

impl View3d {
    fn camera(&mut self) -> OrbitCamera {
        let space = self.space;
        *self
            .camera
            .get_or_insert_with(|| OrbitCamera::fitted(space))
    }

    fn set_space(&mut self, space: View3dSpace) {
        if space != self.space {
            self.space = space;
            self.camera = self.camera.map(|camera| camera.refitted(space));
        }
    }

    pub fn draw_ui(
        &mut self,
        ui: &mut Ui,
        control_points: &mut [ControlPoint],
        curve: CurveSettings,
        selected_index: &mut Option<usize>,
    ) -> Option<View3dItem> {
        let CurveSettings {
            spline_mode,
            picker_space,
            blend_space,
            ..
        } = curve;
        ui.horizontal_wrapped(|ui| {
            let mut space = self.space;
            egui::ComboBox::from_id_salt("view_3d_space")
                .selected_text(space.label())
                .show_ui(ui, |ui| {
                    for option in View3dSpace::ALL {
                        ui.selectable_value(&mut space, option, option.label());
                    }
                });
            self.set_space(space);
            ui.add(egui::Slider::new(&mut self.gamut_opacity, 0.0..=1.0).text("Gamut"))
                .on_hover_text("Opacity of the shaded gamut faces");
            ui.weak(VIEW_3D_HINT);
        });
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let handles_of =
            |selected: Option<usize>| selected.filter(|_| spline_mode == SplineMode::Bezier);
        let space = self.space;
        let items = scene_items(
            self.camera(),
            rect,
            space,
            control_points,
            picker_space,
            handles_of(*selected_index),
        );
        let dragged = self.edit_items(ui, &response, &items, control_points, curve, selected_index);
        let camera = camera_after_input(
            self.camera(),
            space,
            ui,
            &response,
            &items,
            self.drag.is_some(),
        );
        self.camera = Some(camera);
        let items = scene_items(
            camera,
            rect,
            space,
            control_points,
            picker_space,
            handles_of(*selected_index),
        );
        let guide = ui.visuals().weak_text_color();

        let samples =
            gradient_linear_samples(control_points, spline_mode, picker_space, blend_space);
        let mut segments = reference_segments(space, guide.gamma_multiply(REFERENCE_OPACITY));
        segments.extend(colored_polyline(space, &samples, CURVE_WIDTH, 1.0));
        let mut shapes: Vec<(f32, Shape)> = segments
            .iter()
            .filter_map(|segment| segment_shape(camera, rect, segment))
            .collect();
        shapes.extend(item_shapes(
            camera,
            &items,
            *selected_index,
            ui.visuals().text_color(),
        ));
        let item_targets: Vec<(Pos2, Color32)> = items
            .iter()
            .map(|scene_item| (scene_item.screen, linear_to_color32(scene_item.linear)))
            .collect();
        let sample_targets: Vec<(Pos2, Color32)> = samples
            .iter()
            .filter_map(|&linear| {
                let projected = camera.project(space.position(linear), rect)?;
                Some((projected.pos, linear_to_color32(linear)))
            })
            .collect();

        shapes.sort_by(|a, b| b.0.total_cmp(&a.0));
        let [back_faces, front_faces] =
            gamut_faces_back_and_front(camera, rect, space, self.gamut_opacity).map(faces_mesh);
        let painter = ui.painter_at(rect);
        painter.extend(back_faces);
        painter.extend(shapes.into_iter().map(|(_, shape)| shape));
        painter.extend(front_faces);
        paint_axis_labels(&painter, camera, rect, space, guide);

        let hovered_item = response
            .hover_pos()
            .and_then(|pointer| nearest(pointer, &item_targets));
        if self.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if hovered_item.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        let hovered = hovered_item
            .or_else(|| nearest(response.hover_pos()?, &sample_targets))
            .map(|(_, color)| color);
        if let Some(color) = hovered {
            let hex = format_color_as(color, ColorStringCopy::HexNoAlpha, None);
            response.on_hover_text_at_pointer(format!("#{hex}"));
        }
        dragged
    }

    fn edit_items(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        items: &[SceneItem],
        control_points: &mut [ControlPoint],
        curve: CurveSettings,
        selected_index: &mut Option<usize>,
    ) -> Option<View3dItem> {
        let space = self.space;
        if response.drag_started_by(PointerButton::Primary) {
            self.drag = ui.input(|i| i.pointer.press_origin()).and_then(|press| {
                let (_, item) = item_at(items, press)?;
                let pressed = items.iter().find(|scene_item| scene_item.item == item)?;
                Some(ItemDrag {
                    item,
                    start: control_points[item.point_index()].clone(),
                    start_position: space.position(pressed.linear),
                    press,
                })
            });
        }
        if !response.dragged_by(PointerButton::Primary) {
            self.drag = None;
        }
        if response.clicked() {
            let clicked = response
                .interact_pointer_pos()
                .and_then(|pointer| item_at(items, pointer));
            if let Some((_, item)) = clicked {
                *selected_index = Some(item.point_index());
            }
        }

        let drag = self.drag.clone()?;
        *selected_index = Some(drag.item.point_index());
        if !items.iter().any(|scene_item| scene_item.item == drag.item) {
            return None;
        }
        let pointer = response.interact_pointer_pos()?;
        let (camera, rect) = (self.camera(), response.rect);
        let moved = camera.dragged_on_view_plane(drag.start_position, [drag.press, pointer], rect);
        let shown_and_target = [drag.start_position, moved].map(|p| space.linear_at(p));
        let start_value = drag.item.value(&drag.start);
        let is_middle = |index: usize| 0 < index && index + 1 < control_points.len();
        match drag.item {
            View3dItem::Point(index) if curve.is_auto_hue && is_middle(index) => {
                let value = auto_hue_drag_value(
                    start_value,
                    camera.project(moved, rect)?.pos,
                    curve.picker_space,
                    |value| {
                        let linear = value.linear(curve.picker_space);
                        Some(camera.project(space.position(linear), rect)?.pos)
                    },
                );
                move_point_to(control_points, index, value, curve)
            }
            View3dItem::Point(index) => {
                move_point_by_linear(control_points, index, start_value, shown_and_target, curve)
            }
            View3dItem::Handle(index, side) => move_handle_to_linear(
                control_points,
                index,
                side,
                &drag.start,
                shown_and_target,
                curve.picker_space,
                curve.is_auto_comb_running,
            ),
        }
        Some(drag.item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, f32::consts::FRAC_PI_2, rc::Rc};

    use crate::{
        color_space::{is_in_srgb_gamut, oklab_to_oklch, BlendSpace},
        datatypes::control_point::ControlPointValue,
        ui_egui::{
            app::ZColorPickerAppContext,
            auto_comb::{apply_auto_comb, CombEdit},
            color_picker::srgb_move_fraction,
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
            ..OrbitCamera::fitted(View3dSpace::Srgb)
        }
    }

    #[test]
    fn scene_center_projects_to_the_rect_center_at_the_camera_distance() {
        let camera = OrbitCamera::fitted(View3dSpace::Srgb);
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
        let camera = OrbitCamera::fitted(View3dSpace::Srgb);
        assert_eq!(camera.orbited(Vec2::new(0.0, 1e6)).pitch, MAX_PITCH);
        assert_eq!(camera.orbited(Vec2::new(0.0, -1e6)).pitch, -MAX_PITCH);
    }

    #[test]
    fn zoom_clamps_distance() {
        let camera = OrbitCamera::fitted(View3dSpace::Srgb);
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

    const ROUND_TRIP_COLORS: [[f32; 3]; 6] = [
        [0.8, 0.3, 0.1],
        [0.05, 0.6, 0.9],
        [0.4, 0.4, 0.4],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
    ];

    #[test]
    fn linear_at_inverts_position_in_every_space() {
        for space in View3dSpace::ALL {
            for linear in ROUND_TRIP_COLORS {
                let position = space.position(linear);
                let actual = space.linear_at(position);
                for i in 0..3 {
                    assert!(
                        (actual[i] - linear[i]).abs() < 1e-4,
                        "{space:?}: {actual:?} != {linear:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn linear_at_clamps_positions_outside_the_space_into_it() {
        assert_close(
            View3dSpace::LinearRgb.linear_at([0.9, -0.5, 0.0]),
            [1.0, 0.0, 0.5],
        );
        assert_close(View3dSpace::Hsv.linear_at([2.0, 0.9, 0.0]), [1.0, 0.0, 0.0]);
        assert_close(View3dSpace::Hsl.linear_at([0.0, -0.9, 0.0]), [0.0; 3]);
        for position in [[0.6, 0.0, 0.0], [0.0, 0.9, -0.4], [-0.3, -0.7, 0.5]] {
            let linear = View3dSpace::Oklab.linear_at(position);
            assert!(linear.iter().all(|c| (0.0..=1.0).contains(c)), "{linear:?}");
        }
    }

    #[test]
    fn oklab_positions_outside_srgb_lose_chroma_at_their_lightness_and_hue() {
        for position in [[0.6, 0.0, 0.0], [0.0, 0.3, -0.2], [-0.25, -0.2, 0.1]] {
            let [a, height, minus_b] = position;
            let [wanted_l, wanted_chroma, wanted_hue] = oklab_to_oklch([height + 0.5, a, -minus_b]);
            let linear = View3dSpace::Oklab.linear_at(position);
            let [l, chroma, hue] = oklab_to_oklch(linear_to_oklab(linear));
            assert!(is_in_srgb_gamut(linear), "{position:?}: {linear:?}");
            assert!(
                (l - wanted_l).abs() < 1e-3,
                "{position:?}: L {l} != {wanted_l}"
            );
            assert!(
                shortest_hue_delta(hue, wanted_hue).abs() < 1e-3,
                "{position:?}: hue {hue} != {wanted_hue}"
            );
            assert!(chroma < wanted_chroma, "{position:?}");
        }
    }

    #[test]
    fn unproject_puts_the_point_under_the_target_at_the_same_depth() {
        let camera = OrbitCamera::fitted(View3dSpace::Srgb);
        let original = [0.2, -0.1, 0.3];
        let projected = camera.project(original, RECT).unwrap();
        let target = projected.pos + Vec2::new(13.0, -7.0);
        let moved = camera.unproject(target, projected.depth, RECT);
        let reprojected = camera.project(moved, RECT).unwrap();
        assert_pos_close(reprojected.pos, target);
        assert!((reprojected.depth - projected.depth).abs() < 1e-5);
    }

    #[test]
    fn nearest_picks_the_closest_target_within_the_pick_radius() {
        let pointer = Pos2::new(50.0, 50.0);
        let targets = [
            (Pos2::new(56.0, 50.0), View3dItem::Point(0)),
            (Pos2::new(50.0, 53.0), View3dItem::Handle(1, 0)),
            (Pos2::new(54.0, 50.0), View3dItem::Point(2)),
        ];
        assert_eq!(
            nearest(pointer, &targets),
            Some((targets[1].0, View3dItem::Handle(1, 0)))
        );
        assert_eq!(nearest(Pos2::new(90.0, 50.0), &targets), None);
    }

    fn point_with_tangents(value: [f32; 3], t: f32) -> ControlPoint {
        let mut cp = ControlPoint::new_simple(value.into(), t);
        *cp.tangents_mut() = [Some([-0.2, 0.1, 0.0].into()), Some([0.3, 0.2, 0.05].into())];
        cp
    }

    fn unlocked(picker_space: PickerSpace) -> CurveSettings {
        CurveSettings {
            spline_mode: SplineMode::Bezier,
            picker_space,
            blend_space: BlendSpace::Picker,
            is_auto_comb_running: false,
            is_curve_locked: false,
            is_window_lock: false,
            is_auto_hue: false,
        }
    }

    fn move_point_to_linear(
        points: &mut [ControlPoint],
        index: usize,
        target: [f32; 3],
        curve: CurveSettings,
    ) {
        let start = *points[index].val();
        let shown = start.linear(curve.picker_space);
        move_point_by_linear(points, index, start, [shown, target], curve);
    }

    #[test]
    fn moving_a_point_to_a_color_keeps_its_t_tangents_and_hue_turn() {
        let mut points = vec![point_with_tangents([0.5, 0.5, 1.2], 0.0)];
        let target = [0.8, 0.6, 0.2];
        move_point_to_linear(&mut points, 0, target, unlocked(PickerSpace::Okhsl));
        assert_close(points[0].val().linear(PickerSpace::Okhsl), target);
        assert!(points[0].val().h() > 1.0 && points[0].val().h() < 2.0);
        assert_eq!(*points[0].t(), 0.0);
        assert_eq!(
            *points[0].tangents(),
            *point_with_tangents([0.0; 3], 0.0).tangents()
        );
    }

    #[test]
    fn moving_a_point_to_a_gray_keeps_its_hue() {
        let mut points = vec![point_with_tangents([0.5, 0.5, 0.3], 0.0)];
        move_point_to_linear(&mut points, 0, [0.2; 3], unlocked(PickerSpace::Hsv));
        assert_eq!(points[0].val().h(), 0.3);
        assert_eq!(points[0].val().s(), 0.0);
    }

    fn hsv_points(values: [[f32; 3]; 3]) -> Vec<ControlPoint> {
        values
            .into_iter()
            .zip([0.0, 0.5, 1.0])
            .map(|(value, t)| point_with_tangents(value, t))
            .collect()
    }

    #[test]
    fn dragging_a_point_under_lock_moves_every_point_in_s_and_v_and_only_its_own_hue() {
        let original = hsv_points([[0.2, 0.3, 0.1], [0.5, 0.5, 0.4], [0.6, 0.2, 0.7]]);
        let mut points = original.clone();
        let target = ControlPointValue::new(0.7, 0.6, 0.45).linear(PickerSpace::Hsv);
        let locked = CurveSettings {
            is_curve_locked: true,
            ..unlocked(PickerSpace::Hsv)
        };
        move_point_to_linear(&mut points, 1, target, locked);
        for (index, (point, before)) in points.iter().zip(&original).enumerate() {
            let [s, v, h] = point.val().val;
            let [s0, v0, h0] = before.val().val;
            assert_close([s - s0, v - v0, 0.0], [0.2, 0.1, 0.0]);
            if index != 1 {
                assert_eq!(h, h0);
            }
        }
        assert_close([points[1].val().h(), 0.0, 0.0], [0.45, 0.0, 0.0]);
    }

    #[test]
    fn dragging_a_point_under_window_lock_stops_the_group_at_the_square_edge() {
        let mut points = hsv_points([[0.9, 0.3, 0.1], [0.5, 0.5, 0.4], [0.6, 0.2, 0.7]]);
        let target = ControlPointValue::new(0.8, 0.5, 0.4).linear(PickerSpace::Hsv);
        let window_locked = CurveSettings {
            is_curve_locked: true,
            is_window_lock: true,
            ..unlocked(PickerSpace::Hsv)
        };
        move_point_to_linear(&mut points, 1, target, window_locked);
        let s: Vec<f32> = points.iter().map(|point| point.val().s()).collect();
        assert_close([s[0], s[1], s[2]], [1.0, 0.6, 0.7]);
    }

    #[test]
    fn dragging_a_point_outside_srgb_keeps_its_offset_from_the_shown_color() {
        let oklch = PickerSpace::Oklch {
            clip_to_srgb: false,
        };
        let value = ControlPointValue::new(0.9, 0.6, 0.4);
        assert!(!value.is_in_srgb(oklch));
        let mut points = vec![point_with_tangents(value.val, 0.0)];
        let shown = value.linear(oklch);
        let [l, a, b] = linear_to_oklab(shown);
        let lighter = oklab_to_linear([l + 0.05, a, b]);
        move_point_by_linear(&mut points, 0, value, [shown, lighter], unlocked(oklch));
        let moved = *points[0].val();
        assert!(!moved.is_in_srgb(oklch), "{moved:?}");
        assert_close([moved.s(), moved.v(), moved.h()], [0.9, 0.65, 0.4]);
    }

    #[test]
    fn moving_a_handle_to_a_color_changes_only_its_offset() {
        let original = point_with_tangents([0.5, 0.5, 0.3], 0.5);
        let mut points = vec![original.clone()];
        let target = [0.1, 0.5, 0.7];
        let shown = bezier_handle(&points[0], 1).linear(PickerSpace::Hsl);
        move_handle_to_linear(
            &mut points,
            0,
            1,
            &original,
            [shown, target],
            PickerSpace::Hsl,
            false,
        );
        assert_close(
            bezier_handle(&points[0], 1).linear(PickerSpace::Hsl),
            target,
        );
        assert_eq!(points[0].val(), original.val());
        assert_eq!(points[0].t(), original.t());
        assert_eq!(points[0].tangents()[0], original.tangents()[0]);
        assert_ne!(points[0].tangents()[1], original.tangents()[1]);
    }

    const OKLAB: PickerSpace = PickerSpace::Oklab {
        clip_to_srgb: false,
    };

    #[test]
    fn moving_a_point_in_oklab_sets_its_lightness_instead_of_turning_a_hue() {
        let mut points = vec![point_with_tangents([0.5, 0.5, 0.95], 0.0)];
        let dark_gray = [0.01; 3];
        move_point_to_linear(&mut points, 0, dark_gray, unlocked(OKLAB));
        let value = *points[0].val();
        assert!(
            (value.h() - linear_to_oklab(dark_gray)[0]).abs() < 1e-4,
            "{value:?}"
        );
        assert_close(value.unmapped_linear(OKLAB), dark_gray);
    }

    #[test]
    fn moving_a_point_outside_srgb_clips_it_only_with_clip_on() {
        let outside = oklab_to_linear([0.7, 0.3, 0.0]);
        assert!(!is_in_srgb_gamut(outside));
        let oklch = PickerSpace::Oklch {
            clip_to_srgb: false,
        };
        for space in [oklch, OKLAB] {
            let gray = picker_value_from_linear(space, [0.2; 3], 0.5).val;
            let mut kept = vec![point_with_tangents(gray, 0.0)];
            move_point_to_linear(&mut kept, 0, outside, unlocked(space));
            assert_close(kept[0].val().unmapped_linear(space), outside);

            let clipping = space.with_clip_to_srgb(true);
            let mut clipped = vec![point_with_tangents(gray, 0.0)];
            move_point_to_linear(&mut clipped, 0, outside, unlocked(clipping));
            assert!(clipped[0].val().is_in_srgb(clipping), "{space:?}");
            assert_eq!(*clipped[0].val(), kept[0].val().clipped_to_srgb(clipping));
        }
    }

    #[test]
    fn dragging_a_point_under_lock_with_clip_stops_the_group_at_the_srgb_edge_keeping_its_shape() {
        let clipping = PickerSpace::Oklch { clip_to_srgb: true };
        let original = vec![
            point_with_tangents([0.2, 0.5, 0.3], 0.0),
            point_with_tangents([0.05, 0.6, 0.08], 1.0),
        ];
        let target_value = ControlPointValue::new(0.55, 0.6, 0.08);
        assert!(original[0].val().is_in_srgb(clipping));
        assert!(target_value.is_in_srgb(clipping));
        assert!(!ControlPointValue::new(0.7, 0.5, 0.3).is_in_srgb(clipping));
        let mut points = original.clone();
        let locked = CurveSettings {
            is_curve_locked: true,
            ..unlocked(clipping)
        };
        move_point_to_linear(&mut points, 1, target_value.linear(clipping), locked);

        let offset = |points: &[ControlPoint]| points[1].val().vec2() - points[0].val().vec2();
        let shape_change = (offset(&points) - offset(&original)).length();
        assert!(shape_change < 1e-5, "{points:?}");
        assert!(points.iter().all(|cp| cp.val().is_in_srgb(clipping)));
        let [s, v, h] = points[0].val().val;
        assert!(s > 0.2, "{points:?}");
        assert!(!ControlPointValue::new(s + 1e-3, v, h).is_in_srgb(clipping));
    }

    #[test]
    fn dragging_a_point_under_lock_with_clip_diagonally_into_the_srgb_edge_slides_along_it() {
        let clipping = PickerSpace::Oklch { clip_to_srgb: true };
        let original = vec![
            point_with_tangents([0.15, 0.3, 0.3], 0.0),
            point_with_tangents([0.1, 0.4, 0.3], 1.0),
        ];
        let target_value = ControlPointValue::new(0.38, 0.6, 0.3);
        assert!(target_value.is_in_srgb(clipping));
        let delta = target_value.vec2() - original[1].val().vec2();
        let stop_at_edge = delta
            * srgb_move_fraction(&original, clipping, |points, fraction| {
                apply_group_delta_sv(points, fraction * delta)
            });
        let mut points = original.clone();
        let locked = CurveSettings {
            is_curve_locked: true,
            ..unlocked(clipping)
        };
        move_point_to_linear(&mut points, 1, target_value.linear(clipping), locked);

        let offset = |points: &[ControlPoint]| points[1].val().vec2() - points[0].val().vec2();
        let shape_change = (offset(&points) - offset(&original)).length();
        assert!(shape_change < 1e-5, "{points:?}");
        assert!(points.iter().all(|cp| cp.val().is_in_srgb(clipping)));
        let slid = points[0].val().vec2() - original[0].val().vec2();
        assert!(
            slid.x >= stop_at_edge.x - 1e-5,
            "{slid:?} vs {stop_at_edge:?}"
        );
        assert!(
            slid.y > stop_at_edge.y + 0.05,
            "{slid:?} vs {stop_at_edge:?}"
        );
    }

    #[test]
    fn moving_a_handle_with_clip_on_pulls_it_into_srgb() {
        let oklch = PickerSpace::Oklch {
            clip_to_srgb: false,
        };
        for space in [oklch, OKLAB] {
            let clipping = space.with_clip_to_srgb(true);
            let mut points = vec![point_with_tangents([0.4, 0.5, 0.75], 0.0)];
            points[0].tangents_mut()[1] = Some([0.55, 0.0, 0.0].into());
            assert!(!bezier_handle(&points[0], 1).is_in_srgb(space), "{space:?}");
            let shown = bezier_handle(&points[0], 1).linear(space);
            let [l, a, b] = linear_to_oklab(shown);
            let lighter = oklab_to_linear([l + 0.02, a, b]);
            let start = points[0].clone();
            move_handle_to_linear(&mut points, 0, 1, &start, [shown, lighter], clipping, false);
            let handle = bezier_handle(&points[0], 1);
            assert!(handle.is_in_srgb(clipping), "{space:?}: {handle:?}");
        }
    }

    #[test]
    fn curve_samples_follow_the_picker_and_blend_spaces() {
        let hsl_red_to_blue = [(0.0, 0.0), (2.0 / 3.0, 1.0)]
            .map(|(h, t)| ControlPoint::new_simple(ControlPointValue::new(1.0, 0.5, h), t))
            .to_vec();
        let samples = gradient_linear_samples(
            &hsl_red_to_blue,
            SplineMode::Linear,
            PickerSpace::Hsl,
            BlendSpace::LinearRgb,
        );
        assert_eq!(samples.len(), CURVE_SAMPLES);
        for (i, sample) in samples.into_iter().enumerate() {
            let x = i as f32 / (CURVE_SAMPLES - 1) as f32;
            assert_close(sample, [1.0 - x, 0.0, x]);
        }
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

    fn mesh_count(output: &egui::FullOutput) -> usize {
        output
            .shapes
            .iter()
            .filter(|clipped| matches!(clipped.shape, Shape::Mesh(_)))
            .count()
    }

    fn view_rect(output: &egui::FullOutput) -> Rect {
        output
            .shapes
            .iter()
            .find_map(|clipped| {
                matches!(clipped.shape, Shape::Mesh(_)).then_some(clipped.clip_rect)
            })
            .expect("gamut faces are painted")
    }

    fn screen_pos(pane: &View3dPane, rect: Rect, value: ControlPointValue) -> Pos2 {
        let linear = value.linear(pane.ctx.borrow().picker_space);
        let position = pane.view.space.position(linear);
        pane.view
            .camera
            .unwrap()
            .project(position, rect)
            .unwrap()
            .pos
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
        ctx.borrow_mut().picker_space = PickerSpace::Okhsl;
        ctx.borrow_mut().blend_space = BlendSpace::Oklab;
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

        let rect = view_rect(&pane_frame(&egui_ctx, &mut pane, Vec::new()));
        let point = screen_pos(&pane, rect, *points[0].val());
        pane_frame(&egui_ctx, &mut pane, vec![egui::Event::PointerMoved(point)]);
        let hovered = painted_texts(&pane_frame(&egui_ctx, &mut pane, Vec::new()));
        let hex = format!(
            "#{}",
            format_color_as(
                points[0].val().color(PickerSpace::Okhsl),
                ColorStringCopy::HexNoAlpha,
                None
            )
        );
        assert!(hovered.contains(&hex), "{hovered:?}");

        let background = rect.left_top() + Vec2::splat(20.0);
        let end = background + Vec2::new(60.0, 30.0);
        pane_frame(&egui_ctx, &mut pane, vec![primary(background, true)]);
        for step in 1..=3 {
            let pos = background.lerp(end, step as f32 / 3.0);
            pane_frame(&egui_ctx, &mut pane, vec![egui::Event::PointerMoved(pos)]);
        }
        pane_frame(&egui_ctx, &mut pane, vec![primary(end, false)]);
        let orbited = pane.view.camera.unwrap();
        let start = OrbitCamera::fitted(View3dSpace::Srgb);
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
            pane.view.camera.unwrap().distance < orbited.distance,
            "{:?}",
            pane.view.camera
        );

        for pressed in [true, false, true, false] {
            pane_frame(&egui_ctx, &mut pane, vec![primary(end, pressed)]);
        }
        assert_eq!(
            pane.view.camera,
            Some(OrbitCamera::fitted(View3dSpace::Srgb))
        );
        assert_eq!(ctx.borrow().control_points, points);
        assert_eq!(ctx.borrow().spline_mode, SplineMode::Bezier);
    }

    fn three_points() -> Vec<ControlPoint> {
        [
            (0.5, 0.6, 0.1, 0.0),
            (0.4, 0.5, 0.5, 0.5),
            (0.6, 0.4, 0.8, 1.0),
        ]
        .map(|(s, v, h, t)| ControlPoint::new_simple(ControlPointValue::new(s, v, h), t))
        .to_vec()
    }

    fn pane_with(
        points: &[ControlPoint],
        spline_mode: SplineMode,
        selected: Option<usize>,
    ) -> View3dPane {
        let ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        {
            let mut app_ctx = ctx.borrow_mut();
            app_ctx.control_points = points.to_vec();
            app_ctx.spline_mode = spline_mode;
            app_ctx
                .z_color_picker
                .borrow_mut()
                .last_modifying_point_index = selected;
        }
        View3dPane {
            title: None,
            ctx,
            view: View3d::default(),
        }
    }

    fn drag(egui_ctx: &egui::Context, pane: &mut View3dPane, start: Pos2, end: Pos2) {
        pane_frame(egui_ctx, pane, vec![egui::Event::PointerMoved(start)]);
        pane_frame(egui_ctx, pane, vec![primary(start, true)]);
        for step in 1..=3 {
            let pos = start.lerp(end, step as f32 / 3.0);
            pane_frame(egui_ctx, pane, vec![egui::Event::PointerMoved(pos)]);
        }
        pane_frame(egui_ctx, pane, vec![primary(end, false)]);
    }

    fn drag_first_point(pane: &mut View3dPane) -> (Rect, Pos2) {
        let egui_ctx = egui::Context::default();
        let rect = view_rect(&pane_frame(&egui_ctx, pane, Vec::new()));
        let first = *pane.ctx.borrow().control_points[0].val();
        let start = screen_pos(pane, rect, first);
        let end = start + Vec2::new(12.0, -9.0);
        drag(&egui_ctx, pane, start, end);
        (rect, end)
    }

    #[test]
    fn dragging_a_point_moves_only_it_under_the_pointer_and_writes_it_back() {
        let points = three_points();
        let mut pane = pane_with(&points, SplineMode::Linear, None);
        pane.ctx
            .borrow()
            .z_color_picker
            .borrow_mut()
            .options
            .is_hue_middle_interpolated = false;
        let (rect, end) = drag_first_point(&mut pane);

        let ctx = pane.ctx.borrow();
        let moved = &ctx.control_points;
        assert_ne!(moved[0].val(), points[0].val());
        assert_eq!(moved[0].t(), points[0].t());
        assert_eq!(&moved[1..], &points[1..]);
        let under = screen_pos(&pane, rect, *moved[0].val());
        assert!(under.distance(end) < 0.5, "{under:?} != {end:?}");
        let color_picker = ctx.z_color_picker.borrow();
        assert_eq!(color_picker.last_modifying_point_index, Some(0));
        assert_eq!(color_picker.dragging_index, Some(0));
        assert_eq!(color_picker.dragging_tangent, None);
        assert_eq!(
            pane.view.camera,
            Some(OrbitCamera::fitted(View3dSpace::Srgb))
        );
    }

    #[test]
    fn dragging_a_point_runs_the_picker_constraints() {
        let mut pane = pane_with(&three_points(), SplineMode::Linear, None);
        drag_first_point(&mut pane);
        let ctx = pane.ctx.borrow();
        let [first, middle, last] = [0, 1, 2].map(|i| *ctx.control_points[i].val());
        let auto_hue = crate::math::hue_lerp(first.h(), last.h(), 0.5);
        assert!((middle.h() - auto_hue).abs() < 1e-5, "{middle:?}");
    }

    #[test]
    fn dragging_a_handle_changes_only_its_tangent_and_marks_it_dragged() {
        let points: Vec<ControlPoint> = [(0.3, 0.6, 0.0), (0.5, 0.5, 0.5), (0.7, 0.4, 1.0)]
            .map(|(s, v, t)| point_with_tangents([s, v, 0.3], t))
            .to_vec();
        let mut pane = pane_with(&points, SplineMode::Bezier, Some(1));
        let egui_ctx = egui::Context::default();
        let rect = view_rect(&pane_frame(&egui_ctx, &mut pane, Vec::new()));
        let point = screen_pos(&pane, rect, *points[1].val());
        let handle = screen_pos(&pane, rect, bezier_handle(&points[1], 1));
        let start = handle_marker_pos(point, handle, 1);
        drag(&egui_ctx, &mut pane, start, start + Vec2::new(10.0, 8.0));

        let ctx = pane.ctx.borrow();
        let moved = &ctx.control_points;
        assert_ne!(moved[1].tangents()[1], points[1].tangents()[1]);
        for (index, (after, before)) in moved.iter().zip(&points).enumerate() {
            assert_eq!(after.val(), before.val());
            assert_eq!(after.t(), before.t());
            assert_eq!(after.tangents()[0], before.tangents()[0]);
            if index != 1 {
                assert_eq!(after.tangents()[1], before.tangents()[1]);
            }
        }
        assert_eq!(ctx.z_color_picker.borrow().dragging_tangent, Some((1, 1)));
    }

    fn seeded_arc() -> Vec<ControlPoint> {
        let handles = [
            [[0.0, 0.15], [0.0, -0.15]],
            [[-0.15, 0.0], [0.15, 0.0]],
            [[0.0, -0.15], [0.0, 0.15]],
        ];
        let mut points: Vec<ControlPoint> = [(0.2, 0.8, 0.0), (0.5, 0.5, 0.5), (0.8, 0.8, 1.0)]
            .iter()
            .zip(handles)
            .map(|(&(s, v, t), handles)| {
                let mut cp = ControlPoint::new_simple(ControlPointValue::new(s, v, 0.3), t);
                *cp.tangents_mut() = handles.map(|[ds, dv]| Some([ds, dv, 0.0].into()));
                cp
            })
            .collect();
        let reseed = CombEdit {
            is_reseed: true,
            ..Default::default()
        };
        assert_eq!(apply_auto_comb(&mut points, reseed), 0);
        assert!(points.iter().all(|cp| cp.curvature().is_some()));
        points
    }

    fn stretch_handle(pane: &mut View3dPane, index: usize, side: usize) {
        let egui_ctx = egui::Context::default();
        let rect = view_rect(&pane_frame(&egui_ctx, pane, Vec::new()));
        let cp = pane.ctx.borrow().control_points[index].clone();
        let point = screen_pos(pane, rect, *cp.val());
        let handle = screen_pos(pane, rect, bezier_handle(&cp, side));
        let start = handle_marker_pos(point, handle, side);
        let end = start + 12.0 * (start - point).normalized();
        drag(&egui_ctx, pane, start, end);
    }

    #[test]
    fn dragging_a_handle_with_auto_comb_on_sets_its_point_curvature() {
        for is_auto_comb in [false, true] {
            let points = seeded_arc();
            let mut pane = pane_with(&points, SplineMode::Bezier, Some(1));
            pane.ctx
                .borrow()
                .z_color_picker
                .borrow_mut()
                .options
                .is_auto_comb = is_auto_comb;
            stretch_handle(&mut pane, 1, 1);

            let ctx = pane.ctx.borrow();
            let moved = &ctx.control_points;
            assert_ne!(moved[1].tangents()[1], points[1].tangents()[1]);
            assert_eq!(
                moved[1].curvature() != points[1].curvature(),
                is_auto_comb,
                "auto comb {is_auto_comb}: {:?} -> {:?}",
                points[1].curvature(),
                moved[1].curvature()
            );
        }
    }

    const OKLCH: PickerSpace = PickerSpace::Oklch {
        clip_to_srgb: false,
    };
    const HELD_FRAMES: usize = 5;

    fn view_rect_then_hide_gamut(egui_ctx: &egui::Context, pane: &mut View3dPane) -> Rect {
        let rect = view_rect(&pane_frame(egui_ctx, pane, Vec::new()));
        // The faces cost most of a debug frame and take no part in a drag.
        pane.view.gamut_opacity = 0.0;
        rect
    }

    fn drag_hold_and_return<T>(
        egui_ctx: &egui::Context,
        pane: &mut View3dPane,
        [start, end]: [Pos2; 2],
        read: impl Fn(&[ControlPoint]) -> T,
    ) -> [T; 3] {
        let read_pane = |pane: &View3dPane| read(&pane.ctx.borrow().control_points);
        pane_frame(egui_ctx, pane, vec![egui::Event::PointerMoved(start)]);
        pane_frame(egui_ctx, pane, vec![primary(start, true)]);
        for step in 1..=3 {
            let pos = start.lerp(end, step as f32 / 3.0);
            pane_frame(egui_ctx, pane, vec![egui::Event::PointerMoved(pos)]);
        }
        let moved = read_pane(pane);
        for _ in 0..HELD_FRAMES {
            pane_frame(egui_ctx, pane, Vec::new());
        }
        let held = read_pane(pane);
        for pos in [start.lerp(end, 0.5), start] {
            pane_frame(egui_ctx, pane, vec![egui::Event::PointerMoved(pos)]);
        }
        let returned = read_pane(pane);
        pane_frame(egui_ctx, pane, vec![primary(start, false)]);
        [moved, held, returned]
    }

    #[test]
    fn a_point_outside_srgb_stays_put_while_held_and_comes_back_bit_for_bit() {
        let [first_hue, last_hue] = [0.1, 0.6];
        let middle_hue = crate::math::hue_lerp(first_hue, last_hue, 0.5);
        let points: Vec<ControlPoint> = [
            (0.9, 0.7, first_hue, 0.0),
            (0.9, 0.5, middle_hue, 0.5),
            (0.3, 0.4, last_hue, 1.0),
        ]
        .map(|(s, v, h, t)| ControlPoint::new_simple(ControlPointValue::new(s, v, h), t))
        .to_vec();
        for space in View3dSpace::ALL {
            for index in [0, 1] {
                assert!(!points[index].val().is_in_srgb(OKLCH));
                let mut pane = pane_with(&points, SplineMode::Linear, None);
                pane.ctx.borrow_mut().picker_space = OKLCH;
                pane.view.space = space;
                let egui_ctx = egui::Context::default();
                let rect = view_rect_then_hide_gamut(&egui_ctx, &mut pane);
                let start = screen_pos(&pane, rect, *points[index].val());
                let path = [start, start + Vec2::new(12.0, -9.0)];
                let bits = |points: &[ControlPoint]| points[index].val().val.map(f32::to_bits);
                let [moved, held, returned] =
                    drag_hold_and_return(&egui_ctx, &mut pane, path, bits);
                let label = format!("point {index} in {space:?}");
                assert_ne!(moved, bits(&points), "{label}");
                assert_eq!(held, moved, "{label}");
                assert_eq!(returned, bits(&points), "{label}");
            }
        }
    }

    #[test]
    fn a_handle_outside_srgb_stays_put_while_held_and_comes_back_bit_for_bit() {
        let mut points: Vec<ControlPoint> = [(0.3, 0.6, 0.0), (0.4, 0.5, 0.5), (0.7, 0.4, 1.0)]
            .map(|(s, v, t)| point_with_tangents([s, v, 0.75], t))
            .to_vec();
        points[1].tangents_mut()[1] = Some([0.55, 0.0, 0.0].into());
        for picker_space in [OKLCH, OKLAB] {
            assert!(!bezier_handle(&points[1], 1).is_in_srgb(picker_space));
            for space in View3dSpace::ALL {
                let mut pane = pane_with(&points, SplineMode::Bezier, Some(1));
                pane.ctx.borrow_mut().picker_space = picker_space;
                pane.view.space = space;
                let egui_ctx = egui::Context::default();
                let rect = view_rect_then_hide_gamut(&egui_ctx, &mut pane);
                let point = screen_pos(&pane, rect, *points[1].val());
                let handle = screen_pos(&pane, rect, bezier_handle(&points[1], 1));
                let start = handle_marker_pos(point, handle, 1);
                let path = [start, start + Vec2::new(10.0, 8.0)];
                let bits = |points: &[ControlPoint]| {
                    points[1].tangents()[1].unwrap().val.map(f32::to_bits)
                };
                let [moved, held, returned] =
                    drag_hold_and_return(&egui_ctx, &mut pane, path, bits);
                let label = format!("{picker_space:?} in {space:?}");
                assert_ne!(moved, bits(&points), "{label}");
                assert_eq!(held, moved, "{label}");
                assert_eq!(returned, bits(&points), "{label}");
            }
        }
    }

    #[test]
    fn fitted_camera_keeps_every_label_and_guide_inside_the_pane() {
        let wide = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let tall = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 400.0));
        let orbits = [
            Vec2::ZERO,
            Vec2::new(150.0, 100.0),
            Vec2::new(-220.0, -140.0),
        ];
        for space in View3dSpace::ALL {
            let guide_ends = reference_segments(space, Color32::WHITE)
                .into_iter()
                .flat_map(|segment| segment.ends);
            let labels = axis_labels(space).iter().map(|&(_, position)| position);
            let drawn: Vec<[f32; 3]> = guide_ends.chain(labels).collect();
            for orbit in orbits {
                let camera = OrbitCamera::fitted(space).orbited(orbit).refitted(space);
                for rect in [wide, tall] {
                    for &position in &drawn {
                        let pos = camera.project(position, rect).unwrap().pos;
                        assert!(
                            rect.contains(pos),
                            "{space:?} in {rect:?} from {camera:?}: {position:?} at {pos:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn switching_the_space_refits_the_distance_and_keeps_the_orientation() {
        let mut view = View3d::default();
        let orbited = OrbitCamera::fitted(View3dSpace::Srgb)
            .orbited(Vec2::new(40.0, -25.0))
            .zoomed(80.0);
        view.camera = Some(orbited);
        view.set_space(View3dSpace::Srgb);
        assert_eq!(view.camera, Some(orbited));

        view.set_space(View3dSpace::Oklab);
        let camera = view.camera.unwrap();
        assert_eq!(view.space, View3dSpace::Oklab);
        assert_eq!([camera.yaw, camera.pitch], [orbited.yaw, orbited.pitch]);
        assert_eq!(camera, orbited.refitted(View3dSpace::Oklab));
        assert_ne!(camera.distance, orbited.distance);
    }

    #[test]
    fn depth_fade_spans_the_depths_of_the_spaces_own_gamut_sphere() {
        for space in View3dSpace::ALL {
            let camera = OrbitCamera::fitted(space).zoomed(-120.0);
            let radius = gamut_radius(space);
            let [near, far] = [-radius, radius].map(|r| camera.depth_fade(camera.distance + r));
            assert_eq!(near, 1.0, "{space:?}");
            assert!((far - (1.0 - FAR_FADE)).abs() < 1e-6, "{space:?}: {far}");
            let middle = camera.depth_fade(camera.distance);
            assert!(
                (middle - (1.0 - 0.5 * FAR_FADE)).abs() < 1e-6,
                "{space:?}: {middle}"
            );
        }
    }

    #[test]
    fn clicking_a_point_selects_it_without_editing() {
        let points = three_points();
        let mut pane = pane_with(&points, SplineMode::Bezier, Some(0));
        let egui_ctx = egui::Context::default();
        let rect = view_rect(&pane_frame(&egui_ctx, &mut pane, Vec::new()));
        let pos = screen_pos(&pane, rect, *points[2].val());
        pane_frame(&egui_ctx, &mut pane, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            pane_frame(&egui_ctx, &mut pane, vec![primary(pos, pressed)]);
        }
        let ctx = pane.ctx.borrow();
        assert_eq!(
            ctx.z_color_picker.borrow().last_modifying_point_index,
            Some(2)
        );
        assert_eq!(ctx.control_points, points);
    }

    #[test]
    fn gamut_faces_are_painted_only_with_a_positive_opacity() {
        let mut pane = pane_with(&three_points(), SplineMode::Linear, None);
        let egui_ctx = egui::Context::default();
        for space in View3dSpace::ALL {
            pane.view.space = space;
            let output = pane_frame(&egui_ctx, &mut pane, Vec::new());
            assert_eq!(mesh_count(&output), 2, "{space:?}");
        }
        pane.view.gamut_opacity = 0.0;
        assert_eq!(mesh_count(&pane_frame(&egui_ctx, &mut pane, Vec::new())), 0);
    }

    #[test]
    fn front_camera_sees_only_the_near_cube_face_in_front() {
        let distance = 2.6;
        let [back, front] =
            gamut_faces_back_and_front(front_camera(distance), RECT, View3dSpace::Srgb, 0.5);
        assert_eq!(front.len(), FACE_STEPS * FACE_STEPS);
        assert_eq!(back.len(), 5 * FACE_STEPS * FACE_STEPS);
        assert!(front
            .iter()
            .all(|face| (face.depth - (distance - 0.5)).abs() < 1e-4));
    }

    #[test]
    fn view_json_without_gamut_opacity_loads_the_default() {
        let view: View3d = serde_json::from_str(r#"{"space":"Hsv"}"#).unwrap();
        assert_eq!(view.space, View3dSpace::Hsv);
        assert_eq!(view.gamut_opacity, DEFAULT_GAMUT_OPACITY);
    }

    fn double_click(pane: &mut View3dPane, pos: Pos2) {
        let egui_ctx = egui::Context::default();
        pane_frame(&egui_ctx, pane, Vec::new());
        pane_frame(&egui_ctx, pane, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false, true, false] {
            pane_frame(&egui_ctx, pane, vec![primary(pos, pressed)]);
        }
    }

    #[test]
    fn double_clicking_a_point_or_handle_keeps_the_camera_and_empty_space_resets_it() {
        let points: Vec<ControlPoint> = [(0.3, 0.6, 0.0), (0.5, 0.5, 0.5), (0.7, 0.4, 1.0)]
            .map(|(s, v, t)| point_with_tangents([s, v, 0.3], t))
            .to_vec();
        let mut pane = pane_with(&points, SplineMode::Bezier, Some(1));
        let rect = view_rect(&pane_frame(
            &egui::Context::default(),
            &mut pane,
            Vec::new(),
        ));
        let orbited = pane.view.camera.unwrap().orbited(Vec2::new(40.0, -25.0));
        pane.view.camera = Some(orbited);
        let point = screen_pos(&pane, rect, *points[1].val());
        let handle = screen_pos(&pane, rect, bezier_handle(&points[1], 1));
        for pos in [point, handle_marker_pos(point, handle, 1)] {
            double_click(&mut pane, pos);
            assert_eq!(pane.view.camera, Some(orbited), "{pos:?}");
        }
        double_click(&mut pane, rect.left_top() + Vec2::splat(20.0));
        assert_eq!(
            pane.view.camera,
            Some(OrbitCamera::fitted(View3dSpace::Srgb))
        );
        assert_eq!(pane.ctx.borrow().control_points, points);
    }

    #[test]
    fn a_new_pane_starts_at_the_camera_fitted_to_its_space() {
        let mut pane = pane_with(&three_points(), SplineMode::Linear, None);
        pane.view.space = View3dSpace::Oklab;
        pane_frame(&egui::Context::default(), &mut pane, Vec::new());
        assert_eq!(
            pane.view.camera,
            Some(OrbitCamera::fitted(View3dSpace::Oklab))
        );
    }

    #[test]
    fn fitted_camera_fills_most_of_the_shorter_side_with_the_gamut() {
        let wide = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let tall = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 400.0));
        for space in View3dSpace::ALL {
            let camera = OrbitCamera::fitted(space);
            assert!(
                (MIN_DISTANCE..=MAX_DISTANCE).contains(&camera.distance),
                "{space:?}: {camera:?}"
            );
            for rect in [wide, tall] {
                let corners: Vec<Pos2> = gamut_quads(space)
                    .iter()
                    .flatten()
                    .map(|vertex| camera.project(vertex.position, rect).unwrap().pos)
                    .collect();
                let gamut = Rect::from_points(&corners);
                assert!(rect.contains_rect(gamut), "{space:?}: {gamut:?}");
                let fill = gamut.size().max_elem() / rect.size().min_elem();
                assert!(fill > 0.7, "{space:?} in {rect:?}: {fill}");
            }
        }
    }

    fn dragged_then_snapped(
        pane: &View3dPane,
        rect: Rect,
        points: &[ControlPoint],
        index: usize,
        end: Pos2,
    ) -> ControlPointValue {
        let picker_space = pane.ctx.borrow().picker_space;
        let (camera, space) = (pane.view.camera.unwrap(), pane.view.space);
        let shown = points[index].val().linear(picker_space);
        let depth = camera.project(space.position(shown), rect).unwrap().depth;
        let target = space.linear_at(camera.unproject(end, depth, rect));
        let mut snapped = points.to_vec();
        let start = *points[index].val();
        move_point_by_linear(
            &mut snapped,
            index,
            start,
            [shown, target],
            unlocked(picker_space),
        );
        snapped[index].val_mut()[2] = points[index].val().h();
        *snapped[index].val()
    }

    #[test]
    fn auto_hue_drag_keeps_a_middle_hue_and_lands_nearer_the_pointer_than_drag_then_snap() {
        let [first_hue, last_hue] = [0.1, 0.5];
        let middle_hue = crate::math::hue_lerp(first_hue, last_hue, 0.5);
        let points: Vec<ControlPoint> = [
            (0.5, 0.6, first_hue, 0.0),
            (0.4, 0.5, middle_hue, 0.5),
            (0.6, 0.4, last_hue, 1.0),
        ]
        .map(|(s, v, h, t)| ControlPoint::new_simple(ControlPointValue::new(s, v, h), t))
        .to_vec();
        let hue_spaces = PickerSpace::ALL
            .into_iter()
            .filter(|space| space.is_z_hue());
        for picker_space in hue_spaces {
            for space in View3dSpace::ALL {
                let mut pane = pane_with(&points, SplineMode::Linear, None);
                pane.ctx.borrow_mut().picker_space = picker_space;
                pane.view.space = space;
                let egui_ctx = egui::Context::default();
                let rect = view_rect_then_hide_gamut(&egui_ctx, &mut pane);
                let start = screen_pos(&pane, rect, *points[1].val());
                let end = start + Vec2::new(14.0, -10.0);
                drag(&egui_ctx, &mut pane, start, end);

                let moved = *pane.ctx.borrow().control_points[1].val();
                let label = format!("{picker_space:?} in {space:?}: {moved:?}");
                assert_ne!(moved, *points[1].val(), "{label}");
                assert_eq!(moved.h().to_bits(), middle_hue.to_bits(), "{label}");
                let snapped = dragged_then_snapped(&pane, rect, &points, 1, end);
                let [moved_miss, snapped_miss] =
                    [moved, snapped].map(|value| screen_pos(&pane, rect, value).distance(end));
                assert!(
                    moved_miss < snapped_miss,
                    "{label}: {moved_miss} >= {snapped_miss}"
                );
            }
        }
    }

    #[test]
    fn auto_hue_drag_from_any_start_lands_on_a_spot_its_hue_covers() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0));
        let mut seed: u64 = 12345;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let above_noisy_near_black = |y: f32| 0.1 + 0.9 * y;
        let hue_spaces = PickerSpace::ALL
            .into_iter()
            .filter(|space| space.is_z_hue());
        for picker_space in hue_spaces {
            for space in View3dSpace::ALL {
                let camera = OrbitCamera::fitted(space);
                let screen_of = |value: ControlPointValue| {
                    let linear = value.linear(picker_space);
                    Some(camera.project(space.position(linear), rect)?.pos)
                };
                for _ in 0..40 {
                    let z = next();
                    let start = ControlPointValue::new(next(), next(), z);
                    let start = start.clipped_to_srgb(picker_space);
                    let covered = ControlPointValue::new(next(), above_noisy_near_black(next()), z);
                    let target = screen_of(covered).unwrap();
                    let moved = auto_hue_drag_value(start, target, picker_space, screen_of);
                    let miss = screen_of(moved).unwrap().distance(target);
                    assert!(
                        miss < 0.5,
                        "{picker_space:?} in {space:?} from {start:?} to {covered:?}: {miss}"
                    );
                }
            }
        }
    }

    #[test]
    fn auto_hue_drag_under_lock_with_clip_stops_the_group_at_the_srgb_edge_keeping_its_shape() {
        let clipping = PickerSpace::Oklch { clip_to_srgb: true };
        let [first_hue, last_hue] = [0.3, 0.5];
        let middle_hue = crate::math::hue_lerp(first_hue, last_hue, 0.5);
        let original: Vec<ControlPoint> = [
            (0.2, 0.5, first_hue, 0.0),
            (0.15, 0.55, middle_hue, 0.5),
            (0.1, 0.6, last_hue, 1.0),
        ]
        .map(|(s, v, h, t)| ControlPoint::new_simple(ControlPointValue::new(s, v, h), t))
        .to_vec();
        assert!(original.iter().all(|cp| cp.val().is_in_srgb(clipping)));
        let mut pane = pane_with(&original, SplineMode::Linear, None);
        pane.ctx.borrow_mut().picker_space = clipping;
        pane.ctx
            .borrow()
            .z_color_picker
            .borrow_mut()
            .options
            .is_curve_locked = true;
        let egui_ctx = egui::Context::default();
        let rect = view_rect(&pane_frame(&egui_ctx, &mut pane, Vec::new()));
        let start = screen_pos(&pane, rect, *original[1].val());
        let end = screen_pos(&pane, rect, ControlPointValue::new(0.6, 0.55, middle_hue));
        drag(&egui_ctx, &mut pane, start, end);

        let points = pane.ctx.borrow().control_points.clone();
        let offsets = |points: &[ControlPoint]| {
            [1, 2].map(|i| points[i].val().vec2() - points[0].val().vec2())
        };
        for (offset, before) in offsets(&points).into_iter().zip(offsets(&original)) {
            assert!((offset - before).length() < 1e-5, "{points:?}");
        }
        assert!(points.iter().all(|cp| cp.val().is_in_srgb(clipping)));
        let hues = points.iter().map(|cp| cp.val().h().to_bits());
        assert!(hues.eq([first_hue, middle_hue, last_hue].map(f32::to_bits)));
        assert!(points[0].val().s() > 0.2, "{points:?}");
        let is_at_edge = |cp: &ControlPoint| {
            let [s, v, h] = cp.val().val;
            !ControlPointValue::new(s + 1e-3, v, h).is_in_srgb(clipping)
        };
        assert!(points.iter().any(is_at_edge), "{points:?}");
    }
}
