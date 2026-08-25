# POSIX Host Endpoint

## Scope and ownership

Electron Main owns the POSIX Terminal Host client endpoint adapter. The adapter prepares Desktop's
product-scoped runtime directory, authenticates the actual Unix-domain-socket peer, and transfers one
connected descriptor into `HostConnection`. It runs inside Electron Main and starts no endpoint-helper
process. A running POSIX Desktop composition therefore has one `orchbee-terminal-host` process, whose
only product command is `serve`.

Terminal Host owns the listener, endpoint lifetime lock, post-bind endpoint validation, and
kernel-credential authentication of every accepted Desktop peer. Desktop and Terminal Host each
authenticate the peer at their own side of the connection. Neither side treats the endpoint identifier
as a secret or distinguishes another trusted application running under the same effective user.

This contract does not change the Windows named-pipe adapter, Host protocol, terminal lifecycle,
retained-Host reconnect behavior, or Electron's own process model. It does not make Terminal Host a
library loaded into Electron.

## Connection model

| Phase                | Owner                  | Required transition                                                                                                                                                               |
| -------------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Runtime preparation  | Desktop native adapter | Open the absolute runtime directory without following a link; establish or validate effective-UID ownership, private mode, and the absence of extended or inheritable ACL access. |
| Endpoint observation | Desktop native adapter | Resolve only the validated endpoint identifier beneath that directory and require an owned private Unix-domain socket.                                                            |
| Socket connection    | Desktop native adapter | Connect and authenticate the effective UID of the actual connected server from kernel peer credentials. Path validation alone never authenticates the server.                     |
| Descriptor handoff   | Desktop Main           | Transfer the one authenticated descriptor exactly once into a Node `net.Socket`, then construct `HostConnection`; all pre-handoff failures close the descriptor.                  |
| Protocol admission   | `HostConnection`       | Perform the existing exact-version Hello handshake and preserve the existing request-admission and unknown-outcome rules.                                                         |
| Disconnect and retry | Desktop Main           | Close the current socket and repeat runtime, endpoint, and peer validation before every later connection. Only an authenticated connection may suppress a Host launch.            |

Runtime preparation and authenticated connection are fail-closed native operations. Missing native
code, an unsupported kernel credential API, invalid paths, wrong ownership, public permissions,
unexpected ACL access, a non-socket endpoint, peer-credential failure, or a different peer effective
UID cannot fall back to an unauthenticated Node connection.

The complete runtime-directory and endpoint path must fit the target's Unix-domain-socket pathname
limit. Product defaults and the supervised development workspace generate a bounded short path;
explicit overlong overrides fail before protocol work. The POSIX development supervisor uses one
random owner-only workspace directly beneath `/tmp` so the Darwin path remains bounded. That
controlled workspace is not the stable product rendezvous, and the native adapter still applies the
same no-follow, owner, mode, and ACL checks.

The server independently rejects a connected Desktop peer whose kernel identity is unavailable or
does not match the Host effective UID. A pathname race therefore cannot cross the effective-user trust
boundary: the Desktop authenticates the server reached by its connected descriptor, and the Host
authenticates the client accepted from the listener.

## Invariants and scenarios

- Desktop never invokes `orchbee-terminal-host` for runtime preparation or client connection.
- `orchbee-terminal-host serve` is the only Terminal Host product process in a running POSIX Desktop
  composition. A development build watcher is tooling and is not a second Terminal Host product
  process.
- A failed or unauthenticated connection creates no `HostConnection` and sends no Hello record.
- The native adapter owns a connected descriptor until successful return. Desktop owns it after
  return, and a Node `net.Socket` owns it after successful construction. Every failure path closes the
  descriptor exactly once.
- Connect-or-launch may launch `serve` only after an authenticated-connect attempt fails. Connect-only
  never launches it. Both modes repeat authentication on every retry.
- A retained Host may outlive Electron and accept a later authenticated Desktop reconnect. Closing the
  Desktop socket neither owns nor terminates the Host.
- Explicit development and test overrides pass through the same validation as derived product paths.

| Scenario                                                                                      | Observable result                                                                                                   |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Missing private runtime directory                                                             | Native preparation creates it with the required ownership and access, then validates the opened directory.          |
| Symlink, foreign owner, non-directory, or public runtime mode                                 | Preparation or connection fails before protocol work.                                                               |
| Unexpected ACL on an otherwise private owned runtime directory                                | Preparation removes it and validates the opened directory again; a later connection rejects an ACL that reappeared. |
| Missing, replaced, foreign-owned, non-socket, public, or unexpectedly ACL-accessible endpoint | Connection fails before protocol work.                                                                              |
| Runtime and endpoint path exceeds the platform socket-path bound                              | Connection fails before protocol work.                                                                              |
| Same-UID endpoint and authenticated server                                                    | Exactly one connected descriptor reaches `HostConnection`.                                                          |
| Missing or mismatched server credentials                                                      | The descriptor is closed and Host launch is not suppressed.                                                         |
| Socket loss followed by retry                                                                 | The previous descriptor is retired and the complete authentication sequence runs again.                             |
| Missing or unloadable native adapter                                                          | Desktop fails closed with no direct-connect fallback.                                                               |
| Windows composition                                                                           | Existing named-pipe preparation, server authentication, and direct `HostConnection` transport remain unchanged.     |

## Evidence and limits

| Claim                                               | Required evidence                                                                                                                                     | Limit                                                                              |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Runtime and endpoint checks enforce the POSIX rules | Native adapter tests on supported macOS and Linux targets cover type, no-follow, owner, mode, ACL, endpoint, and peer-credential cases.               | Platform kernels and filesystems outside the supported targets are not claimed.    |
| Descriptor ownership is exact                       | Native/JavaScript integration tests cover successful handoff plus preparation, connect, authentication, socket-construction, and disconnect failures. | Operating-system termination can reclaim descriptors without running cleanup code. |
| Desktop runs one Terminal Host product process      | A real Desktop system trace identifies the configured endpoint and requires one `serve` process with no `connect` or `prepare-runtime` process.       | Electron helpers and development build-tool processes are outside this count.      |
| Retained reconnect behavior is unchanged            | Deterministic Desktop tests and a real-Host integration trace reconnect after the first Desktop connection closes.                                    | Host or operating-system crash recovery remains outside the product promise.       |
| Packaged loading is deterministic                   | Desktop build and package verification require the current-platform native artifact at the code-resolved packaged path.                               | A modified installation is rejected rather than repaired at runtime.               |

The connection and retry state machine is unchanged: the native adapter replaces only the
pre-protocol POSIX authentication transport. Existing deterministic `DevelopmentHostRuntime` and
`HostConnection` tests remain the state-model refinement; the native and system checks above own the
new process-boundary evidence. The same-user boundary is not multi-tenant isolation and does not
protect against another malicious process already running with the same effective UID.

## Delivery and rollback

Desktop and Terminal Host are built and packaged together. The native adapter is a build artifact,
not a separately versioned runtime dependency, and the private Host protocol and endpoint identity do
not migrate. Rollback restores the previous Desktop/Host build together; there is no durable-data
migration or endpoint conversion. Production never carries a compatibility flag or an insecure
fallback path for the removed helper ABI.
