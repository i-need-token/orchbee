# Desktop SSH Environment Release

## Scope

Slice 2.3B delivered the frozen schema-version-1 Runtime-only release. Slice 3.1A adds the
schema-version-2 SSH Environment release containing one SSH Runtime and one Terminal Host for each
accepted platform. Repository automation builds and assembles the cohort, an external authority
signs its exact canonical payload, and Desktop verifies and privately stages every component before
Electron Packager runs.

`desktop:package:dev` and `desktop:package:local` are structurally release-free.
`desktop:package:release` requires v2. Slice 3.1B now consumes that verified cohort for atomic remote
installation and exact Terminal Host relay negotiation. This release boundary still does not create
a remote Workspace or terminal, publish a feed, sign or notarize Electron, or define update and
cleanup policy.

## Accepted decisions D37-D46

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                          |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D37      | `@orchbee/ssh-environment-protocol` owns the shared release-manifest and trust-descriptor contract; the Runtime owns builds and offline bundle preparation/finalization; Desktop owns independent verification and package composition.                                                                                    |
| D38      | The exact targets are `aarch64-macos` and `x86_64-linux-gnu`, both with one explicit semantic Runtime version, Zig `0.16.0`, `ReleaseSafe`, and stripping. Release identity never comes from Git state.                                                                                                                    |
| D39      | Prepared, finalized, and packaged bundles have fixed layouts and canonical JSON. Unknown fields, entries, symlinks, non-regular files, size drift, digest drift, and platform-matrix drift fail closed.                                                                                                                    |
| D40      | Repository code never reads a production private key. The workflow is prepare → external or HSM-backed raw Ed25519 signature → finalize with the 64-byte signature and independent public key.                                                                                                                             |
| D41      | One Desktop release trusts one Ed25519 key and one exact manifest. Its descriptor stores canonical SPKI, `ed25519-spki-sha256:<digest>`, and the SHA-256 of the exact final manifest bytes.                                                                                                                                |
| D42      | The trust descriptor is staged inside `app.asar`; the manifest and artifacts are staged beneath Electron `Resources`. Desktop reads, verifies, and buffers all admitted bytes into a private staging area before packaging.                                                                                                |
| D43      | Explicit Dev and Local packaging exclude release payloads. `desktop:package:release` requires an absolute finalized-bundle path and independent public-key path and fails before Electron Packager on missing or invalid input.                                                                                            |
| D44      | This slice has no multi-key trust, feed, automatic upgrade, or downgrade policy. A signing-key or Runtime change produces a new Desktop package; an application rollback carries its own original bundle and descriptor.                                                                                                   |
| D45      | This slice does not compose SSH connection, installation, supervisor, or installed-artifact fast-path behavior and does not own Electron code signing or notarization.                                                                                                                                                     |
| D46      | Deterministic evidence uses ephemeral Ed25519 keys, full prepare/sign/finalize/verify flow, mutation and canonical-layout cases, two-target compilation, platform-native Runtime tests, and the repository gate. No production key enters tests; no TLA+ or Lean artifact is required for this finite sequential workflow. |

## Accepted decisions D77-D86

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D77      | Phase 3 is sliced. Slice 3.1A owns only remote-execution component construction, signing, verification, and Desktop packaging; it creates no remote installation, process, Workspace, or terminal effect.                                                                                                                                                                                                                                                  |
| D78      | Existing `orchbee-terminal-host` remains the sole PTY, process-tree, Ghostty-state, and terminal-recovery authority. SSH Runtime does not reimplement terminal behavior and gains no generic execution, shell, or network listener in this slice.                                                                                                                                                                                                          |
| D79      | `ssh-environment-release-v2` is independent from frozen v1 and contains exactly `ssh-environment-runtime` then `terminal-host`; each component contains exactly `darwin-arm64` then `linux-x64`, for four artifacts total.                                                                                                                                                                                                                                 |
| D80      | Runtime and Terminal Host retain distinct semantic component versions, sizes, and SHA-256 digests. The signed manifest digest identifies the compatibility cohort; it does not replace Runtime build identity or future Terminal Host epoch identity.                                                                                                                                                                                                      |
| D81      | Both components use Zig `0.16.0`, `ReleaseSafe`, stripping, and explicit versions. The exact targets are Apple Silicon macOS and x64 glibc Linux, with artifact names `orchbee-ssh-environment-runtime` and `orchbee-terminal-host`.                                                                                                                                                                                                                       |
| D82      | Each native application owns its build recipe. Repository-level release automation owns cross-application prepare/finalize and consumes installed build output; neither application imports the other's source tree.                                                                                                                                                                                                                                       |
| D83      | Prepare is offline and finalize accepts one external raw Ed25519 signature. One canonical signature covers every component role, version, platform, size, and digest. Layouts are exact, symlinks and extra entries fail closed, every artifact is bounded at 128 MiB, and the manifest is bounded at 64 KiB. Production private keys remain outside repository tooling.                                                                                   |
| D84      | Explicit Desktop release packaging independently verifies the signature, manifest, four artifacts, and public key before staging. It places the exact trust descriptor inside `app.asar` and buffered manifest/artifacts beneath `Resources`; Dev and Local packaging remain release-free.                                                                                                                                                                 |
| D85      | Frozen v1 artifacts remain valid only for already delivered Runtime and `project-root-v1` behavior. A future Terminal Host capability requires verified v2 and cannot use a v1 companion, remote `PATH`, remote build, network download, mixed cohort, or validation-failure downgrade.                                                                                                                                                                    |
| D86      | Evidence covers strict schemas and canonical bytes, missing/reordered/expanded/oversized/tampered inputs, wrong keys and signatures, symlinks, fixed build arguments, real target compilation, prepare/finalize/package round trips, documentation pairing, and the repository gate. Real SSH, remote upload, production signing, Workspace, terminal execution, and UI remain outside the claim; this bounded sequential workflow requires no TLA+ model. |

