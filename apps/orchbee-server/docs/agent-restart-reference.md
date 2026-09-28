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

Seeding is prepared before mutation, obeys report resource/revision bounds and advances only the report revision. It does not set detected Agent, fallback state, authority, sequence watermarks, suppression history or canonical Agent observations. Stored references contain no executable path, shell command, environment or credentials.

## Windows resume launch

The unpublished Site prepares one resume launch per distinct official source, Agent, reference kind and value in Presentation traversal order. Duplicate Presentations start ordinary shells without a retained provider reference. Disabled resume preserves references and starts ordinary shells. ORCHBEE_RESUME_AGENTS_ON_RESTORE accepts exactly true or false and defaults to true. Client activation rejects invalid values before durable Active publication or marker commitment; existing Site admission is unaffected. Server independently validates before Worker launch. Correcting an invalid value permits another activation without recovery or checkpoint loss. This startup input does not implement the separate config.toml session setting or live reload.

The Server selects the current Windows provider command and literal resume arguments, then starts system Windows PowerShell with -NoLogo -NoProfile -NoExit -EncodedCommand in the existing terminal creation transaction. Shared literal encoding supports native programs, .cmd/.bat and .ps1. Program discovery admits applications and external scripts, never functions or aliases. The shell remains available after provider exit or resolution failure; its existence does not prove provider readiness.

| Stage | Guard and result |
| --- | --- |
| Planning | Validate every launch and reserve distinct references before Host effects |
| Dispatch | Submit each prepared terminal once under its existing creation identity |
| Completion or unknown outcome | Settle the terminal record without replay |
| Fresh physical binding | Seed only the reference; new observations establish Agent state |
| Duplicate reference | Preserve topology, start a shell and omit its reference |
| Provider failure or exit | Retain the terminal and its actual error/output |

Workbench/OBCS/OBCN launch selections admit 30 KiB of executable/argument UTF-8 bytes and 128 arguments, with a second 30 KiB worst-case JSON string-content budget: ASCII controls count as six bytes, quotes/backslashes as two, and other UTF-8 bytes as one. OBCS JSON frames admit 63 KiB within their existing u16 header; OBCN binary frames admit 40 KiB and Workbench launch requests 96 KiB. The independent Host 64 KiB control-document and Windows command-line limits still apply. Encoded resume arguments are ASCII and limited to 30,000 characters; batch dispatch rejects command lines over 8,000 characters. Older peers reject larger payloads; activate and roll back complete matching cohorts.

The reservation is conservatively retained after both known creation failure and unknown outcome. Herdr's alternate-duplicate retry after proven pre-process spawn failure remains unimplemented. Observed cwd, the configuration-file resume switch, names and managed-state restoration remain required integration work.

## Evidence and compatibility

Tests cover official ID/path references, primary/child separation, exact physical-generation admission, malformed and duplicate records, membership deletion, v1/v2 compatibility, bounded encoding, fresh identity seeding, candidate rollback and clearing without resurrection. Windows acceptance must exercise actual clean stop, fresh Site restart and public report reads; storage and seed evidence does not prove that a provider was resumed or authenticated.

Older binaries reject version 2 explicitly. Version 1 files can upgrade on the next successful clean stop; failed capture/publication preserves the preceding committed file. Deploy matched cohorts. Do not erase an unsupported checkpoint to make startup succeed. Full regression and real provider resume acceptance remain required before the Windows parity goal is complete.
