# Desktop SSH Runtime Release

## Scope and delivered boundary

Slice 2.3B delivers one offline, release-only composition path for the SSH Environment Runtime. The
Runtime workspace prepares one parameter-fixed two-target build, an external signer authenticates
the exact canonical payload, the Runtime workspace finalizes one signed bundle, and Desktop verifies
that bundle against an independently supplied public key before invoking Electron Packager.

The ordinary `desktop:package` command remains a development package with no SSH Runtime release.
The new `desktop:package:release` command is the only path that admits the signed bundle. This slice
does not connect an Environment, compose `SshEnvironmentSupervisor`, negotiate installed artifacts,
publish a feed, sign or notarize Electron, or add any Runtime service.

## Accepted decisions D37-D46

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                          |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D37      | `@orchbee/ssh-environment-protocol` owns the shared release-manifest and trust-descriptor contract; the Runtime owns builds and offline bundle preparation/finalization; Desktop owns independent verification and package composition.                                                                                    |
| D38      | The exact targets are `aarch64-macos` and `x86_64-linux-gnu`, both with one explicit semantic Runtime version, Zig `0.16.0`, `ReleaseSafe`, and stripping. Release identity never comes from Git state.                                                                                                                    |
| D39      | Prepared, finalized, and packaged bundles have fixed layouts and canonical JSON. Unknown fields, entries, symlinks, non-regular files, size drift, digest drift, and platform-matrix drift fail closed.                                                                                                                    |
| D40      | Repository code never reads a production private key. The workflow is prepare → external or HSM-backed raw Ed25519 signature → finalize with the 64-byte signature and independent public key.                                                                                                                             |
| D41      | One Desktop release trusts one Ed25519 key and one exact manifest. Its descriptor stores canonical SPKI, `ed25519-spki-sha256:<digest>`, and the SHA-256 of the exact final manifest bytes.                                                                                                                                |
| D42      | The trust descriptor is staged inside `app.asar`; the manifest and artifacts are staged beneath Electron `Resources`. Desktop reads, verifies, and buffers all admitted bytes into a private staging area before packaging.                                                                                                |
| D43      | `desktop:package` keeps its existing development behavior. `desktop:package:release` requires an absolute finalized-bundle path and independent public-key path and fails before Electron Packager on missing or invalid input.                                                                                            |
| D44      | This slice has no multi-key trust, feed, automatic upgrade, or downgrade policy. A signing-key or Runtime change produces a new Desktop package; an application rollback carries its own original bundle and descriptor.                                                                                                   |
| D45      | This slice does not compose SSH connection, installation, supervisor, or installed-artifact fast-path behavior and does not own Electron code signing or notarization.                                                                                                                                                     |
| D46      | Deterministic evidence uses ephemeral Ed25519 keys, full prepare/sign/finalize/verify flow, mutation and canonical-layout cases, two-target compilation, platform-native Runtime tests, and the repository gate. No production key enters tests; no TLA+ or Lean artifact is required for this finite sequential workflow. |

## Modeling record

| Field           | Slice 2.3B record                                                                                                                                                                                                                                                                                                                                                                            |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem         | A release engineer must bind the exact supported remote Runtime bytes to one Desktop package without putting a production private key in the repository, asking a remote machine to download code, or allowing package-time substitution.                                                                                                                                                    |
| Inputs          | Trusted intent is one explicit Runtime semantic version, one externally governed Ed25519 signer, and one independently selected public key. Build outputs, prepared directories, signatures, manifests, artifacts, filesystem kinds, package environment variables, and absence of any required input are untrusted until validated.                                                         |
| Data            | Schema-version-1 unsigned and signed manifests contain the ordered two-platform artifact matrix. A schema-version-1 trust descriptor contains canonical SPKI, its derived signing-key identity, and the exact final-manifest digest. Prepared and final bundles have fixed paths and no optional entries.                                                                                    |
| Outputs         | Prepare either atomically publishes a signing payload plus artifacts or leaves its requested output absent. Finalize either atomically publishes the signed manifest plus artifacts or leaves its output absent. Release packaging either invokes Electron Packager with verified buffered resources or fails first. Ordinary packaging intentionally includes no SSH Runtime release.       |
| Invariants      | Both artifacts share one explicit Runtime version; every signed size and digest matches buffered bytes; the manifest and descriptor are canonical; the descriptor pins both signer identity and exact manifest bytes; a production private key is never an input; verified bytes, not later source-path reads, are staged; unknown filesystem content grants no authority.                   |
| Decisions       | D37-D46 fix ownership, targets, toolchain mode, canonical formats, private-key exclusion, trust binding, package placement, command separation, rollback, non-goals, and evidence. There is no runtime policy switch inside this slice.                                                                                                                                                      |
| Model           | A finite workflow owns `prepareSshRuntimeRelease`, external signing, `finalizeSshRuntimeRelease`, `readVerifiedSshRuntimeRelease`, and `stageSshRuntimeReleasePackageResources`. Canonical schema functions are deterministic data-contract operations.                                                                                                                                      |
| Transitions     | Each repository-owned workflow starts with an absent output, validates and buffers every input, writes only into a private candidate or staging directory, and publishes or invokes its next boundary only after all checks pass. It has no background state, retry, partial-success result, or automatic recovery.                                                                          |
| Dependencies    | Zig `0.16.0`, Node.js cryptography and filesystem APIs, an external Ed25519 signer or HSM, an owner-controlled release workspace, and Electron Packager. Network access and production signing credentials are not repository capabilities.                                                                                                                                                  |
| Ownership       | The protocol package owns encoding schemas; the Runtime workspace owns build parameters and prepared/final bundle construction; the external release authority owns the private key and signature act; Desktop owns public-key normalization, trust-descriptor construction, verification policy, private staging, and package placement.                                                    |
| Verification    | Contract unit tests cover canonical encodings and strict fields. Runtime Node tests cover fixed arguments, prepare/finalize, signatures, mutations, and layouts. Desktop verifier and package tests cover exact trust, wrong keys, changed bytes, noncanonical manifests, extra entries, symlinks, and buffered staging. Zig compiles both targets and runs on supported CI hosts.           |
| Correctness     | The claims are exact acceptance/rejection and phase ordering under the trust assumptions below. This is a bounded sequential workflow without concurrent state, retry, cancellation, or critical unbounded transformation, so example and integration tests are the selected evidence; TLA+ and Lean would not strengthen the claimed boundary in this slice.                                |
| Reference scope | The contract covers release schema `1`, trust schema `1`, Zig `0.16.0`, Node.js `>=22`, macOS arm64 and glibc Linux x86_64 Runtime artifacts, and Electron Packager resource placement. It does not claim bit-for-bit reproducibility across machines, Electron signing/notarization, hostile concurrent mutation of the owner-controlled workspace, or Runtime execution on another target. |

