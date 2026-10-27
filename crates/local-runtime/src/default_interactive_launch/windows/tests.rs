use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchbee-shell-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, b"resolver fixture, never executed").unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-shell-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

fn image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 130];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    for (offset, value) in [(68, machine), (70, 1), (84, 2), (86, 2), (88, 0x020b)] {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[test]
fn automatic_powershell_skips_bad_candidates_and_ignores_comspec() {
    let first = Fixture::new();
    let second = Fixture::new();
    let bad = first.file("pwsh.exe");
    let good = second.file("pwsh.exe");
    fs::write(&good, image(0x8664)).unwrap();
    let path = std::env::join_paths([Path::new("relative"), first.0.as_path(), second.0.as_path()])
        .unwrap();
    let env = |name: &str| match name {
        "PATH" => Some(path.clone()),
        "SystemRoot" => std::env::var_os("SystemRoot"),
        "COMSPEC" | "SHELL" => Some(bad.clone().into_os_string()),
        _ => None,
    };
    assert_eq!(
        Path::new(
            &select_default_interactive_launch(usable_shell_candidates(&env, 0x8664))
                .unwrap()
                .executable
        ),
        good
    );
    fs::write(&bad, image(0x014c)).unwrap();
    assert_eq!(
        Path::new(
            &select_default_interactive_launch(usable_shell_candidates(&env, 0x8664))
                .unwrap()
                .executable
        ),
        bad
    );
    fs::write(&bad, image(0xaa64)).unwrap();
    fs::write(&good, b"invalid").unwrap();
    let fallback =
        select_default_interactive_launch(usable_shell_candidates(&env, 0x8664)).unwrap();
    assert!(fallback.executable.ends_with("powershell.exe"));
    assert!(
        select_default_interactive_launch(usable_shell_candidates(
            &|name| (name == "COMSPEC").then(|| bad.clone().into_os_string()),
            0x8664
        ))
        .is_err()
    );
}

#[test]
fn pe_candidate_admission_checks_header_bounds_flags_and_architectures() {
    let fixture = Fixture::new();
    let path = fixture.file("pwsh.exe");
    for host in [0x014c, 0x8664, 0xaa64, 0] {
        for machine in [0x014c, 0x8664, 0xaa64, 0x9999] {
            fs::write(&path, image(machine)).unwrap();
            let expected = machine == 0x014c
                || (machine == 0x8664 && host != 0x014c)
                || (machine == 0xaa64 && host == 0xaa64);
            assert_eq!(
                pe::compatible_image(&path, host),
                expected,
                "{host:x}/{machine:x}"
            );
        }
    }
    for (offset, value) in [
        (0, 0),
        (64, 0),
        (70, 0),
        (70, 2),
        (84, 1),
        (84, 3),
        (86, 0),
        (86, 0x2002),
        (88, 0),
    ] {
        let mut bytes = image(0x8664);
        bytes[offset..offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(!pe::compatible_image(&path, 0x8664), "{offset}/{value}");
    }
    for offset in [0, 63, 107, u32::MAX] {
        let mut bytes = image(0x8664);
        bytes[60..64].copy_from_slice(&offset.to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(!pe::compatible_image(&path, 0x8664));
    }
    for length in [0, 63, 64, 87, 89, 129, 130] {
        fs::write(&path, &image(0x8664)[..length]).unwrap();
        assert_eq!(pe::compatible_image(&path, 0x8664), length == 130);
    }
    assert!(!pe::compatible_image(&fixture.0, 0x8664));
}

#[test]
fn automatic_powershell_skips_packaged_images() {
    for nested in [false, true] {
        let package = Fixture::new();
        let ordinary = Fixture::new();
        package.file("AppxManifest.xml");
        fs::create_dir(ordinary.0.join("AppxManifest.xml")).unwrap();
        if nested {
            fs::create_dir(package.0.join("bin")).unwrap();
        }
        let packaged = package.file(if nested { "bin/pwsh.exe" } else { "pwsh.exe" });
        let unpackaged = ordinary.file("pwsh.exe");
        for executable in [&packaged, &unpackaged] {
            fs::write(executable, image(native_machine())).unwrap();
        }
        let directory = packaged.parent().unwrap();
        let search = std::env::join_paths([directory, ordinary.0.as_path()]).unwrap();
        let environment = |name: &str| match name {
            "PATH" => Some(search.clone()),
            "SystemRoot" => std::env::var_os("SystemRoot"),
            _ => None,
        };
        let automatic = resolve(environment).unwrap();
        assert_eq!(Path::new(&automatic.executable), unpackaged);
        let only_packaged = std::env::join_paths([directory]).unwrap();
        let automatic = resolve(|name| match name {
            "PATH" => Some(only_packaged.clone()),
            "SystemRoot" => std::env::var_os("SystemRoot"),
            _ => None,
        })
        .unwrap();
        let fallback = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        assert_eq!(Path::new(&automatic.executable), fallback);
    }
}
