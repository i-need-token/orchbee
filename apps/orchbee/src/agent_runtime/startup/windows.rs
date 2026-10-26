//! Agent launch is admitted without creating any native resource.
/// Admit the existing Windows launch implementation without allocating or observing resources.
///
/// # Errors
/// This adapter always admits; the shared signature also represents unsupported hosts.
pub(crate) fn admit() -> Result<(), super::Unsupported> {
    Ok(())
}
