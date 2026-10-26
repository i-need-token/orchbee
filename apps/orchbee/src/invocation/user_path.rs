//! Inert path syntax accepts the existing Windows and absolute wire vocabularies.
//! The final execution adapter owns native expansion and supported path semantics.
use orchbee_local_site_runtime::{WindowsUserPathError, WindowsUserPathOperand};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParsedUserPath {
    Windows(WindowsUserPathOperand),
    Absolute(WorkbenchDirectoryPath),
}

impl ParsedUserPath {
    pub(crate) fn parse(value: &str) -> Result<Self, WindowsUserPathError> {
        WindowsUserPathOperand::parse(value)
            .map(Self::Windows)
            .or_else(|_| {
                WorkbenchDirectoryPath::parse(value)
                    .map(Self::Absolute)
                    .map_err(|_| WindowsUserPathError::Input)
            })
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Windows(path) => path.as_str(),
            Self::Absolute(path) => path.as_str(),
        }
    }
}
