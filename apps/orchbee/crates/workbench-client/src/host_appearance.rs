//! Appearance facts observed from the outer host terminal; probing stays with
//! `terminal_host_appearance`.

/// The outer host's observed background polarity, independent of the System theme fallback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HostAppearance {
    Dark,
    Light,
    #[default]
    Unknown,
}

/// Colors captured from the outer terminal, never from a retained child frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostColors {
    pub(crate) foreground: Option<u32>,
    pub(crate) background: Option<u32>,
}
