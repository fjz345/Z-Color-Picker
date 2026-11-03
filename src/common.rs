use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Copy, Clone, Default, Serialize, Deserialize)]
pub enum ColorStringCopy {
    #[serde(rename = "HEX")]
    Hex,
    #[default]
    #[serde(rename = "HEXNOA")]
    HexNoAlpha,
    #[serde(rename = "SRGBHEX")]
    SrgbHex,
    #[serde(rename = "HSV")]
    Hsv,
    #[serde(rename = "HSVA")]
    Hsva,
    #[serde(rename = "INT")]
    Int,
    #[serde(rename = "FLOAT")]
    Float,
    #[serde(rename = "RGB")]
    Rgb,
    #[serde(rename = "SRGB")]
    Srgb,
    #[serde(rename = "RGBA")]
    Rgba,
    #[serde(rename = "SRGBA")]
    Srgba,
    #[serde(rename = "HSL")]
    Hsl,
    #[serde(rename = "OKLAB")]
    Oklab,
    #[serde(rename = "OKLCH")]
    Oklch,
}

impl ColorStringCopy {
    pub fn label(self) -> &'static str {
        match self {
            ColorStringCopy::Hex => "HEX",
            ColorStringCopy::HexNoAlpha => "HEXNOA",
            ColorStringCopy::SrgbHex => "SRGBHEX",
            ColorStringCopy::Hsv => "HSV",
            ColorStringCopy::Hsva => "HSVA",
            ColorStringCopy::Int => "INT",
            ColorStringCopy::Float => "FLOAT",
            ColorStringCopy::Rgb => "RGB",
            ColorStringCopy::Srgb => "SRGB",
            ColorStringCopy::Rgba => "RGBA",
            ColorStringCopy::Srgba => "SRGBA",
            ColorStringCopy::Hsl => "HSL",
            ColorStringCopy::Oklab => "OKLAB",
            ColorStringCopy::Oklch => "OKLCH",
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Default)]
pub enum SplineMode {
    Linear,
    Bezier,
    #[default]
    HermiteBezier,
}

impl SplineMode {
    pub const ALL: [SplineMode; 3] = [
        SplineMode::Linear,
        SplineMode::Bezier,
        SplineMode::HermiteBezier,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_format_json_name(f: ColorStringCopy) -> &'static str {
        match f {
            ColorStringCopy::Hex => "HEX",
            ColorStringCopy::HexNoAlpha => "HEXNOA",
            ColorStringCopy::SrgbHex => "SRGBHEX",
            ColorStringCopy::Hsv => "HSV",
            ColorStringCopy::Hsva => "HSVA",
            ColorStringCopy::Int => "INT",
            ColorStringCopy::Float => "FLOAT",
            ColorStringCopy::Rgb => "RGB",
            ColorStringCopy::Srgb => "SRGB",
            ColorStringCopy::Rgba => "RGBA",
            ColorStringCopy::Srgba => "SRGBA",
            ColorStringCopy::Hsl => "HSL",
            ColorStringCopy::Oklab => "OKLAB",
            ColorStringCopy::Oklch => "OKLCH",
        }
    }

    fn spline_mode_json_name(m: SplineMode) -> &'static str {
        match m {
            SplineMode::Linear => "Linear",
            SplineMode::Bezier => "Bezier",
            SplineMode::HermiteBezier => "HermiteBezier",
        }
    }

    #[test]
    fn copy_format_json_names_are_pinned() {
        let all = [
            ColorStringCopy::Hex,
            ColorStringCopy::HexNoAlpha,
            ColorStringCopy::SrgbHex,
            ColorStringCopy::Hsv,
            ColorStringCopy::Hsva,
            ColorStringCopy::Int,
            ColorStringCopy::Float,
            ColorStringCopy::Rgb,
            ColorStringCopy::Srgb,
            ColorStringCopy::Rgba,
            ColorStringCopy::Srgba,
            ColorStringCopy::Hsl,
            ColorStringCopy::Oklab,
            ColorStringCopy::Oklch,
        ];
        let distinct: std::collections::HashSet<_> =
            all.iter().map(|&f| copy_format_json_name(f)).collect();
        assert_eq!(distinct.len(), 14);

        for f in all {
            let json = format!("\"{}\"", copy_format_json_name(f));
            assert_eq!(serde_json::to_string(&f).unwrap(), json);
            assert_eq!(serde_json::from_str::<ColorStringCopy>(&json).unwrap(), f);
        }
    }

    #[test]
    fn spline_mode_json_names_are_pinned() {
        let distinct: std::collections::HashSet<_> = SplineMode::ALL
            .iter()
            .map(|&m| spline_mode_json_name(m))
            .collect();
        assert_eq!(distinct.len(), 3);

        for m in SplineMode::ALL {
            let json = format!("\"{}\"", spline_mode_json_name(m));
            assert_eq!(serde_json::to_string(&m).unwrap(), json);
            assert_eq!(serde_json::from_str::<SplineMode>(&json).unwrap(), m);
        }
    }

    #[test]
    fn copy_format_default_is_hexnoa() {
        assert_eq!(ColorStringCopy::default(), ColorStringCopy::HexNoAlpha);
    }

    #[test]
    fn spline_mode_default_is_hermite_bezier() {
        assert_eq!(SplineMode::default(), SplineMode::HermiteBezier);
    }

    #[test]
    fn spline_mode_json_rejects_polynomial() {
        assert!(serde_json::from_str::<SplineMode>("\"Polynomial\"").is_err());
        assert_eq!(
            serde_json::from_str::<SplineMode>("\"HermiteBezier\"").unwrap(),
            SplineMode::HermiteBezier
        );
    }
}
