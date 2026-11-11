use super::{
    LINUX_SOCKET_PARENT, MACOS_SOCKET_PARENT, SocketRole, endpoint_id, socket_leaf, socket_root,
};

/// The longest socket path any environment derives must fit the smaller socket address limit,
/// macOS's 103 usable bytes (Linux allows 107), whatever the home folder or Session name.
#[test]
fn every_socket_path_fits_the_macos_address_limit() {
    let digest = "f".repeat(16);
    let stems = [
        "orchbee".to_owned(),
        format!("ob-dev-{digest}"),
        format!("ob-test-{digest}"),
    ];
    let id = endpoint_id(&[b'a'; 4096], &"S".repeat(64));
    let mut longest = 0;
    for stem in &stems {
        assert!(crate::environment::valid_environment_stem(stem), "{stem}");
        for parent in [LINUX_SOCKET_PARENT, MACOS_SOCKET_PARENT] {
            for role in [
                SocketRole::Client,
                SocketRole::Pending,
                SocketRole::Worker,
                SocketRole::Host,
                SocketRole::Agent,
            ] {
                let path = format!(
                    "{}/{}",
                    socket_root(parent, stem, u32::MAX),
                    socket_leaf(&id, role)
                );
                longest = longest.max(path.len());
            }
        }
    }
    assert!(longest <= 103, "{longest}");
}

/// A Session's sockets share one name however its folder is spelled, and two runtimes never do.
#[test]
fn endpoint_ids_ignore_session_case_and_separate_runtimes() {
    let id = endpoint_id(b"/home/a/.orchbee/internal/runtime", "Work");
    assert_eq!(id.len(), 32);
    assert!(
        id.bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    assert_eq!(
        id,
        endpoint_id(b"/home/a/.orchbee/internal/runtime", "work")
    );
    assert_ne!(
        id,
        endpoint_id(b"/home/a/.ob-dev-0123456789abcdef/internal/runtime", "work")
    );
    assert_ne!(
        id,
        endpoint_id(b"/home/a/.orchbee/internal/runtime", "other")
    );
}
