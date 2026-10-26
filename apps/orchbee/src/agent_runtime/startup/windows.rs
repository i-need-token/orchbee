//! Agent launch is admitted without creating any native resource.
pub(crate) fn admit() -> Result<(), super::Unsupported> {
    Ok(())
}
