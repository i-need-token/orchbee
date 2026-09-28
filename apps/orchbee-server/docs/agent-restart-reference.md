# Agent restart references

## Ownership and scope

The Windows clean-stop checkpoint retains external provider session references by stable Workbench Presentation identity. This is restart intent, not an Orchbee canonical Agent Session, a live process, a name binding, a source authority or an authorization to replay a hook. The existing terminal-report owner supplies the reference; the checkpoint codec validates it before any runtime starts.

The selected reference is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `persist/snapshot.rs` and `terminal/state.rs`: use the current authority's provider reference, otherwise the retained provider reference. Cleared or released references must not reappear from an older checkpoint seed. Both primary and child Presentations have their own identity and reference.

## Storage and transition model

Checkpoint version 2 adds a required `agentSessions` list. Each record carries `presentationId` and `session` with exact official source, normalized Agent label, and provider reference kind/value. Version 1 remains readable only without this field. Unknown fields, duplicate records, missing membership, unsupported versions, invalid source/Agent combinations, control characters, invalid reference bounds or nonabsolute native paths reject the entire checkpoint. The existing 2 MiB encoded limit remains.

| Transition | Required observation | Result |
| --- | --- | --- |
| Capture live reference | Committed Presentation and exact retained Worker generation/Host identity | Store only that report owner's provider reference |
| Capture pending restart intent | Still retained Presentation without a superseding report owner | Preserve the admitted reference; do not invent process state |
| Parse and reconstruct | Valid complete checkpoint and fresh Site/Worker | Reconstruct unbound topology and separate pending references |
| Prepare new binding | Exact unchanged Presentation membership and reference | Carry the reference on the typed restore target |
| Seed after physical creation | Current logical binding and uniquely retained fresh physical identity | Seed only the retained reference into an otherwise empty report owner |
| Report owner already exists | Current physical report owner is authoritative | Consume pending seed without replacing current state |
| Clear or exit after seed | Ordinary report transition | Subsequent capture reflects current state, including absence |

Seeding is prepared before mutation, obeys report resource/revision bounds and advances only the report revision. It does not set detected Agent, fallback state, authority, sequence watermarks, suppression history or canonical Agent observations. Stored references contain no executable path, shell command, environment or credentials. Runtime launch, deduplication of provider resume requests, observed cwd, names and managed-state restoration remain distinct required consumers.

## Evidence and compatibility

Tests cover official ID/path references, primary/child separation, exact physical-generation admission, malformed and duplicate records, membership deletion, v1/v2 compatibility, bounded encoding, fresh identity seeding, candidate rollback and clearing without resurrection. Windows acceptance must exercise actual clean stop, fresh Site restart and public report reads; storage and seed evidence does not prove that a provider was resumed or authenticated.

Older binaries reject version 2 explicitly. Version 1 files can upgrade on the next successful clean stop; failed capture/publication preserves the preceding committed file. Deploy matched cohorts. Do not erase an unsupported checkpoint to make startup succeed. Full regression and real provider resume acceptance remain required before the Windows parity goal is complete.
