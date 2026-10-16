//! The habitat rule type. `build.rs` includes this file by path, so keep it
//! free of dependencies (no serde).

/// One rule with block names resolved to ids.
#[derive(Clone, Debug, PartialEq)]
pub struct HabitatRule {
    pub id: String,
    pub frozen: Option<bool>,
    pub temp_min: Option<f64>,
    pub temp_max: Option<f64>,
    pub depth_min: Option<u32>,
    pub depth_max: Option<u32>,
    pub shore_distance_min: Option<u32>,
    pub shore_distance_max: Option<u32>,
    pub bottom: Option<Vec<u8>>,
}

impl HabitatRule {
    pub fn matches_everything(&self) -> bool {
        self.frozen.is_none()
            && self.temp_min.is_none()
            && self.temp_max.is_none()
            && self.depth_min.is_none()
            && self.depth_max.is_none()
            && self.shore_distance_min.is_none()
            && self.shore_distance_max.is_none()
            && self.bottom.is_none()
    }
}
