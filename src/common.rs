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
    Polynomial,
}

#[allow(unused_macros)]
macro_rules! offset_of {
    ($type:ty, $field:tt) => {{
        let dummy = ::core::mem::MaybeUninit::<$type>::uninit();

        let dummy_ptr = dummy.as_ptr();
        let member_ptr = ::core::ptr::addr_of!((*dummy_ptr).$field);
        member_ptr as usize - dummy_ptr as usize
    }};
}
