use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Copy, Clone, Default, Serialize, Deserialize)]
pub enum ColorStringCopy {
    HEX,
    #[default]
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

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Default)]
pub enum SplineMode {
    Linear,
    Bezier,
    #[default]
    HermiteBezier,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_mode_json_rejects_polynomial() {
        assert!(serde_json::from_str::<SplineMode>("\"Polynomial\"").is_err());
        assert_eq!(
            serde_json::from_str::<SplineMode>("\"HermiteBezier\"").unwrap(),
            SplineMode::HermiteBezier
        );
    }
}
