//! Production connection/file-root traces; every fixture joins consumers and reaps its private runtime.
use super::*;
use orchbee_local_site_runtime::WindowsPrivateCache;
use std::{fs, os::windows::fs::OpenOptionsExt, path::PathBuf};

struct Runtime {
    path: PathBuf,
    cache: Option<WindowsPrivateCache>,
}
impl Runtime {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("orchbee-graphics-files-{nonce:032x}"));
        let cache = WindowsPrivateCache::acquire(&path).unwrap();
        Self {
            path,
            cache: Some(cache),
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cache.take();
        let path = self.path.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-graphics-files-")
        );
        assert!(!fs::read_dir(&path).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("graphics-")
        }));
        fs::remove_dir_all(path).unwrap();
    }
}
fn open(target: &PaneGraphicsTarget, stream: WorkbenchRequestId) -> Action {
    Action::StreamOpen {
        stream_id: stream,
        target: target.clone(),
        layer_id: "live-file".into(),
        z_index: 1,
    }
}
fn file(stream: WorkbenchRequestId, sequence: u64, path: &std::path::Path) -> Action {
    Action::StreamFrame {
        stream_id: stream,
        sequence: WorkbenchRevision::new(sequence),
        revision: WorkbenchRevision::new(u64::MAX),
        image: PaneGraphicsStreamImage::File {
            format: PaneGraphicsFormat::Bgra,
            width: 1,
            height: 1,
            path: path.to_str().unwrap().into(),
            placement: PaneGraphicsPlacement::default(),
        },
        damage: vec![],
    }
}

#[test]
fn graphics_file_frames_actual_connections_keep_private_roots_complete_pixels_sequence_and_retryable_cleanup()
 {
    let runtime = Runtime::new();
    let fixture = Fixture::with_graphics_runtime(&runtime.path);
    let selected = target(&fixture);
    let service = service(&fixture, true);
    let writer = service.client_connection(NonZeroU64::MIN);
    let foreign = service.client_connection(NonZeroU64::new(2).unwrap());
    execute(&writer, &fixture, set(&selected));
    let stream = id(30001);
    let opened = execute(&writer, &fixture, open(&selected, stream.clone()));
    let Outcome::StreamOpened {
        file_frame_directory: Some(ref directory),
        ..
    } = opened
    else {
        panic!("private stream root");
    };
    let root = PathBuf::from(directory);
    assert_eq!(root.parent(), Some(runtime.path.as_path()));
    assert!(root.is_dir());
    assert_eq!(
        execute(&writer, &fixture, open(&selected, stream.clone())),
        opened
    );
    assert!(
        matches!(execute(&writer, &fixture, Action::Info { target: selected.clone(), client_id: None }), Outcome::Info { file_frame_directory: Some(ref own), .. } if own == directory)
    );
    assert!(matches!(
        execute(
            &foreign,
            &fixture,
            Action::Info {
                target: selected.clone(),
                client_id: None
            }
        ),
        Outcome::Info {
            file_frame_directory: None,
            ..
        }
    ));
    let path = root.join("帧.bgra");
    fs::write(&path, [255, 10, 0, 128]).unwrap();
    assert_eq!(
        execute(&foreign, &fixture, file(stream.clone(), 1, &path)),
        Outcome::Failed {
            reason: Failure::StreamClosed
        }
    );
    let applied = execute(&writer, &fixture, file(stream.clone(), 1, &path));
    assert!(
        matches!(applied, Outcome::FrameApplied { frame_revision, sequence, .. } if frame_revision.value() == u64::MAX && sequence.value() == 1)
    );
    let Outcome::Catalog { layers, .. } = execute(
        &writer,
        &fixture,
        Action::Catalog {
            target: selected.clone(),
        },
    ) else {
        panic!("catalog");
    };
    let layer = layers
        .iter()
        .find(|layer| layer.layer_id == "live-file")
        .unwrap();
    let pixels = execute(
        &writer,
        &fixture,
        Action::Pixels {
            target: selected.clone(),
            layer_id: layer.layer_id.clone(),
            resource_revision: layer.resource_revision,
            offset: 0,
            maximum_bytes: 4,
        },
    );
    let Outcome::Pixels {
        data_base64,
        complete: true,
        ..
    } = pixels
    else {
        panic!("whole pixels");
    };
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .unwrap(),
        [0, 10, 255, 128]
    );
    assert_eq!(
        execute(
            &writer,
            &fixture,
            file(stream.clone(), 2, &runtime.path.join("outside.bgra"))
        ),
        Outcome::Failed {
            reason: Failure::InvalidFile
        }
    );
    fs::write(&path, [1, 2, 3]).unwrap();
    assert_eq!(
        execute(&writer, &fixture, file(stream.clone(), 2, &path)),
        Outcome::Failed {
            reason: Failure::InvalidFile
        }
    );
    fs::write(&path, [1, 2, 3, 255]).unwrap();
    assert!(matches!(
        execute(&writer, &fixture, file(stream.clone(), 2, &path)),
        Outcome::FrameApplied { .. }
    ));
    assert_eq!(
        execute(&writer, &fixture, file(stream.clone(), 2, &path)),
        Outcome::Failed {
            reason: Failure::InvalidSequence
        }
    );
    let blocker = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert!(matches!(
        execute(
            &writer,
            &fixture,
            Action::StreamClose {
                stream_id: stream.clone()
            }
        ),
        Outcome::Closed { .. }
    ));
    assert!(fixture.service.graphics_maintenance_turn().is_some());
    assert!(root.join(".orchbee-frame-owner.json").is_file());
    assert_eq!(
        execute(&writer, &fixture, file(stream, 3, &path)),
        Outcome::Failed {
            reason: Failure::StreamClosed
        }
    );
    drop(blocker);
    assert!(fixture.service.graphics_maintenance_turn().is_none());
    assert!(!root.exists());
    fixture.service.start_close_maintenance().unwrap();
    let Outcome::StreamOpened {
        file_frame_directory: Some(second),
        ..
    } = execute(&writer, &fixture, open(&selected, id(30002)))
    else {
        panic!("new root");
    };
    assert_ne!(second, directory.as_str());
    drop(writer);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::path::Path::new(&second).exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!std::path::Path::new(&second).exists());
    assert!(
        matches!(execute(&foreign, &fixture, Action::Catalog { target: selected }), Outcome::Catalog { layers, .. } if layers.len() == 1 && layers[0].layer_id == "static")
    );
    assert!(fixture.barrier().unwrap().iter().all(|effect| !matches!(
        effect,
        Effect::Attach(..) | Effect::Input(..) | Effect::Resize(..)
    )));
}

