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
            ColorStringCopy::HEX => "HEX",
            ColorStringCopy::HEXNOA => "HEXNOA",
            ColorStringCopy::SRGBHEX => "SRGBHEX",
            ColorStringCopy::HSV => "HSV",
            ColorStringCopy::HSVA => "HSVA",
            ColorStringCopy::INT => "INT",
            ColorStringCopy::FLOAT => "FLOAT",
            ColorStringCopy::RGB => "RGB",
            ColorStringCopy::SRGB => "SRGB",
            ColorStringCopy::RGBA => "RGBA",
            ColorStringCopy::SRGBA => "SRGBA",
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
            ColorStringCopy::HEX,
            ColorStringCopy::HEXNOA,
            ColorStringCopy::SRGBHEX,
            ColorStringCopy::HSV,
            ColorStringCopy::HSVA,
            ColorStringCopy::INT,
            ColorStringCopy::FLOAT,
            ColorStringCopy::RGB,
            ColorStringCopy::SRGB,
            ColorStringCopy::RGBA,
            ColorStringCopy::SRGBA,
        ];
        let distinct: std::collections::HashSet<_> =
            all.iter().map(|&f| copy_format_json_name(f)).collect();
        assert_eq!(distinct.len(), 11);

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
        assert_eq!(ColorStringCopy::default(), ColorStringCopy::HEXNOA);
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
