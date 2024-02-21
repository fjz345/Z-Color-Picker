use eframe::egui::{self, *};

pub fn vertex_gradient(ui: &mut Ui, size: Vec2, bg_fill: Color32, gradient: &Gradient) -> Response {
    use egui::epaint::*;
    let (rect, response) = ui.allocate_at_least(size, Sense::hover());
    if bg_fill != Default::default() {
        let mut mesh = Mesh::default();
        mesh.add_colored_rect(rect, bg_fill);
        ui.painter().add(Shape::mesh(mesh));
    }
    {
        let n = gradient.0.len();
        assert!(n >= 2);
        let mut mesh = Mesh::default();
        for (i, &color) in gradient.0.iter().enumerate() {
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

#[derive(Clone, Copy)]
pub enum Interpolation {
    Linear,
    Gamma,
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Gradient(pub Vec<Color32>);

impl Gradient {
    pub fn one_color(srgba: Color32) -> Self {
        Self(vec![srgba, srgba])
    }

    pub fn endpoints(left: Color32, right: Color32) -> Self {
        Self(vec![left, right])
    }

    pub fn ground_truth_gradient(
        left: Color32,
        right: Color32,
        interpolation: Interpolation,
    ) -> Self {
        match interpolation {
            Interpolation::Linear => Self::ground_truth_linear_gradient(left, right),
            Interpolation::Gamma => Self::ground_truth_gamma_gradient(left, right),
        }
    }

    pub fn ground_truth_linear_gradient(left: Color32, right: Color32) -> Self {
        let left = Rgba::from(left);
        let right = Rgba::from(right);

        let n = 255;
        Self(
            (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    Color32::from(lerp(left..=right, t))
                })
                .collect(),
        )
    }

    pub fn ground_truth_gamma_gradient(left: Color32, right: Color32) -> Self {
        let n = 255;
        Self(
            (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    lerp_color_gamma(left, right, t)
                })
                .collect(),
        )
    }

    /// Do premultiplied alpha-aware blending of the gradient on top of the fill color
    /// in gamma-space.
    pub fn with_bg_fill(self, bg: Color32) -> Self {
        Self(
            self.0
                .into_iter()
                .map(|fg| {
                    let a = fg.a() as f32 / 255.0;
                    Color32::from_rgba_premultiplied(
                        (bg[0] as f32 * (1.0 - a) + fg[0] as f32).round() as u8,
                        (bg[1] as f32 * (1.0 - a) + fg[1] as f32).round() as u8,
                        (bg[2] as f32 * (1.0 - a) + fg[2] as f32).round() as u8,
                        (bg[3] as f32 * (1.0 - a) + fg[3] as f32).round() as u8,
                    )
                })
                .collect(),
        )
    }

    pub fn to_pixel_row(&self) -> Vec<Color32> {
        self.0.clone()
    }
}

fn mul_color_gamma(left: Color32, right: Color32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (left.r() as f32 * right.r() as f32 / 255.0).round() as u8,
        (left.g() as f32 * right.g() as f32 / 255.0).round() as u8,
        (left.b() as f32 * right.b() as f32 / 255.0).round() as u8,
        (left.a() as f32 * right.a() as f32 / 255.0).round() as u8,
    )
}

fn lerp_color_gamma(left: Color32, right: Color32, t: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        lerp((left[0] as f32)..=(right[0] as f32), t).round() as u8,
        lerp((left[1] as f32)..=(right[1] as f32), t).round() as u8,
        lerp((left[2] as f32)..=(right[2] as f32), t).round() as u8,
        lerp((left[3] as f32)..=(right[3] as f32), t).round() as u8,
    )
}
