/// Generation 1's acknowledgement, whose bytes never change: the independent reference the
/// transport proofs compare the endpoint's reply against.
const ACK: &[u8] = b"{\"accepted\":true,\"version\":1}
";

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