## Versioned contracts

| Contract | Components                                    | Consumer                                                                                                                                      |
| -------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| v1       | Two SSH Runtime artifacts                     | Frozen legacy contract for existing Slice 2.3B through 2.5 behavior; new Desktop release packaging does not admit it                          |
| v2       | SSH Runtime and Terminal Host × two platforms | Current `desktop:package:release` and Slice 3.1B SSH activation; bootstrap installs and commits the exact selected-platform cohort atomically |

The schemas are not a permissive union. A v2 path never treats v1 bytes as a missing companion and
never falls back after v2 validation fails.

## Modeling record

| Field        | Slice 3.1A record                                                                                                                                                                                                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Bind the exact Runtime and Terminal Host compatibility cohort to one Desktop package without admitting a production private key, remote download, remote build, path lookup, or component substitution.                                                                                        |
| Inputs       | Trusted intent is two explicit semantic versions, one externally governed Ed25519 signer, and one independently supplied public key. Build output, prepared/final directories, signatures, manifests, filesystem kinds, package variables, and absent inputs are untrusted until validated.    |
| Data         | Schema-version-2 manifests contain an ordered two-component by two-platform matrix. Trust schema `2` binds canonical SPKI, its derived signing-key identity, and the exact final-manifest digest.                                                                                              |
| Outputs      | Prepare and finalize atomically publish exact absent-target directories. Release packaging either invokes Electron Packager with a privately buffered verified cohort or fails first. Dev and Local packaging contain no SSH Environment release.                                              |
| Invariants   | Every required component exists once, every signed size and digest matches buffered bytes, component versions remain distinct, canonical manifest bytes define the cohort, trust pins signer and manifest, and no production private key is an input.                                          |
| Ownership    | Native applications own component builds; repository automation owns cohort assembly; the protocol package owns data contracts; the external authority owns signing; Desktop owns public-key normalization, trust construction, complete verification, buffering, and package placement.       |
| Verification | Protocol and Desktop unit tests, repository-automation tests, both native target compiles, documentation pairing, and `pnpm verify`. The workflow is finite and sequential, so deterministic transition and negative-path tests are stronger evidence than a concurrency model for this claim. |

## Canonical artifacts

Prepare emits exactly:

```text
prepared-release/
  artifacts/
    darwin-arm64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
    linux-x64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
  manifest-signing-payload.json
```

The signing payload has no trailing newline. Finalize accepts one matching public key and one raw
64-byte Ed25519 signature, rechecks all four artifacts, and replaces the payload with canonical
`manifest.json`. The final manifest and trust descriptor each have one trailing newline.

Release packaging creates:

```text
app.asar:/dist/main/ssh-environment-release-trust-v2.json
Resources/ssh-environment-release-v2/
  artifacts/
    darwin-arm64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
    linux-x64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
  manifest.json
```

The descriptor remains application trust state inside `app.asar`. Executable bytes remain real
files under `Resources` for later exact-artifact installation. The bundle never supplies its own
trust root.

## Offline workflow

All paths are absolute and requested output directories must be absent. From the repository root:

```sh
pnpm release:ssh-environment:prepare --output /absolute/prepared-release --runtime-version 1.2.3 --terminal-host-version 4.5.6

# An external authority signs the exact bytes in
# /absolute/prepared-release/manifest-signing-payload.json
# and writes one raw 64-byte Ed25519 signature.

pnpm release:ssh-environment:finalize --prepared /absolute/prepared-release --signature /absolute/signature.bin --public-key /absolute/release-public-key.pem --output /absolute/final-release

ORCHBEE_SSH_ENVIRONMENT_RELEASE_DIRECTORY=/absolute/final-release \
ORCHBEE_SSH_ENVIRONMENT_RELEASE_PUBLIC_KEY=/absolute/release-public-key.pem \
pnpm desktop:package:release
```

`pnpm desktop:package:dev` and `pnpm desktop:package:local` ignore these variables and include no
release cohort or trust descriptor.

## Trust and current limits

A Desktop package contains one descriptor and matching cohort. Changing either component or the
signing key requires another Desktop package; application rollback restores its original descriptor
and cohort together. There is no mutable current pointer, feed, key rollover, remote downgrade,
cleanup, or component replacement.

The manifest exposes component versions, platforms, sizes, and hashes. The descriptor exposes the
public key, derived key identity, and manifest digest. SSH credentials, Environment targets,
usernames, remote paths, AskPass values, and private keys are structurally absent.

Slice 3.1B remotely installs and consumes the exact Terminal Host artifact through one Runtime-owned
opaque relay, but does not change release signing or packaging ownership. Workspace admission,
terminal creation/routing, Agent execution, real-SSH interoperability, Electron signing,
notarization, publication, update distribution, component cleanup, and service installation require
later accepted slices.