## Canonical artifacts and placement

Prepare emits exactly:

```text
prepared-release/
  artifacts/
    darwin-arm64/orchbee-ssh-environment-runtime
    linux-x64/orchbee-ssh-environment-runtime
  manifest-signing-payload.json
```

The signing payload has no trailing newline. Finalize accepts exactly one raw 64-byte Ed25519
signature and one matching public key, verifies them against those payload bytes, rechecks both
artifacts, and emits exactly:

```text
final-release/
  artifacts/
    darwin-arm64/orchbee-ssh-environment-runtime
    linux-x64/orchbee-ssh-environment-runtime
  manifest.json
```

The final manifest and trust descriptor each use their canonical property order and one trailing
newline. Release packaging creates this split:

```text
app.asar:/dist/main/ssh-environment-runtime-release-trust-v1.json
Resources/ssh-environment-runtime-release-v1/
  artifacts/
    darwin-arm64/orchbee-ssh-environment-runtime
    linux-x64/orchbee-ssh-environment-runtime
  manifest.json
```

The descriptor is application trust state and therefore remains inside `app.asar`. The signed
manifest and executable bytes must be real files outside `app.asar` so the later bootstrap can read
and stream the selected artifact. Desktop derives the canonical DER SPKI and signing-key identity
from the independent public-key file; the final release bundle does not supply its own trust root.

## Offline release workflow

All paths are absolute, all requested output directories must be absent, and the production signer
must run outside repository tooling. From the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-runtime release:prepare --output /absolute/prepared-release --runtime-version 1.2.3

# The external release authority signs the exact bytes in:
# /absolute/prepared-release/manifest-signing-payload.json
# It writes a raw 64-byte Ed25519 signature without exposing the private key to this process.

pnpm --filter @orchbee/ssh-environment-runtime release:finalize --prepared /absolute/prepared-release --signature /absolute/signature.bin --public-key /absolute/release-public-key.pem --output /absolute/final-release

ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY=/absolute/final-release \
ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY=/absolute/release-public-key.pem \
pnpm desktop:package:release
```

The public key passed to Desktop is an independent release input, not a file trusted from inside the
bundle. Missing variables, relative paths, a wrong key, a wrong signature, noncanonical JSON,
changed bytes, unsupported matrix, or expanded layout stops the release command before Electron
Packager. The ordinary command remains:

```sh
pnpm desktop:package
```

It does not read these release variables and does not include a Runtime bundle or descriptor.

## Trust, rollback, and privacy

The production release authority must authenticate the prepared payload, protect the private key,
and supply the intended public key to Desktop packaging through an access-controlled release job.
The workflow trusts the pinned Zig and Node toolchains, Node's SHA-256 and Ed25519 implementation,
the owner-controlled local release workspace, and Electron Packager's documented `asar` and
`extraResource` placement. Ordinary CI proves the mechanism only with ephemeral keys; it never
claims that a production signer approved a release.

There is no mutable `current` trust identity. A Desktop package contains one descriptor and one
matching bundle. Changing either the Runtime bytes/version or signing key requires producing and
shipping another Desktop package. Rolling back the application restores the older package's own
descriptor and bundle together; this slice performs no remote downgrade, cleanup, feed lookup, or
key rollover.

The manifest exposes Runtime version, supported platforms, sizes, and hashes. The descriptor
exposes a public key, its digest-derived identity, and the manifest digest. These are release
metadata, not secrets. Private keys, SSH credentials, Environment targets, usernames, remote paths,
and AskPass values are structurally absent and must not be added to either bundle.

## Verification and current limits

Run the focused evidence from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop test:release-package
pnpm --filter @orchbee/desktop typecheck
```

The macOS native CI leg runs the Runtime tests in addition to the two-target compilation performed
by the repository gate. A full release rehearsal may use a temporary Ed25519 key, but production
keys remain outside local and ordinary CI evidence.

Slice 2.3C now consumes this exact package boundary for installed-artifact negotiation and
process-local supervisor composition. The release bundle alone still cannot create or connect an
Environment, open a remote terminal, browse files, execute a generic command, operate a Project or
Workspace, run an Agent, install an operating-system service, or survive logout/reboot. Electron
application signing, notarization, installer creation, publication, and update distribution also
remain separate release work.
