use serde::{Deserialize, Serialize};

use crate::hsv_key_value::HsvKeyValue;

pub fn create_tangent_for_control_point() -> ControlPointTangent {
    let hsv = ControlPointType::new(0.0, 0.0, 0.0);
    ControlPointTangent { val: hsv.val }
}

pub type ControlPointType = HsvKeyValue;
pub type ControlPointTangent = ControlPointType;
pub type ControlPointTangents = [Option<ControlPointTangent>; 2];
pub type ControlPointT = f32;

#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlPointStorage {
    pub val: ControlPointType,
    pub t: ControlPointT,
    pub tangents: ControlPointTangents,
}

impl ControlPointStorage {
    pub fn default() -> Self {
        Self {
            val: ControlPointType::default(),
            t: 0.0,
            tangents: [None; 2],
        }
    }
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

#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ControlPoint {
    ControlPointSimple(ControlPointStorage),
    ControlPointLeftRightTangent(ControlPointStorage),
}

impl ControlPoint {
    pub fn default() -> Self {
        Self::ControlPointSimple(ControlPointStorage::default())
    }

    pub fn new_simple(val: ControlPointType, t: ControlPointT) -> Self {
        let mut default = ControlPointStorage::default();
        default.val = val;
        default.t = t;
        Self::ControlPointSimple(default)
    }

    pub fn new(val: ControlPointType, t: ControlPointT, tangents: ControlPointTangents) -> Self {
        let mut default = ControlPointStorage::default();
        default.val = val;
        default.t = t;
        default.tangents = tangents;
        Self::ControlPointLeftRightTangent(default)
    }

    pub fn storage(&self) -> &ControlPointStorage {
        match self {
            ControlPoint::ControlPointSimple(c)
            | ControlPoint::ControlPointLeftRightTangent(c)
            | ControlPoint::ControlPointSimple(c) => &c,
        }
    }
    pub fn storage_mut(&mut self) -> &mut ControlPointStorage {
        match self {
            ControlPoint::ControlPointLeftRightTangent(c) | ControlPoint::ControlPointSimple(c) => {
                c
            }
        }
    }

    pub fn val(&self) -> &ControlPointType {
        &self.storage().val
    }
    pub fn val_mut(&mut self) -> &mut ControlPointType {
        &mut self.storage_mut().val
    }

    pub fn t(&self) -> &ControlPointT {
        &self.storage().t
    }

    pub fn t_mut(&mut self) -> &mut ControlPointT {
        &mut self.storage_mut().t
    }

    pub fn tangents(&self) -> &ControlPointTangents {
        &self.storage().tangents
    }
    pub fn tangents_mut(&mut self) -> &mut ControlPointTangents {
        &mut self.storage_mut().tangents
    }

    pub fn flip_tangents(&mut self) {
        self.tangents_mut().swap(0, 1);
    }
}
