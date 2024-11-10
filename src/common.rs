use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Copy, Clone, Default)]
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

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum SplineMode {
    Linear,
    Bezier,
    HermiteBezier,
    Polynomial,
}
