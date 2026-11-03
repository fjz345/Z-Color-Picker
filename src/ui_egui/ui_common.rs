use eframe::egui::InnerResponse;
use eframe::egui::Window;
use eframe::{
    egui::{Painter, Response, Sense, Ui, WidgetInfo, WidgetType},
    emath::remap_clamp,
    epaint::{vec2, Color32, Mesh, Pos2, Rect, Rgba, Shape, StrokeKind, Vec2},
};

use crate::ui_egui::gradient::color_function_gradient;

pub fn black_or_white_against(background: Color32) -> Color32 {
    let [r, g, b, _] = Rgba::from(background).to_array();
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if luminance < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

const CHECKER_CELL: f32 = 6.0;
const CHECKER_DARK: Color32 = Color32::from_gray(40);
const CHECKER_LIGHT: Color32 = Color32::from_gray(115);

fn checkerboard(rect: Rect) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(rect, CHECKER_DARK);
    let columns = (rect.width() / CHECKER_CELL).ceil().max(0.0) as usize;
    let rows = (rect.height() / CHECKER_CELL).ceil().max(0.0) as usize;
    for row in 0..rows {
        for column in (row % 2..columns).step_by(2) {
            let min = rect.min + CHECKER_CELL * vec2(column as f32, row as f32);
            let cell = Rect::from_min_size(min, Vec2::splat(CHECKER_CELL)).intersect(rect);
            mesh.add_colored_rect(cell, CHECKER_LIGHT);
        }
    }
    mesh
}

pub fn paint_swatch(painter: &Painter, rect: Rect, color: Color32) {
    if !color.is_opaque() {
        painter.add(Shape::mesh(checkerboard(rect)));
    }
    painter.rect_filled(rect, 0.0, color);
}

pub fn swatch(ui: &mut Ui, size: Vec2, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));
    paint_swatch(ui.painter(), rect, color);
    response
}

fn paint_outline(ui: &Ui, response: &Response) {
    let stroke = ui.style().interact(response).bg_stroke;
    ui.painter()
        .rect_stroke(response.rect, 0.0, stroke, StrokeKind::Middle);
}

pub fn color_strip(
    ui: &mut Ui,
    size: Vec2,
    color_at: impl Fn(f32) -> Color32,
) -> (Response, Option<f32>) {
    let mut response = color_function_gradient(ui, size, color_at);
    paint_outline(ui, &response);
    let picked = response
        .interact_pointer_pos()
        .map(|pointer| remap_clamp(pointer.x, response.rect.x_range(), 0.0..=1.0));
    if picked.is_some() {
        response.mark_changed();
    }
    (response, picked)
}

const COLOR_FIELD_CELLS: u32 = 32;
const COLOR_FIELD_EDGE_SUB_CELLS: u32 = 8;

// `area` is in field fractions, x right and y down.
fn add_grid_vertices(
    mesh: &mut Mesh,
    rect: Rect,
    area: Rect,
    cells: u32,
    color_at: &impl Fn(f32, f32) -> Color32,
) -> u32 {
    let first = mesh.vertices.len() as u32;
    for row in 0..=cells {
        for column in 0..=cells {
            let unit = area.lerp_inside(vec2(column as f32, row as f32) / cells as f32);
            mesh.colored_vertex(
                rect.lerp_inside(unit.to_vec2()),
                color_at(unit.x, 1.0 - unit.y),
            );
        }
    }
    first
}

fn add_grid_cell(mesh: &mut Mesh, first: u32, cells: u32, row: u32, column: u32) {
    let top_left = first + row * (cells + 1) + column;
    let bottom_left = top_left + cells + 1;
    mesh.add_triangle(top_left, top_left + 1, bottom_left);
    mesh.add_triangle(top_left + 1, bottom_left + 1, bottom_left);
}

