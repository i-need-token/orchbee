# Desktop Terminal Settings

## Ownership and scope

Electron Main owns the device-local terminal-default record and the only `SetTerminalDefaults`
authority. The Web renderer receives a narrow correlated settings capability; it never receives a
Host connection, storage path, generic filesystem capability, or permission to update a running
terminal. Gateway and ordinary Browser clients do not receive this capability.

The current record contains only `scrollbackBytes`, an integer from `0` through `1_000_000_000`.
The product default is `10_000_000` decimal bytes. Main stores the strict version-1 record at
`terminal-default-settings-v1.json` beneath Electron `userData`.

## Startup and recovery

Main reads the record before terminal runtime initialization and commits the selected value to the
singleton Terminal Host. A valid stored value wins over the runtime default. Missing storage uses
the runtime default without writing. Invalid or unreadable storage also uses the runtime default,
preserves the existing file, and projects a recoverable issue to Settings.

Startup fails closed when Host rejects or cannot confirm the selected value. Main does not expose a
renderer capability or create a terminal runtime until Host and the in-memory snapshot agree.

## Update transaction

One renderer document may have at most one settings request in progress. Main serializes admitted
updates, validates the complete configuration, asks Host to commit it, and then awaits owner-only
atomic replacement of the record. Main publishes the new session snapshot when that persistence
attempt succeeds or fails. While persistence is pending, Host already owns the new default but
the Desktop snapshot still contains the previous value; a new terminal captures Host's value.

| Failure point                               | Session value | Stored value   | Renderer result                                    |
| ------------------------------------------- | ------------- | -------------- | -------------------------------------------------- |
| Invalid command                             | Unchanged     | Unchanged      | Capability retires                                 |
| Host rejection or unavailable control       | Unchanged     | Unchanged      | Failed with the previous snapshot                  |
| Storage write failure after Host acceptance | New value     | Previous value | Completed with `write-failed` and source `session` |
| Host and storage success                    | New value     | New value      | Completed with source `stored`                     |

Retrying the same session value repeats the Host precommit and storage attempt, so a transient
write failure can converge without choosing a different value.

## Terminal lifetime

Terminal launches keep `configuration: null`; Terminal Host therefore captures its current default
when a new terminal is created. Changing `scrollbackBytes` affects only future terminals. Main does
not send `UpdateTerminalConfiguration`, evict retained rows, recreate an attachment, or truncate a
running terminal. Restored terminals keep the configuration owned by their existing Host session.

## Renderer capability

The tenth Desktop bootstrap port is `terminal-default-settings`. It supports one snapshot request
and one complete update request, both correlated by bounded request identity. Malformed data or
transferred ports retire the capability. Renderer-document replacement disposes the previous
broker, while the Main-owned controller and Host default survive the page replacement.

The Settings surface shows the control only when this Desktop capability initializes. Without it,
the shared Browser/Gateway surface keeps `terminal.scrollback` explicitly marked as planned.

## Verification

Run from the repository root:

```sh
pnpm exec vitest run apps/desktop/src/main/terminal-default-settings/__tests__
pnpm --filter @orchbee/desktop test
```

Controller, persistence and composition tests check one unsettled mutation, Host-first commit, failure retention, explicit retry, complete settings publication and preservation of already retained terminals. These are implementation traces; they do not provide a separate TLA proof or native row-storage/compression evidence. Run the applicable root gate at completion.
