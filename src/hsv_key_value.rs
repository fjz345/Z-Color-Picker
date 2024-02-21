type HsvKeyValueInnerType = [f32; 3];
#[derive(Copy, Clone)]
pub struct HsvKeyValue {
    pub val: HsvKeyValueInnerType,
}

impl Default for HsvKeyValue {
    fn default() -> Self {
        Self {
            val: [0.0, 0.0, 0.0],
        }
    }
}

impl From<HsvKeyValueInnerType> for HsvKeyValue {
    fn from(item: HsvKeyValueInnerType) -> Self {
        HsvKeyValue { val: item }
    }
}

impl std::ops::Index<usize> for HsvKeyValue {
    type Output = f32;
    fn index(&self, s: usize) -> &f32 {
        match s {
            0 => &self.val[0],
            1 => &self.val[1],
            2 => &self.val[2],
            _ => panic!("unknown field: {}", s),
        }
    }
}

impl std::ops::IndexMut<usize> for HsvKeyValue {
    fn index_mut(&mut self, s: usize) -> &mut f32 {
        match s {
            0 => &mut self.val[0],
            1 => &mut self.val[1],
            2 => &mut self.val[2],
            _ => panic!("unknown field: {}", s),
        }
    }
}

// impl std::ops::IndexMut<&'_ usize> for HsvKeyValue {
//     fn index_mut(&mut self, s: &str) -> &mut i32 {
//         match s {
//             "x" => &mut self.x,
//             "y" => &mut self.y,
//             _ => panic!("unknown field: {}", s),
//         }
//     }
// }

impl std::ops::Add<f32> for HsvKeyValue {
    type Output = Self;

    fn add(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] + rhs, self.val[1] + rhs, self.val[2] + rhs],
        }
    }
}

impl std::ops::Add<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn add(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [rhs.val[0] + self, rhs.val[1] + self, rhs.val[2] + self],
        }
    }
}

impl std::ops::Add<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn add(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] + rhs.val[0],
                self.val[1] + rhs.val[1],
                self.val[2] + rhs.val[2],
            ],
        }
    }
}

impl std::ops::Sub<f32> for HsvKeyValue {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] - rhs, self.val[1] - rhs, self.val[2] - rhs],
        }
    }
}

impl std::ops::Sub<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn sub(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [self - rhs.val[0], self - rhs.val[1], self - rhs.val[2]],
        }
    }
}

impl std::ops::Sub<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn sub(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] - rhs.val[0],
                self.val[1] - rhs.val[1],
                self.val[2] - rhs.val[2],
            ],
        }
    }
}

impl std::ops::Mul<f32> for HsvKeyValue {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] * rhs, self.val[1] * rhs, self.val[2] * rhs],
        }
    }
}

impl std::ops::Mul<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn mul(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [self * rhs.val[0], self * rhs.val[1], self * rhs.val[2]],
        }
    }
}

impl std::ops::Mul<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn mul(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] * rhs.val[0],
                self.val[1] * rhs.val[1],
                self.val[2] * rhs.val[2],
            ],
        }
    }
}

impl std::ops::Div<f32> for HsvKeyValue {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::Output {
            val: [self.val[0] / rhs, self.val[1] / rhs, self.val[2] / rhs],
        }
    }
}

impl std::ops::Div<HsvKeyValue> for f32 {
    type Output = HsvKeyValue;

    fn div(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [self / rhs.val[0], self / rhs.val[1], self / rhs.val[2]],
        }
    }
}

impl std::ops::Div<HsvKeyValue> for HsvKeyValue {
    type Output = HsvKeyValue;

    fn div(self, rhs: HsvKeyValue) -> Self::Output {
        Self::Output {
            val: [
                self.val[0] / rhs.val[0],
                self.val[1] / rhs.val[1],
                self.val[2] / rhs.val[2],
            ],
        }
    }
}

impl splines::interpolate::Interpolate<f32> for HsvKeyValue {
    fn step(t: f32, threshold: f32, a: Self, b: Self) -> Self {
        if t < threshold {
            a
        } else {
            b
        }
    }

    fn cosine(t: f32, a: Self, b: Self) -> Self {
        let cos_nt = (1. - (t * std::f32::consts::PI).cos()) * 0.5;
        <Self as splines::interpolate::Interpolate<f32>>::lerp(cos_nt, a, b)
    }

    fn lerp(t: f32, a: Self, b: Self) -> Self {
        a * (1. - t) + b * t
    }

    fn cubic_hermite(
        t: f32,
        x: (f32, Self),
        a: (f32, Self),
        b: (f32, Self),
        y: (f32, Self),
    ) -> Self {
        // sampler stuff
        let two_t = t * 2.;
        let three_t = t * 3.;
        let t2 = t * t;
        let t3 = t2 * t;
        let two_t3 = t2 * two_t;
        let two_t2 = t * two_t;
        let three_t2 = t * three_t;

        // tangents
        let m0 = (b.1 - x.1) / (b.0 - x.0) * (b.0 - a.0);
        let m1 = (y.1 - a.1) / (y.0 - a.0) * (b.0 - a.0);

        a.1 * (two_t3 - three_t2 + 1.)
            + m0 * (t3 - two_t2 + t)
            + b.1 * (three_t2 - two_t3)
            + m1 * (t3 - t2)
    }

    fn quadratic_bezier(t: f32, a: Self, u: Self, b: Self) -> Self {
        let one_t = 1. - t;
        let one_t2 = one_t * one_t;

        u + (a - u) * one_t2 + (b - u) * t * t
    }

    fn cubic_bezier(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        let one_t = 1. - t;
        let one_t2 = one_t * one_t;
        let one_t3 = one_t2 * one_t;
        let t2 = t * t;

        a * one_t3 + (u * one_t2 * t + v * one_t * t2) * 3. + b * t2 * t
    }

    fn cubic_bezier_mirrored(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        <Self as splines::interpolate::Interpolate<f32>>::cubic_bezier(t, a, u, b + b - v, b)
    }
}
