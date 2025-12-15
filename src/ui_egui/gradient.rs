#[allow(unused_imports)]
use crate::error::Result;
use eframe::egui::*;

pub fn color_function_gradient(
    ui: &mut Ui,
    size: Vec2,
    color_at: impl Fn(f32) -> Color32,
) -> Response {
    let (rect, response) = ui.allocate_at_least(size, Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let _visuals = ui.style().interact(&response);

        // background_checkers(ui.painter(), rect); // for alpha:

        {
            let num_: u32 = 6 * 6;
            // fill color:
            let mut mesh = Mesh::default();
            for i in 0..=num_ {
                let t = i as f32 / (num_ as f32);
                let color = color_at(t);
                let x = lerp(rect.left()..=rect.right(), t);
                mesh.colored_vertex(pos2(x, rect.top()), color);
                mesh.colored_vertex(pos2(x, rect.bottom()), color);
                if i < num_ {
                    mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                    mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
                }
            }
            ui.painter().add(Shape::mesh(mesh));
        }
    }

    response
}

pub fn mesh_gradient(ui: &mut Ui, size: Vec2, vertex_colors: &[Color32]) -> Response {
    let (rect, response) = ui.allocate_at_least(size, Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let _visuals = ui.style().interact(&response);

        // background_checkers(ui.painter(), rect); // for alpha:

        {
            let num_: u32 = (vertex_colors.len() - 1) as u32;
            // fill color:
            let mut mesh = Mesh::default();
            for i in 0..=num_ {
                let t = i as f32 / (num_ as f32);
                let index = (t * (vertex_colors.len()) as f32) as usize;
                let color = vertex_colors[index.clamp(0, vertex_colors.len() - 1)];
                let x = lerp(rect.left()..=rect.right(), t);
                mesh.colored_vertex(pos2(x, rect.top()), color);
                mesh.colored_vertex(pos2(x, rect.bottom()), color);
                if i < num_ {
                    mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                    mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
                }
            }
            ui.painter().add(Shape::mesh(mesh));
        }
    }

    response
}

pub fn vertex_gradient(ui: &mut Ui, size: Vec2, bg_fill: Color32, gradient: &Gradient) -> Response {
    let (rect, response) = ui.allocate_at_least(size, Sense::hover());
    if bg_fill != Default::default() {
        let mut mesh = Mesh::default();
        mesh.add_colored_rect(rect, bg_fill);
        ui.painter().add(Shape::mesh(mesh));
    }
    {
        let n = 256;
        assert!(n >= 2);
        let mut mesh = Mesh::default();
        for (i, color) in gradient.iter(n).enumerate() {
            let t = i as f32 / (n as f32 - 1.0);
            let x = lerp(rect.x_range(), t);
            mesh.colored_vertex(pos2(x, rect.top()), color);
            mesh.colored_vertex(pos2(x, rect.bottom()), color);
            if i < n - 1 {
                let i = i as u32;
                mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2);
                mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
            }
        }
        ui.painter().add(Shape::mesh(mesh));
    }
    response
}

fn lerp_color_gamma(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        lerp(a.r() as f32..=b.r() as f32, t).round() as u8,
        lerp(a.g() as f32..=b.g() as f32, t).round() as u8,
        lerp(a.b() as f32..=b.b() as f32, t).round() as u8,
        lerp(a.a() as f32..=b.a() as f32, t).round() as u8,
    )
}
#[derive(Clone, Copy)]
pub enum GammaMode {
    Linear,
    Gamma,
}

#[derive(Clone, Copy)]
pub struct Gradient {
    start: Color32,
    end: Color32,
    mode: GammaMode,
}

impl Gradient {
    pub fn solid(color: Color32) -> Self {
        Self {
            start: color,
            end: color,
            mode: GammaMode::Gamma,
        }
    }

    pub fn endpoints(start: Color32, end: Color32, mode: GammaMode) -> Self {
        Self { start, end, mode }
    }

    /// Sample the gradient at t ∈ [0, 1]
    pub fn sample(&self, t: f32) -> Color32 {
        match self.mode {
            GammaMode::Gamma => lerp_color_gamma(self.start, self.end, t),
            GammaMode::Linear => {
                let a = Rgba::from(self.start);
                let b = Rgba::from(self.end);
                Color32::from(lerp(a..=b, t))
            }
        }
    }

    pub fn iter(&self, steps: usize) -> impl Iterator<Item = Color32> + '_ {
        (0..steps).map(move |i| {
            let t = i as f32 / (steps - 1) as f32;
            self.sample(t)
        })
    }
}
