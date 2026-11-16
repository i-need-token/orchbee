//! Host-independent admission of the declared Windows and POSIX user-path grammars; resolution
//! uses only caller-supplied context.
mod posix;
mod windows;
pub use posix::normalize_posix_user_path;
pub use windows::{USER_PATH_LIMIT, UserPathError, UserPathOperand, normalize_user_path};

/// The grammar a typed local folder is resolved in. Pure callers hold it as data, so either
/// grammar can be exercised on any host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPathGrammar {
    /// [`normalize_user_path`].
    Windows,
    /// [`normalize_posix_user_path`].
    Posix,
}

impl UserPathGrammar {
    /// Resolves `value` in this grammar from the supplied context only.
    ///
    /// # Errors
    /// Returns the grammar's `Input` or `Context` error.
    pub fn normalize(
        self,
        value: &str,
        environment: impl FnMut(&str) -> Option<std::ffi::OsString>,
        cwd: impl FnOnce() -> std::io::Result<std::path::PathBuf>,
    ) -> Result<String, UserPathError> {
        match self {
            Self::Windows => normalize_user_path(value, environment, cwd),
            Self::Posix => normalize_posix_user_path(value, environment, cwd),
        }
    }
}
