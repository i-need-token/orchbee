//! Host-independent admission of the declared Windows user-path grammar; resolution uses only caller-supplied context.
mod windows;
pub use windows::{USER_PATH_LIMIT, UserPathError, UserPathOperand, normalize_user_path};
