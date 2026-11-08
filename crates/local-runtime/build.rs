//! Resolves this build's Windows environment stem (ORC-382). Staging names one through
//! `ORCHBEE_ARTIFACT_STEM`; any other build is this checkout's test environment,
//! `ob-test-<checkout digest>`, so only a build that names `orchbee` carries the release names.
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

const STEM_VARIABLE: &str = "ORCHBEE_ARTIFACT_STEM";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={STEM_VARIABLE}");
    // POSIX cohorts keep the shipped names until ORC-328.
    if env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    let stem = match env::var(STEM_VARIABLE) {
        Ok(stem) => stem,
        Err(env::VarError::NotPresent) => format!("ob-test-{}", checkout_digest()),
        Err(env::VarError::NotUnicode(_)) => panic!("{STEM_VARIABLE} must be Unicode"),
    };
    println!("cargo:rustc-env=ORCHBEE_ENVIRONMENT_STEM={stem}");
}

/// The checkout digest that `createDesktopDevelopmentScope` in
/// `apps/desktop/src/distribution/desktop-distribution-profile.ts` defines: the first 16 hex
/// digits of SHA-256 over `orchbee-development-data-root-v1\0` and the checkout's canonical path
/// with a lowercase drive letter. Node also applies NFC, which changes only a path stored in
/// decomposed form.
fn checkout_digest() -> String {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let checkout = Path::new(&manifest).join("../..");
    let canonical = fs::canonicalize(&checkout)
        .unwrap_or_else(|error| panic!("canonicalize {}: {error}", checkout.display()));
    let canonical = canonical
        .to_str()
        .expect("the checkout path must be Unicode");
    // `realpathSync.native` reports the same final path without the verbatim prefix.
    let canonical = match canonical.strip_prefix(r"\\?\") {
        Some(unc) if unc.starts_with(r"UNC\") => format!(r"\{}", &unc[3..]),
        Some(local) => local.to_owned(),
        None => canonical.to_owned(),
    };
    let canonical = match canonical.as_bytes() {
        [drive @ b'A'..=b'Z', b':', ..] => {
            format!("{}{}", drive.to_ascii_lowercase() as char, &canonical[1..])
        }
        _ => canonical,
    };
    let digest = Sha256::new()
        .chain_update(b"orchbee-development-data-root-v1\0")
        .chain_update(canonical.as_bytes())
        .finalize();
    format!("{digest:x}")[..16].to_owned()
}
