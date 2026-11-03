use serde::{Deserialize, Serialize};

use super::hsv_key_value::HsvKeyValue;

pub fn create_tangent_for_control_point() -> ControlPointTangent {
    let hsv = ControlPointValue::new(0.0, 0.0, 0.0);
    ControlPointTangent { val: hsv.val }
}

pub type ControlPointValue = HsvKeyValue;
pub type ControlPointTangent = ControlPointValue;
pub type ControlPointTangents = [Option<ControlPointTangent>; 2];
pub type ControlPointT = f32;

#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ControlPointStorage {
    pub val: ControlPointValue,
    pub t: ControlPointT,
    pub tangents: ControlPointTangents,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curvature: Option<f32>,
}

impl ControlPointStorage {
    pub fn default() -> Self {
        Self {
            val: ControlPointValue::default(),
            t: 0.0,
            tangents: [None; 2],
            curvature: None,
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ControlPoint {
    ControlPointSimple(ControlPointStorage),
    ControlPointLeftRightTangent(ControlPointStorage),
}

impl ControlPoint {
    pub fn default() -> Self {
        Self::ControlPointSimple(ControlPointStorage::default())
    }

    pub fn new_simple(val: ControlPointValue, t: ControlPointT) -> Self {
        let mut default = ControlPointStorage::default();
        default.val = val;
        default.t = t;
        Self::ControlPointSimple(default)
    }

    pub fn storage(&self) -> &ControlPointStorage {
        match self {
            ControlPoint::ControlPointSimple(c) | ControlPoint::ControlPointLeftRightTangent(c) => {
                &c
            }
        }
    }
    pub fn storage_mut(&mut self) -> &mut ControlPointStorage {
        match self {
            ControlPoint::ControlPointLeftRightTangent(c) | ControlPoint::ControlPointSimple(c) => {
                c
            }
        }
    }

    pub fn val(&self) -> &ControlPointValue {
        &self.storage().val
    }
    pub fn val_mut(&mut self) -> &mut ControlPointValue {
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

    pub fn curvature(&self) -> Option<f32> {
        self.storage().curvature
    }
    pub fn curvature_mut(&mut self) -> &mut Option<f32> {
        &mut self.storage_mut().curvature
    }

    pub fn flip_tangents(&mut self) {
        self.tangents_mut().swap(0, 1);
    }
}

#[cfg(test)]
pub mod test_support {
    use super::{ControlPoint, ControlPointValue};

    pub const RUNAWAY_POINT: [f32; 3] = [0.035, 0.855, 0.566];
    pub const RUNAWAY_TANGENT: [f32; 3] = [161.96, 35.47, 0.0];

    pub fn point_with_tangent(val: [f32; 3], slot: usize, offset: [f32; 3]) -> ControlPoint {
        let mut cp = ControlPoint::new_simple(ControlPointValue::from(val), 0.0);
        cp.tangents_mut()[slot] = Some(ControlPointValue::from(offset));
        cp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(cp: &ControlPoint) -> String {
        let json = serde_json::to_string(cp).unwrap();
        assert_eq!(&serde_json::from_str::<ControlPoint>(&json).unwrap(), cp);
        json
    }

    #[test]
    fn simple_point_json_is_pinned() {
        let cp = ControlPoint::new_simple(ControlPointValue::new(0.25, 0.5, 0.75), 0.5);
        assert_eq!(
            round_trip(&cp),
            r#"{"ControlPointSimple":{"val":{"val":[0.25,0.5,0.75]},"t":0.5,"tangents":[null,null]}}"#
        );
    }

    #[test]
    fn left_right_tangent_point_json_round_trips() {
        let cp = ControlPoint::ControlPointLeftRightTangent(ControlPointStorage {
            val: ControlPointValue::new(0.5, 0.25, 0.125),
            t: 1.0,
            tangents: [None, Some(ControlPointTangent::new(0.125, -0.25, 0.0))],
            curvature: None,
        });
        let json = round_trip(&cp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let storage = parsed["ControlPointLeftRightTangent"].as_object().unwrap();
        let mut keys: Vec<_> = storage.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["t", "tangents", "val"]);
        assert_eq!(
            storage["tangents"][1]["val"],
            serde_json::json!([0.125, -0.25, 0.0])
        );
    }

    #[test]
    fn curvature_round_trips_and_json_without_it_loads_as_none() {
        let mut cp = ControlPoint::new_simple(ControlPointValue::new(0.25, 0.5, 0.75), 0.5);
        *cp.curvature_mut() = Some(-2.5);
        assert!(round_trip(&cp).contains(r#""curvature":-2.5"#));

        let old = r#"{"ControlPointSimple":{"val":{"val":[0.25,0.5,0.75]},"t":0.5,"tangents":[null,null]}}"#;
        let loaded: ControlPoint = serde_json::from_str(old).unwrap();
        assert_eq!(loaded.curvature(), None);
    }
}
