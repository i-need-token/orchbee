use super::*;
fn id(n: usize) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&format!("{n:032x}")).unwrap()
}
fn source(n: usize) -> MetadataSource {
    MetadataSource::parse(&format!("source-{n}")).unwrap()
}
fn patch(values: &[(&str, Option<&str>)]) -> MetadataTokenPatch {
    MetadataTokenPatch::new(
        values
            .iter()
            .map(|(key, value)| {
                (
                    MetadataTokenKey::parse(key).unwrap(),
                    value.map(|value| MetadataTokenValue::parse(value).unwrap()),
                )
            })
            .collect(),
    )
    .unwrap()
}
fn values(registry: &WorkspaceMetadata) -> BTreeMap<String, String> {
    registry
        .resources
        .get(&id(1))
        .map_or_else(BTreeMap::new, |resource| {
            resource
                .tokens
                .iter()
                .map(|(key, token)| (key.as_str().into(), token.value.as_str().into()))
                .collect()
        })
}
#[test]
fn sequences_survive_clear_and_expiry_without_stale_refresh() {
    let now = Instant::now();
    let mut registry = WorkspaceMetadata::default();
    let live = BTreeSet::from([id(1)]);
    registry
        .report(
            &id(1),
            &source(1),
            &patch(&[("summary", Some("old"))]),
            Some(WorkbenchRevision::new(0)),
            Some(10),
            now,
        )
        .unwrap();
    assert_eq!(
        registry.report(
            &id(1),
            &source(1),
            &patch(&[("summary", Some("stale"))]),
            Some(WorkbenchRevision::new(0)),
            Some(1000),
            now + Duration::from_millis(9)
        ),
        Ok(Disposition::Stale)
    );
    registry
        .maintain(&live, now + Duration::from_millis(10))
        .unwrap();
    assert!(values(&registry).is_empty());
    assert_eq!(
        registry.report(
            &id(1),
            &source(1),
            &patch(&[("summary", Some("resurrect"))]),
            Some(WorkbenchRevision::new(0)),
            None,
            now
        ),
        Ok(Disposition::Stale)
    );
    registry
        .report(
            &id(1),
            &source(1),
            &patch(&[("summary", Some("unsequenced"))]),
            None,
            None,
            now,
        )
        .unwrap();
    registry
        .report(
            &id(1),
            &source(1),
            &patch(&[("summary", None)]),
            Some(WorkbenchRevision::new(u64::MAX)),
            None,
            now,
        )
        .unwrap();
    assert!(values(&registry).is_empty());
    assert_eq!(
        registry.resources[&id(1)].sequences[&source(1)],
        WorkbenchRevision::new(u64::MAX)
    );
    assert_eq!(
        registry.report(
            &id(1),
            &source(1),
            &patch(&[("summary", Some("stale"))]),
            Some(WorkbenchRevision::new(u64::MAX)),
            None,
            now
        ),
        Ok(Disposition::Stale)
    );
}
#[test]
fn key_overwrite_is_global_and_old_expiry_cannot_remove_replacement() {
    let now = Instant::now();
    let mut registry = WorkspaceMetadata::default();
    let live = BTreeSet::from([id(1)]);
    registry
        .report(
            &id(1),
            &source(1),
            &patch(&[("x", Some("first")), ("other", Some("expires"))]),
            None,
            Some(10),
            now,
        )
        .unwrap();
    registry
        .report(
            &id(1),
            &source(2),
            &patch(&[("x", Some("second"))]),
            None,
            None,
            now,
        )
        .unwrap();
    registry
        .maintain(&live, now + Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        values(&registry),
        BTreeMap::from([("x".into(), "second".into())])
    );
    assert_eq!(
        registry.bytes,
        registry
            .resources
            .values()
            .map(Resource::bytes)
            .sum::<usize>()
    );
    registry.maintain(&BTreeSet::new(), now).unwrap();
    assert_eq!(registry.bytes, 0);
    assert!(registry.resources.is_empty());
}
#[test]
fn atomic_token_and_source_limits_do_not_consume_rejected_sequences() {
    let now = Instant::now();
    let mut registry = WorkspaceMetadata::default();
    for n in 0..32 {
        registry
            .report(
                &id(1),
                &source(n),
                &patch(&[(&format!("k{n}"), Some("value"))]),
                Some(WorkbenchRevision::new(1)),
                None,
                now,
            )
            .unwrap();
    }
    let before = registry.resources[&id(1)].clone();
    let revision = registry.revision;
    assert_eq!(
        registry.report(
            &id(1),
            &source(0),
            &patch(&[("extra", Some("overflow"))]),
            Some(WorkbenchRevision::new(2)),
            None,
            now
        ),
        Err(Failure::TokenLimit)
    );
    assert_eq!(
        registry.report(
            &id(1),
            &source(33),
            &patch(&[("k0", None)]),
            Some(WorkbenchRevision::new(1)),
            None,
            now
        ),
        Err(Failure::SequenceSourceLimit)
    );
    assert_eq!(registry.resources[&id(1)], before);
    assert_eq!(registry.revision, revision);
    registry
        .report(
            &id(1),
            &source(0),
            &patch(&[("k0", None), ("extra", Some("replacement"))]),
            Some(WorkbenchRevision::new(2)),
            None,
            now,
        )
        .unwrap();
    assert_eq!(registry.resources[&id(1)].tokens.len(), 32);
}
#[test]
fn revision_and_registry_capacity_fail_without_partial_replacement() {
    let now = Instant::now();
    let mut registry = WorkspaceMetadata::default();
    let token = patch(&[("k", Some("v"))]);
    for n in 1..=MAXIMUM_RESOURCES {
        registry
            .report(&id(n), &source(0), &token, None, None, now)
            .unwrap();
    }
    let bytes = registry.bytes;
    assert_eq!(
        registry.report(
            &id(MAXIMUM_RESOURCES + 1),
            &source(0),
            &token,
            None,
            None,
            now
        ),
        Err(Failure::ResourceLimit)
    );
    assert_eq!(registry.bytes, bytes);
    registry.revision = u64::MAX;
    let before = registry.resources[&id(1)].clone();
    assert_eq!(
        registry.report(
            &id(1),
            &source(0),
            &token,
            Some(WorkbenchRevision::new(1)),
            None,
            now
        ),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(registry.resources[&id(1)], before);
    assert_eq!(
        registry.maintain(&BTreeSet::new(), now),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(registry.resources.len(), MAXIMUM_RESOURCES);
}
#[test]
fn retained_payload_limit_is_independent_of_resource_count() {
    let now = Instant::now();
    let mut registry = WorkspaceMetadata::default();
    let value = "字".repeat(80);
    let patch = MetadataTokenPatch::new(
        (0..16)
            .map(|n| {
                (
                    MetadataTokenKey::parse(&format!("key-{n}")).unwrap(),
                    Some(MetadataTokenValue::parse(&value).unwrap()),
                )
            })
            .collect(),
    )
    .unwrap();
    let mut rejected = false;
    for n in 1..MAXIMUM_RESOURCES {
        match registry.report(&id(n), &source(0), &patch, None, None, now) {
            Ok(_) => assert!(registry.bytes <= MAXIMUM_RETAINED_BYTES),
            Err(Failure::ResourceLimit) => {
                assert!(!registry.resources.contains_key(&id(n)));
                rejected = true;
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(rejected);
    assert_eq!(
        registry.bytes,
        registry
            .resources
            .values()
            .map(Resource::bytes)
            .sum::<usize>()
    );
}
#[test]
fn generated_report_and_clock_traces_match_independent_visible_value_model() {
    let base = Instant::now();
    let live = BTreeSet::from([id(1)]);
    let mut registry = WorkspaceMetadata::default();
    let mut model = BTreeMap::<String, (String, Option<u64>)>::new();
    let mut sequences = BTreeMap::<usize, u64>::new();
    let mut random = 7_u64;
    let mut tick = 0;
    for _ in 0..4000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        tick += (random >> 32) % 5;
        let who = (random % 3) as usize;
        let key = format!("k{}", (random >> 8) % 6);
        let seq = (!(random >> 16).is_multiple_of(3)).then_some((random >> 20) % 20);
        let ttl = (random >> 40).is_multiple_of(2).then_some(7);
        let clear = (random >> 48).is_multiple_of(4);
        let value = format!("v{}", random % 11);
        model.retain(|_, (_, deadline)| deadline.is_none_or(|deadline| deadline > tick));
        registry
            .maintain(&live, base + Duration::from_millis(tick))
            .unwrap();
        let stale = seq.is_some_and(|seq| sequences.get(&who).is_some_and(|last| seq <= *last));
        if !stale {
            if let Some(seq) = seq {
                sequences.insert(who, seq);
            }
            if clear {
                model.remove(&key);
            } else {
                model.insert(
                    key.clone(),
                    (value.clone(), ttl.map(|ttl| tick + u64::from(ttl))),
                );
            }
        }
        assert_eq!(
            registry
                .report(
                    &id(1),
                    &source(who),
                    &patch(&[(&key, (!clear).then_some(&value))]),
                    seq.map(WorkbenchRevision::new),
                    ttl,
                    base + Duration::from_millis(tick)
                )
                .unwrap(),
            if stale {
                Disposition::Stale
            } else {
                Disposition::Applied
            }
        );
        assert_eq!(
            values(&registry),
            model
                .iter()
                .map(|(key, (value, _))| (key.clone(), value.clone()))
                .collect()
        );
        assert_eq!(
            registry.bytes,
            registry
                .resources
                .values()
                .map(Resource::bytes)
                .sum::<usize>()
        );
    }
}

#[test]
fn deadline_overflow_is_atomic_and_stale_reports_do_not_refresh_it() {
    let now = Instant::now();
    // Find a representable instant within one second of the platform limit.
    let mut edge = now;
    for bit in (0..64).rev() {
        if let Some(next) = edge.checked_add(Duration::from_secs(1_u64 << bit)) {
            edge = next;
        }
    }
    let mut registry = WorkspaceMetadata::default();
    let token = patch(&[("k", Some("v"))]);
    registry
        .report(
            &id(1),
            &source(0),
            &token,
            Some(WorkbenchRevision::new(1)),
            None,
            now,
        )
        .unwrap();
    let before = registry.resources[&id(1)].clone();
    let revision = registry.revision;
    assert_eq!(
        registry.report(
            &id(1),
            &source(0),
            &token,
            Some(WorkbenchRevision::new(2)),
            Some(86_400_000),
            edge
        ),
        Err(Failure::DeadlineOverflow)
    );
    assert_eq!(
        registry.report(
            &id(1),
            &source(0),
            &token,
            Some(WorkbenchRevision::new(1)),
            Some(86_400_000),
            edge
        ),
        Ok(Disposition::Stale)
    );
    assert_eq!(registry.resources[&id(1)], before);
    assert_eq!(registry.revision, revision);
}
