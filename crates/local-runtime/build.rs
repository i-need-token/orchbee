//! Resolves this build's environment stem (ORC-382) on every target. Staging names one through
//! `ORCHBEE_ARTIFACT_STEM`; any other build is this checkout's test environment,
//! `ob-test-<checkout digest>`, so only a build that names `orchbee` is release. A stem that carries
//! this checkout's digest also names the checkout, so the TUI can mark which checkout built it.
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

const STEM_VARIABLE: &str = "ORCHBEE_ARTIFACT_STEM";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={STEM_VARIABLE}");
    let checkout = canonical_checkout();
    let digest = checkout_digest(&checkout);
    let stem = match env::var(STEM_VARIABLE) {
        Ok(stem) => stem,
        Err(env::VarError::NotPresent) => format!("ob-test-{digest}"),
        Err(env::VarError::NotUnicode(_)) => panic!("{STEM_VARIABLE} must be Unicode"),
    };
    println!("cargo:rustc-env=ORCHBEE_ENVIRONMENT_STEM={stem}");
    // Release and the packaged test build, whose digest names a home, carry no checkout name.
    let from_checkout = ["ob-dev-", "ob-test-"]
        .iter()
        .any(|prefix| stem.strip_prefix(prefix) == Some(digest.as_str()));
    let name = from_checkout
        .then(|| checkout_name(&checkout))
        .flatten()
        .unwrap_or_default();
    println!("cargo:rustc-env=ORCHBEE_CHECKOUT_NAME={name}");
}

/// The checkout's final directory name, unless it holds a control character that a compiled
/// string could not carry as written.
fn checkout_name(checkout: &str) -> Option<String> {
    let name = Path::new(checkout).file_name()?.to_str()?;
    (!name.is_empty() && !name.chars().any(char::is_control)).then(|| name.to_owned())
}

/// The checkout's canonical path as `realpathSync.native` reports it: without the verbatim prefix,
/// and with a lowercase drive letter on Windows.
fn canonical_checkout() -> String {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let checkout = Path::new(&manifest).join("../..");
    let canonical = fs::canonicalize(&checkout)
        .unwrap_or_else(|error| panic!("canonicalize {}: {error}", checkout.display()));
    let canonical = canonical
        .to_str()
        .expect("the checkout path must be Unicode");
    let canonical = match canonical.strip_prefix(r"\\?\") {
        Some(unc) if unc.starts_with(r"UNC\") => format!(r"\{}", &unc[3..]),
        Some(local) => local.to_owned(),
        None => canonical.to_owned(),
    };
    match canonical.as_bytes() {
        [drive @ b'A'..=b'Z', b':', ..] => {
            format!("{}{}", drive.to_ascii_lowercase() as char, &canonical[1..])
        }
        _ => canonical,
    }
}

/// The checkout digest that `createDesktopDevelopmentScope` in
/// `apps/desktop/src/distribution/desktop-distribution-profile.ts` defines: the first 16 hex
/// digits of SHA-256 over `orchbee-development-data-root-v1\0` and the checkout's canonical path,
/// with a lowercase drive letter on Windows. Node also applies NFC, which changes only a path
/// stored in decomposed form. The build host's path is hashed, so a cross-compiled build carries
/// the digest of the checkout it was built from.
fn checkout_digest(canonical: &str) -> String {
    let digest = Sha256::new()
        .chain_update(b"orchbee-development-data-root-v1\0")
        .chain_update(canonical.as_bytes())
        .finalize();
    format!("{digest:x}")[..16].to_owned()
}
