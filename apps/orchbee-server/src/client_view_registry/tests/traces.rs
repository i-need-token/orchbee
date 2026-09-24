//! Generated interleavings check ownership and evidence preservation, shrinking a failing tape.
use super::*;
use std::collections::BTreeSet;

fn replay(tape: &[u8]) -> Result<(), String> {
    let start = Instant::now();
    let mut now = start;
    let mut registry = ClientViewRegistry::default();
    let mut live = [true; 2];
    let mut samples = [sample(), sample()];
    for generation in 1..=2 {
        registry.execute(owner(generation), sample(), now);
    }
    let mut delivered = BTreeSet::new();
    for (index, encoded) in tape.iter().copied().enumerate() {
        let target = u64::from(encoded >> 6) % 2 + 1;
        let slot = usize::try_from(target - 1).unwrap();
        let id = u64::from((encoded >> 3) % 8) + 20;
        let terminal_before: Vec<_> = registry
            .receipts
            .iter()
            .filter(|(_, receipt)| !receipt.operation.status.pending())
            .map(|(id, receipt)| (id.clone(), receipt.operation.clone()))
            .collect();
        let action = match encoded % 8 {
            0 | 7 => {
                let mut command = command(id, target);
                if encoded % 8 == 7 {
                    command.minimum_workbench_revision = WorkbenchRevision::new(13);
                }
                Some(ClientViewAction::Submit {
                    command: Box::new(command),
                })
            }
            1..=3 if live[slot] => {
                let ClientViewAction::Synchronize {
                    heartbeat,
                    view_revision,
                    view,
                    completion,
                } = &mut samples[slot]
                else {
                    unreachable!();
                };
                *heartbeat = ClientViewSequence::new(heartbeat.value() + 1).unwrap();
                *completion = None;
                if encoded % 8 == 2 {
                    *view_revision = ClientViewSequence::new(view_revision.value() + 1).unwrap();
                    let selection = view.selection.as_mut().unwrap();
                    selection.zoomed = !selection.zoomed;
                }
                if encoded % 8 == 3 {
                    *completion = Some(ClientViewCompletion {
                        operation_id: operation_id(id),
                        outcome: ClientViewCompletionOutcome::Applied {},
                    });
                }
                Some(samples[slot].clone())
            }
            4 => {
                registry.disconnect(owner(target), now);
                live[slot] = false;
                None
            }
            5 => Some(ClientViewAction::Read {
                operation_id: operation_id(id),
            }),
            6 => {
                now += Duration::from_millis(100);
                None
            }
            _ => None,
        };
        if let Some(action) = action {
            let outcome = registry.execute(owner(target), action, now);
            if let ClientViewOutcome::Synchronized {
                client,
                control: Some(command),
            } = outcome
            {
                if client.client_id != super::client(target)
                    || command.client_id != client.client_id
                {
                    return Err(format!("wrong owner at {index}"));
                }
                if !delivered.insert(command.operation_id) {
                    return Err(format!("redelivery at {index}"));
                }
            }
        }
        for (id, previous) in terminal_before {
            if registry
                .receipts
                .get(&id)
                .is_none_or(|receipt| receipt.operation != previous)
            {
                return Err(format!("completed evidence changed at {index}"));
            }
        }
        for generation in 1..=2 {
            let pending = registry
                .receipts
                .values()
                .filter(|receipt| {
                    receipt.operation.command.client_id == client(generation)
                        && receipt.operation.status.pending()
                })
                .count();
            if pending > 1 || (!live[usize::try_from(generation - 1).unwrap()] && pending > 0) {
                return Err(format!("ownership leak at {index}"));
            }
        }
        if registry.receipts.len() > MAXIMUM_RECEIPTS
            || registry.clients.len() > MAXIMUM_CLIENT_VIEWS
        {
            return Err(format!("capacity exceeded at {index}"));
        }
        for receipt in registry.receipts.values() {
            receipt
                .operation
                .validate()
                .map_err(|error| format!("invalid receipt at {index}: {error}"))?;
            if matches!(
                receipt.operation.status,
                ClientViewControlStatus::Applied { .. }
            ) && !delivered.contains(&receipt.operation.command.operation_id)
            {
                return Err(format!("success without delivery at {index}"));
            }
        }
    }
    Ok(())
}

#[test]
fn generated_client_interleavings_preserve_single_delivery_and_completed_evidence() {
    for seed in 1..=96_u64 {
        let mut entropy = seed;
        let mut tape = vec![0, 1, 3, 64, 65, 67];
        for _ in 0..128 {
            entropy = entropy
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            tape.push(entropy.to_be_bytes()[0]);
        }
        if let Err(original) = replay(&tape) {
            // Deletion shrinking preserves only steps needed to reproduce the invariant failure.
            let mut index = 0;
            while index < tape.len() {
                let mut shorter = tape.clone();
                shorter.remove(index);
                if replay(&shorter).is_err() {
                    tape = shorter;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!(
                "seed={seed}, reduced={tape:?}, failure={original}, reduced failure={:?}",
                replay(&tape)
            );
        }
    }
}