// A cell with blank (alpha 0) and colored corners is drawn as a finer grid in the second mesh, so
// the color fades into the blank over one sub-cell instead of the whole cell.
fn color_field_mesh(rect: Rect, cells: u32, color_at: impl Fn(f32, f32) -> Color32) -> [Mesh; 2] {
    assert!(cells > 0);
    let mut field = Mesh::default();
    let mut edges = Mesh::default();
    let unit = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0));
    add_grid_vertices(&mut field, rect, unit, cells, &color_at);
    for row in 0..cells {
        for column in 0..cells {
            let is_blank = [(0, 0), (0, 1), (1, 0), (1, 1)].map(|(down, right)| {
                let corner = (row + down) * (cells + 1) + column + right;
                field.vertices[corner as usize].color.a() == 0
            });
            if !(is_blank.contains(&true) && is_blank.contains(&false)) {
                add_grid_cell(&mut field, 0, cells, row, column);
                continue;
            }
            let min = Pos2::new(column as f32, row as f32) / cells as f32;
            let area = Rect::from_min_size(min, Vec2::splat(1.0 / cells as f32));
            let sub_cells = COLOR_FIELD_EDGE_SUB_CELLS;
            let first = add_grid_vertices(&mut edges, rect, area, sub_cells, &color_at);
            for sub_row in 0..sub_cells {
                for sub_column in 0..sub_cells {
                    add_grid_cell(&mut edges, first, sub_cells, sub_row, sub_column);
                }
            }
        }
    }
    [field, edges]
}

pub fn color_field(ui: &mut Ui, size: Vec2, color_at: impl Fn(f32, f32) -> Color32) -> Response {
    let (rect, response) = ui.allocate_at_least(size, Sense::click());
    if ui.is_rect_visible(rect) {
        for mesh in color_field_mesh(rect, COLOR_FIELD_CELLS, color_at) {
            if !mesh.vertices.is_empty() {
                ui.painter().add(Shape::mesh(mesh));
            }
        }
        paint_outline(ui, &response);
    }
    response
}

pub trait ContentWindow {
    fn is_open(&self) -> bool;
    fn close(&mut self);
    fn open(&mut self);

    fn title(&self) -> &str {
        "Content Window"
    }

    fn draw_content(&mut self, _ui: &mut Ui) {}

    fn draw_ui(&mut self, ui: &mut Ui) -> Option<InnerResponse<Option<()>>> {
        let prev_visuals = ui.visuals_mut().clone();

        let mut open = self.is_open();
        let response = Window::new(self.title())
            .resizable(true)
            .title_bar(true)
            .open(&mut open)
            .auto_sized()
            .show(ui.ctx(), |ui| {
                self.draw_content(ui);
            });

        if open {
            self.open();
        } else {
            self.close();
        }

        ui.ctx().set_visuals(prev_visuals);

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, Event, PointerButton, Pos2};

    const STRIP_SIZE: Vec2 = Vec2::new(200.0, 40.0);
    const FIELD_SIZE: Vec2 = Vec2::new(160.0, 160.0);
    const SWATCH_SIZE: Vec2 = Vec2::new(30.0, 20.0);

