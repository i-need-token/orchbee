use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

/// An owned folder of empty files laid out like Git installations and other `PATH` folders.
struct Layout(PathBuf);

impl Layout {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchbee-git-launcher-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn file(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"").unwrap();
        path
    }

    fn target(&self, folders: &[&str]) -> Option<PathBuf> {
        launcher_target(folders.iter().map(|folder| self.0.join(folder)).collect())
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn only_a_first_match_launcher_with_one_installation_git_is_replaced() {
    let layout = Layout::new();
    let x64 = layout.file(r"x64\mingw64\bin\git.exe");
    layout.file(r"x64\cmd\git.exe");
    layout.file(r"x64\bin\git.exe");
    let arm = layout.file(r"arm\clangarm64\bin\git.exe");
    layout.file(r"arm\cmd\git.exe");
    layout.file(r"plain\git.exe");
    layout.file(r"bare\cmd\git.exe");
    layout.file(r"both\cmd\git.exe");
    layout.file(r"both\mingw64\bin\git.exe");
    layout.file(r"both\clangarm64\bin\git.exe");

    // Folders without a `git.exe` are passed over, as process creation passes them.
    assert_eq!(layout.target(&["missing", r"x64\cmd"]), Some(x64.clone()));
    assert_eq!(layout.target(&[r"x64\bin"]), Some(x64));
    assert_eq!(layout.target(&[r"arm\cmd"]), Some(arm));
    // The first match decides: an earlier Git, such as one in the Worker's or the system folder,
    // the installation's own Git or a launcher without exactly one Git to start keeps the name.
    assert_eq!(layout.target(&["plain", r"x64\cmd"]), None);
    assert_eq!(layout.target(&[r"x64\mingw64\bin", r"x64\cmd"]), None);
    assert_eq!(layout.target(&[r"bare\cmd", r"x64\cmd"]), None);
    assert_eq!(layout.target(&[r"both\cmd"]), None);
    assert_eq!(layout.target(&["missing"]), None);
    assert_eq!(layout.target(&[]), None);
}
