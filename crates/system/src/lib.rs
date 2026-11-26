//! doto-system: HardwareProbe, ResourceManager (profile Nhẹ/Cân bằng/Tối đa), admission control.
pub mod hardware;
pub mod resources;

pub use hardware::{HardwareInfo, HardwareProbe};
pub use resources::{compute_limits, HostCapacity, Profile, ResourceLimits, ResourceManager, ResourceOverrides, StoredResourceSettings, SystemSample};
