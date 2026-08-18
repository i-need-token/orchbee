# Desktop Terminal Host Update Continuity

| Field             | Value                                                                                                            |
| ----------------- | ---------------------------------------------------------------------------------------------------------------- |
| Status            | Accepted future design; versioned Host installation and updater activation are not implemented                   |
| Owner             | `apps/desktop` maintainers                                                                                       |
| Current authority | [`apps/desktop/README.md`](../../apps/desktop/README.md) and the owning Terminal package and application guides  |
| Exit condition    | Implement the accepted behavior, move maintained facts into Desktop-owned guides and tests, then delete proposal |

Simplified Chinese review translation:
[desktop-terminal-host-update-continuity.zh-CN.md](desktop-terminal-host-update-continuity.zh-CN.md).
English remains canonical.

## Decision

Desktop updates must not terminate retained terminal processes merely because Electron restarts or
the bundled Host version changes. Desktop owns Host installation, discovery, compatibility checks,
activation, rollback, and cleanup. Terminal Host owns neither product-update policy nor release
channels.

## Versioned installation

- Install verified Host binaries in an application-data runtime directory keyed by platform,
  architecture, version, and release channel.
- A signed manifest identifies the Host artifact, digest, and exact protocol contract.
- Endpoint metadata identifies the active Host epoch, executable version, exact protocol, process,
  and release channel without containing terminal content.
- Reuse a compatible running Host across Desktop and renderer restarts even when the Desktop bundle
  contains a newer Host.
- Never replace a Host while it owns a retained terminal.

## Activation

On Desktop startup:

1. Discover and negotiate with the active Host for the same user and release channel.
2. Reuse it when Desktop and Host declare the same exact protocol.
3. Reconcile Desktop recovery bindings with Host inventory by `CreationId`.
4. Reattach retained terminals with new attachment identities and fresh projections.
5. Activate a newer Host only after the active Host owns no retained terminal.

The updater may download an update while terminals are retained. A compatible Desktop update may
restart Electron and reattach to the live Host. If the new Desktop cannot communicate with the live
Host, activation remains deferred until the retained terminals are closed or another explicitly
accepted recovery path is available.

## Failure and cleanup

| Failure                                   | Required behavior                                                                 |
| ----------------------------------------- | --------------------------------------------------------------------------------- |
| New Host fails before owning a terminal   | Stop it and return to the last verified compatible Host                           |
| Protocol mismatch with retained terminals | Keep the compatible Desktop and Host available; do not replace the terminal owner |
| Renderer or Electron restart              | Detach client projections and preserve Host terminals                             |
| Host crash, OS restart, or PTY loss       | Report loss; never claim process continuity                                       |

Remove an old Host binary only when no active process uses it, no supported installed Desktop needs
its protocol contract, and rollback policy no longer retains it.

## Reserved decisions

The implementation slice must separately accept:

- update-provider and signed-manifest transport;
- signing and platform trust integration;
- release-channel policy and, only after a released compatibility need exists, the policy and wire
  representation for supporting more than one protocol contract;
- installer and uninstall behavior;
- operating-system shutdown behavior; and
- user recovery when a release intentionally removes compatibility with a retained Host.

Packaging a development Host beside Electron is not evidence for this design. Network sharing,
remote clients, durable PTY recovery after Host or OS loss, and product topology in Host remain
outside this proposal.
