#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(any(target_os = "linux", windows))]
fn observation() -> super::fs::Observation {
    super::fs::Observation::admit().expect("supported native doctor fixture")
}
