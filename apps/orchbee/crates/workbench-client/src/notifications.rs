//! Notification delivery around the Client-local card state in `client_view::notifications`: the
//! card painter in `paint`, the Client Connection transport in `subscription`, platform effects in
//! `system`, and the inherited terminal's notification host in `terminal`.
pub(crate) mod paint;
pub(crate) mod subscription;
pub(crate) mod system;
pub(crate) mod terminal;

#[cfg(test)]
mod tests;
