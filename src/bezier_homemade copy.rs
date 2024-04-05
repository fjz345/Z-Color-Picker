use crate::error::Result;

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