    fn run_frame(
        ctx: &egui::Context,
        events: Vec<Event>,
        mut add: impl FnMut(&mut Ui),
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| add(ui));
        })
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn painted_colors_within(output: &egui::FullOutput, rect: Rect) -> Vec<Color32> {
        let mut colors = Vec::new();
        for clipped in &output.shapes {
            if !rect.contains_rect(clipped.shape.visual_bounding_rect()) {
                continue;
            }
            match &clipped.shape {
                Shape::Rect(shape) => colors.push(shape.fill),
                Shape::Mesh(mesh) => colors.extend(mesh.vertices.iter().map(|v| v.color)),
                _ => {}
            }
        }
        colors.retain(|color| *color != Color32::TRANSPARENT);
        colors
    }

    fn vertex_color_at(output: &egui::FullOutput, pos: Pos2) -> Option<Color32> {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                Shape::Mesh(mesh) => mesh.vertices.iter().find(|v| v.pos == pos).map(|v| v.color),
                _ => None,
            })
    }

    fn strip_color(t: f32) -> Color32 {
        Color32::from_gray((t * 255.0).round() as u8)
    }

    fn strip(ui: &mut Ui) -> (Response, Option<f32>) {
        color_strip(ui, STRIP_SIZE, strip_color)
    }

    fn strip_frame(ctx: &egui::Context, events: Vec<Event>) -> (Response, Option<f32>) {
        let mut result = None;
        run_frame(ctx, events, |ui| result = Some(strip(ui)));
        result.unwrap()
    }

    fn field_color(x: f32, y: f32) -> Color32 {
        Color32::from_rgb((x * 255.0).round() as u8, (y * 255.0).round() as u8, 0)
    }

    fn field(ui: &mut Ui) -> Response {
        color_field(ui, FIELD_SIZE, field_color)
    }

    fn swatch_frame(color: Color32) -> (Response, egui::FullOutput) {
        let mut response = None;
        let output = run_frame(&egui::Context::default(), vec![], |ui| {
            response = Some(swatch(ui, SWATCH_SIZE, color));
        });
        (response.unwrap(), output)
    }

    #[test]
    fn contrast_is_white_on_dark_colors_and_black_on_bright_ones() {
        for (background, expected) in [
            (Color32::BLACK, Color32::WHITE),
            (Color32::WHITE, Color32::BLACK),
            (Color32::from_gray(100), Color32::WHITE),
            (Color32::from_gray(200), Color32::BLACK),
            (Color32::RED, Color32::WHITE),
            (Color32::GREEN, Color32::BLACK),
            (Color32::BLUE, Color32::WHITE),
            (Color32::YELLOW, Color32::BLACK),
            (Color32::from_rgb(255, 0, 255), Color32::WHITE),
            (Color32::from_rgb(0, 255, 255), Color32::BLACK),
        ] {
            assert_eq!(
                black_or_white_against(background),
                expected,
                "{background:?}"
            );
        }
    }

    #[test]
    fn strip_allocates_its_size_and_senses_click_and_drag() {
        let (response, _) = strip_frame(&egui::Context::default(), vec![]);
        assert_eq!(response.rect.size(), STRIP_SIZE);
        assert!(response.sense.senses_click());
        assert!(response.sense.senses_drag());
    }

    #[test]
    fn strip_picks_the_pointer_fraction_clamped_to_its_ends() {
        let ctx = egui::Context::default();
        let (response, picked) = strip_frame(&ctx, vec![]);
        assert_eq!(picked, None);
        let rect = response.rect;
        let quarter = rect.lerp_inside(Vec2::new(0.25, 0.5));
        strip_frame(&ctx, vec![Event::PointerMoved(quarter)]);
        let (response, picked) = strip_frame(&ctx, vec![press(quarter, true)]);
        assert!(response.changed());
        assert!((picked.unwrap() - 0.25).abs() < 1e-6, "{picked:?}");

        let left_of_strip = rect.left_center() - Vec2::X * 5.0;
        let (_, picked) = strip_frame(&ctx, vec![Event::PointerMoved(left_of_strip)]);
        assert_eq!(picked, Some(0.0));
        let right_of_strip = rect.right_center() + Vec2::X * 5.0;
        let (_, picked) = strip_frame(&ctx, vec![Event::PointerMoved(right_of_strip)]);
        assert_eq!(picked, Some(1.0));

        strip_frame(&ctx, vec![press(right_of_strip, false)]);
        let (response, picked) = strip_frame(&ctx, vec![]);
        assert_eq!(picked, None);
        assert!(!response.changed());
    }

    #[test]
    fn strip_paints_color_at_from_left_to_right() {
        let mut rect = Rect::NOTHING;
        let output = run_frame(&egui::Context::default(), vec![], |ui| {
            rect = strip(ui).0.rect;
        });
        assert_eq!(
            vertex_color_at(&output, rect.left_top()),
            Some(strip_color(0.0))
        );
        assert_eq!(
            vertex_color_at(&output, rect.left_bottom()),
            Some(strip_color(0.0))
        );
        assert_eq!(
            vertex_color_at(&output, rect.right_top()),
            Some(strip_color(1.0))
        );
        assert_eq!(
            vertex_color_at(&output, rect.right_bottom()),
            Some(strip_color(1.0))
        );
    }

    #[test]
    fn field_allocates_its_size_and_senses_clicks_only() {
        let mut response = None;
        run_frame(&egui::Context::default(), vec![], |ui| {
            response = Some(field(ui));
        });
        let response = response.unwrap();
        assert_eq!(response.rect.size(), FIELD_SIZE);
        assert!(response.sense.senses_click());
        assert!(!response.sense.senses_drag());
    }

    #[test]
    fn field_paints_color_at_with_y_up_from_the_bottom_left_corner() {
        let mut rect = Rect::NOTHING;
        let output = run_frame(&egui::Context::default(), vec![], |ui| {
            rect = field(ui).rect
        });
        for (corner, x, y) in [
            (rect.left_bottom(), 0.0, 0.0),
            (rect.left_top(), 0.0, 1.0),
            (rect.right_top(), 1.0, 1.0),
            (rect.right_bottom(), 1.0, 0.0),
        ] {
            assert_eq!(
                vertex_color_at(&output, corner),
                Some(field_color(x, y)),
                "{corner:?}"
            );
        }
    }

    #[test]
    fn field_fades_into_blank_cells_over_one_edge_sub_cell_only() {
        let boundary = 0.3;
        let unit = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0));
        let meshes = color_field_mesh(unit, COLOR_FIELD_CELLS, |x, _| {
            if x < boundary {
                Color32::WHITE
            } else {
                Color32::TRANSPARENT
            }
        });
        let sub_cell = 1.0 / (COLOR_FIELD_CELLS * COLOR_FIELD_EDGE_SUB_CELLS) as f32;
        let mut mixed = 0;
        for mesh in &meshes {
            for triangle in mesh.indices.chunks(3) {
                let vertices = triangle.iter().map(|&i| mesh.vertices[i as usize]);
                let blank = vertices.clone().filter(|v| v.color.a() == 0).count();
                if blank == 0 || blank == 3 {
                    continue;
                }
                mixed += 1;
                let xs = vertices.map(|v| v.pos.x);
                let width = xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min);
                assert!(width <= sub_cell + 1e-6, "{width}");
            }
        }
        assert!(mixed > 0);
    }

    #[test]
    fn swatch_allocates_its_exact_size_and_senses_click_and_drag() {
        let (response, _) = swatch_frame(Color32::RED);
        assert_eq!(response.rect.size(), SWATCH_SIZE);
        assert!(response.sense.senses_click());
        assert!(response.sense.senses_drag());
    }

    #[test]
    fn opaque_swatch_paints_only_its_color() {
        let color = Color32::from_rgb(10, 120, 200);
        let (response, output) = swatch_frame(color);
        let colors = painted_colors_within(&output, response.rect);
        assert!(!colors.is_empty());
        assert!(colors.iter().all(|c| *c == color), "{colors:?}");
    }

    #[test]
    fn transparent_swatch_paints_opaque_checkers() {
        let (response, output) = swatch_frame(Color32::TRANSPARENT);
        let mut colors = painted_colors_within(&output, response.rect);
        colors.dedup();
        colors.sort_by_key(|c| c.to_array());
        colors.dedup();
        assert_eq!(colors.len(), 2, "{colors:?}");
        assert!(colors.iter().all(|c| c.is_opaque()), "{colors:?}");
    }
}
