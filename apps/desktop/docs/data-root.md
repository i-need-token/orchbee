# Desktop Data Root

## Authority and scope

This reference owns the Desktop product-data root and development-instance isolation. The data root
scopes Electron `userData`, the single-instance lock, Terminal Host identity, recovery state, local
preferences, and the default managed Git Worktree location. It does not define synchronized product
state, user-selectable storage locations, migration from earlier Electron-managed locations, or
automatic deletion.

## Resolution

| Environment | Data root                            |
| ----------- | ------------------------------------ |
| Packaged    | `<home>/.orchbee`                    |
| Development | `<home>/.orchbee-dev-<scope-digest>` |

Development resolves the canonical source-checkout root before hashing. The scope digest is the
first 16 lowercase hexadecimal characters of SHA-256 over a version-tagged identity containing the
platform-normalized real path. The version tag keeps a future identity migration explicit without
making process CWD or a nested package path part of the identity. The development launcher supplies
the checkout root explicitly.

Every development root contains one bounded scope marker with the complete canonical checkout path
and digest version. Startup accepts an existing root only when the marker agrees. A mismatch fails
closed instead of opening another checkout's state. A moved checkout receives a new root; the old
root remains untouched.

The built-in managed Worktree root is `<data-root>/worktrees`. Each created Worktree uses one
Main-allocated opaque identity as its direct child name. Project ID, repository ID, base ref, branch
name, and display name never determine the physical path.

## Startup order and ownership

Desktop Main resolves and creates the data root before requesting Electron's single-instance lock.
It sets Electron `userData` to that root first, so one scope controls both disk state and lock
identity. The same checkout therefore admits one Desktop instance, while packaged Desktop and
different development checkouts may run concurrently.

On POSIX, Main creates product-owned data directories with owner-only access. The Electron renderer
may receive the fixed managed Worktree root as read-only form copy, but it receives no source
repository path, data-root enumeration, location override, or generic filesystem capability.
Gateway clients receive no managed-Worktree capability. Tests use an explicitly injected temporary
home and source root.

## Invariants

- Packaged and development roots never resolve to the same path.
- Equal canonical development checkout paths resolve to equal roots.
- An accepted development root marker names exactly the path used to derive its digest.
- Different paths may not share one accepted root, including after a truncated-digest collision.
- Changing or moving a checkout never deletes or migrates the prior root.
- Worktrees are durable user code, not cache, and no reset or cleanup operation removes them
  implicitly.
- The fixed Worktree root is the only location policy in this slice. Application-level and
  Project-level overrides remain absent.

## Verification

Deterministic resolver tests cover packaged and development paths, canonical-path stability,
platform normalization, marker creation and mismatch, and different-checkout isolation. Desktop
configuration tests verify that the selected `userData` root feeds the managed Worktree and recovery
paths; the Main entrypoint selects `userData` before requesting the single-instance lock.
