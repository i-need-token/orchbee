//! Controlled Windows rename contention at the managed publication boundary.
use super::*;
use crate::tests::windows_adapters::Fixture;

#[test]
fn directory_publication_settles_a_locked_member_without_replacing_an_arrival() {
    for arrival in [false, true] {
        let fixture = Fixture::new();
        let source = fixture.0.join("staged");
        let target = fixture.0.join("published");
        fs::create_dir(&source).unwrap();
        let member = source.join("member");
        fs::write(&member, b"owned payload").unwrap();
        let mut reader = Some(
            OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&member)
                .unwrap(),
        );
        // A descendant opened without delete sharing produces the same native
        // directory-rename error as a short-lived external reader.
        assert_eq!(
            winsafe::MoveFile(&win32_path(&source).unwrap(), &win32_path(&target).unwrap()),
            Err(winsafe::co::ERROR::ACCESS_DENIED)
        );
        let mut waits = 0;
        let result = move_no_replace_with_wait(&source, &target, |_| {
            waits += 1;
            if let Some(file) = reader.take() {
                drop(file);
                if arrival {
                    fs::create_dir(&target).unwrap();
                    fs::write(target.join("member"), b"foreign arrival").unwrap();
                }
            }
        });
        drop(reader);
        assert_eq!(
            result,
            if arrival {
                FsStep::Conflict
            } else {
                FsStep::Done
            }
        );
        assert!((1..=3).contains(&waits));
        if arrival {
            assert_eq!(fs::read(&member).unwrap(), b"owned payload");
            assert_eq!(fs::read(target.join("member")).unwrap(), b"foreign arrival");
        } else {
            assert!(!source.exists());
            assert_eq!(fs::read(target.join("member")).unwrap(), b"owned payload");
        }
    }
}

#[test]
fn persistent_directory_denial_is_bounded_and_file_moves_do_not_wait() {
    for directory in [true, false] {
        let fixture = Fixture::new();
        let source = fixture.0.join("staged");
        let target = fixture.0.join("published");
        let member = if directory {
            fs::create_dir(&source).unwrap();
            source.join("member")
        } else {
            source.clone()
        };
        fs::write(&member, b"owned payload").unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&member)
            .unwrap();
        let mut waits = 0;
        let result = move_no_replace_with_wait(&source, &target, |_| waits += 1);
        drop(reader);
        assert_eq!(result, FsStep::Failed);
        assert_eq!(waits, if directory { 3 } else { 0 });
        assert_eq!(fs::read(&member).unwrap(), b"owned payload");
        assert!(!target.exists());

        let missing = fixture.0.join("missing");
        waits = 0;
        assert_eq!(
            move_no_replace_with_wait(&missing, &target, |_| waits += 1),
            FsStep::Failed
        );
        assert_eq!(waits, 0);
    }
}