#[test]
fn graphics_file_preparation_token_prevents_early_collection_and_bad_runtime_preserves_static_image()
 {
    let runtime = Runtime::new();
    let fixture = Fixture::with_graphics_runtime(&runtime.path);
    let selected = target(&fixture);
    let site = read_snapshot(&fixture.service).site_id;
    let source = fixture.service.graphics_source(&site, &selected).unwrap();
    let lease = fixture
        .service
        .graphics_files
        .as_ref()
        .unwrap()
        .create(&site, &source, NonZeroU64::MIN, &id(31000))
        .unwrap();
    let directory = lease.directory().unwrap().to_owned();
    assert!(fixture.service.graphics_maintenance_turn().is_some());
    assert!(directory.exists());
    drop(lease);
    assert!(fixture.service.graphics_maintenance_turn().is_none());
    assert!(!directory.exists());
    drop(fixture);
    let missing = runtime.path.join("absent");
    let fixture = Fixture::with_graphics_runtime(&missing);
    let selected = target(&fixture);
    let service = service(&fixture, true);
    let writer = service.client_connection(NonZeroU64::MIN);
    execute(&writer, &fixture, set(&selected));
    assert_eq!(
        execute(&writer, &fixture, open(&selected, id(31001))),
        Outcome::Failed {
            reason: Failure::Unavailable
        }
    );
    assert!(!missing.exists());
    assert!(
        matches!(execute(&writer, &fixture, Action::Catalog { target: selected }), Outcome::Catalog { layers, .. } if layers.len() == 1 && layers[0].layer_id == "static")
    );
}

#[test]
fn graphics_file_root_capacity_counts_uncommitted_leases_and_shutdown_requires_the_last_consumer() {
    let runtime = Runtime::new();
    let fixture = Fixture::with_graphics_runtime(&runtime.path);
    let selected = target(&fixture);
    let site = read_snapshot(&fixture.service).site_id;
    let source = fixture.service.graphics_source(&site, &selected).unwrap();
    let files = fixture.service.graphics_files.as_ref().unwrap();
    let mut leases = (0..128)
        .map(|n| {
            files
                .create(&site, &source, NonZeroU64::MIN, &id(32000 + n))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        files.create(&site, &source, NonZeroU64::MIN, &id(33000)),
        Err(Failure::Busy)
    ));
    let first = leases.pop().unwrap().directory().unwrap().to_owned();
    assert!(fixture.service.graphics_maintenance_turn().is_some());
    assert!(!first.exists());
    leases.push(
        files
            .create(&site, &source, NonZeroU64::MIN, &id(33001))
            .unwrap(),
    );
    assert_eq!(
        fixture
            .service
            .stop_graphics_before(std::time::Instant::now())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::TimedOut
    );
    drop(leases);
    fixture
        .service
        .stop_graphics_before(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .unwrap();
    let service = service(&fixture, true);
    let writer = service.client_connection(NonZeroU64::MIN);
    assert_eq!(
        execute(&writer, &fixture, open(&selected, id(33002))),
        Outcome::Failed {
            reason: Failure::Unavailable
        }
    );
}

#[test]
fn graphics_file_root_retires_after_actual_canonical_tab_removal_before_old_stream_can_read() {
    let runtime = Runtime::new();
    let fixture = Fixture::with_graphics_runtime(&runtime.path);
    let selected = target(&fixture);
    let service = service(&fixture, true);
    let writer = service.client_connection(NonZeroU64::MIN);
    let stream = id(34000);
    let Outcome::StreamOpened {
        file_frame_directory: Some(directory),
        ..
    } = execute(&writer, &fixture, open(&selected, stream.clone()))
    else {
        panic!("private root");
    };
    let path = PathBuf::from(&directory).join("frame.bgra");
    fs::write(&path, [1, 2, 3, 255]).unwrap();
    fixture.dispose(0);
    // Maintenance reads retained identities without pruning topology; no graphics read causes retirement.
    assert!(fixture.service.graphics_maintenance_turn().is_none());
    assert!(!PathBuf::from(&directory).exists());
    assert_eq!(
        execute(&writer, &fixture, file(stream, 1, &path)),
        Outcome::Failed {
            reason: Failure::StreamClosed
        }
    );
}
