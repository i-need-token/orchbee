//! Shared resource limits for one Windows child-environment projection.

/// Both registry scopes combined. A normal interactive environment is far
/// below this, while the bound keeps enumeration and projection work finite.
pub const registry_values: usize = 256;
/// Stored WTF-8 names and values, including their entry separators.
pub const registry_snapshot_bytes: usize = 512 * 1024;
pub const registry_name_code_units: usize = 32 * 1024;
pub const registry_name_wtf8_bytes: usize = 3 * registry_name_code_units;
pub const registry_value_bytes: usize = 256 * 1024;
pub const expanded_value_bytes: usize = 256 * 1024;
pub const expansion_work_units: usize = 64 * 1024;
pub const path_segments: usize = 4096;
pub const projected_environment_entries: usize = 1536;
pub const projected_environment_bytes: usize = 1024 * 1024;
