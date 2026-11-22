//! Evidence for automatic confirmation's trust boundary: captured Agent screens, the path table,
//! and the per-run decision under a controlled clock.
use super::place::Scope;

mod places;
mod runs;
mod screens;
mod styled_screens;

const POSIX: Scope<'static> = Scope {
    root: Some("/work/app"),
    directory: Some("/work/app"),
};
const WINDOWS: Scope<'static> = Scope {
    root: Some(r"C:\work\app"),
    directory: Some(r"C:\work\app\crates"),
};
