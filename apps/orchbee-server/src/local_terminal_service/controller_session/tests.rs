use super::*;
#[test]
fn reservation_excludes_competitors_and_stale_cleanup_cannot_release_a_new_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let mut owners = BTreeMap::new();
    let target = ControlTarget {
        worker: [1; 16],
        generation: 1,
        host_epoch: [2; 16],
        terminal: [3; 16],
    };
    let first = OperationId::from_bytes([4; 16])?;
    let next = OperationId::from_bytes([5; 16])?;
    assert!(reserve(&mut owners, target, first));
    assert!(!reserve(&mut owners, target, next));
    remove_exact(&mut owners, target, next);
    assert_eq!(owners.get(&target), Some(&first));
    remove_exact(&mut owners, target, first);
    assert!(reserve(&mut owners, target, next));
    remove_exact(&mut owners, target, first);
    assert_eq!(owners.get(&target), Some(&next));
    assert!(reserve(
        &mut owners,
        ControlTarget {
            terminal: [6; 16],
            ..target
        },
        first
    ));
    Ok(())
}
