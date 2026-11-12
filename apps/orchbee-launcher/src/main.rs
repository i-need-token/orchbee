//! The Windows launcher, installed by the PATH step as `bin\<name>.exe` in the release store. It
//! reads the store's `active.json` and runs `versions\<product>\<name>.exe` with the unchanged
//! command line, console, standard handles and environment, ends that Client when the launcher
//! itself is ended, and returns its exit code. It reads nothing else.
mod localization;
#[cfg(windows)]
mod resolve;
#[cfg(windows)]
mod windows;

/// Exit code `1`: a local failure before any Client started, under `docs/cli/output.md`.
#[cfg(windows)]
const LOCAL_FAILURE: i32 = 1;

#[cfg(windows)]
fn main() {
    let code = match windows::launch() {
        Ok(code) => code,
        Err(failure) => {
            let locale = localization::locale(|name| std::env::var_os(name));
            eprintln!("{}", localization::failure(locale, &failure));
            LOCAL_FAILURE
        }
    };
    std::process::exit(code);
}

/// Exit code `4`: this platform cannot serve the request.
#[cfg(not(windows))]
fn main() {
    let locale = localization::locale(|name| std::env::var_os(name));
    eprintln!("{}", localization::unsupported_platform(locale));
    std::process::exit(4);
}
