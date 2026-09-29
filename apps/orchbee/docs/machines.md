# Saved Windows machines

The native Windows Client owns saved SSH machine profiles. The six `machine`
commands manage labels, SSH targets, explicit remote Site names, and enabled
state. OpenSSH owns credentials, keys, host trust, and SSH configuration.
Reference behavior is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`.

## Catalog and transitions

`machines/catalog.json` beside the selected Windows configuration file is a
private, atomic, schema-version-1 document. Its revision is a nonnegative safe
integer; profiles have distinct random 32-character lowercase hexadecimal IDs.
At most 64 profiles and 128 KiB are admitted. Labels are nonblank, control-free,
and at most 128 UTF-8 bytes. Targets are at most 1024 ASCII bytes: an SSH host
alias, `[user@]host`, or `ssh://[user@]host[:port]`; bracketed IPv6 is supported.
Passwords and shell metacharacters are rejected. Session names use the native
Windows Site grammar, defaulting to `default`. Duplicate target/session pairs
may have distinct IDs, as in the reference.

| Event          | Guard and transition                                                                                  | Effects                                    |
| -------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| List           | Missing catalog is empty; invalid data fails                                                          | Read only; no directory creation           |
| Add            | Validate and check capacity; prepare remote; reload latest catalog under lock; append enabled profile | Remote Site may start before local commit  |
| Rename         | Exact ID exists; valid label                                                                          | Replace label; same value is unchanged     |
| Enable/disable | Exact ID exists                                                                                       | Set flag; same value is unchanged          |
| Remove         | Exact ID exists                                                                                       | Remove profile; never stop its remote Site |

Writers use the existing Windows private-cache lock and atomic replacement.
Revision advances once per actual change. Reads reject unknown fields, invalid
identities, duplicate IDs, overflow, oversized files, and unsafe filesystem
objects. Corruption never becomes an empty catalog. A failed local commit does
not roll back a remotely started Site. Cancellation after commit preserves the
committed receipt.

## Remote preparation

Add uses the OS OpenSSH executable and a 60-second total deadline. Batch mode,
strict host-key checking, no forwarding, bounded output, and an owned process
Job apply. Password prompts and new host trust must first be handled through
OpenSSH. SSH diagnostics are not copied into machine JSON or product errors.
The Windows remote command is an encoded PowerShell program with literal data.

Preparation checks the installed `orchbee.exe` Client version, platform,
protocols, and admitted cohort before starting the selected Site. A second SSH
invocation checks that the same cohort is still running and compatible after
the first invocation has ended. Only then can the profile be committed. The
receipt distinguishes `not-contacted`, `unchanged`, `possibly-started`, and
`ready` remote state, independently of the local catalog commit. Unknown
process settlement is reported, never retried as a successful add.

Missing or incompatible remote installations fail without saving. Automated
remote installation, interactive authentication, live local TUI connections,
machine switching, and remote CLI routing remain outside the implemented
boundary; an enabled profile is not evidence of a live connection. No remote
session is stopped by rename, enable, disable, or remove.

## Evidence and lifecycle

Deterministic validation and mutation traces cover invalid inputs, repeated
operations, capacity, and revision bounds. Private-store tests exercise the
real Windows lock and atomic writer. Remote-program tests run real PowerShell
against owned fixtures, including failed preparation and malformed replies.
These tests do not establish a real two-machine SSH/TUI witness. Fixtures and
their process Jobs are owned by the test and released after the last consumer.
