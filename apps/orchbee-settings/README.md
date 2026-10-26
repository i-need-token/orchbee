# Settings bridge

## Responsibility

`orchbee-settings --stdio-v1` consumes one bounded JSON request from closed standard input and
emits one JSON response. It neither starts a Session nor accepts a path or command from the request.
Desktop owns the exact executable identity, a two-second exchange deadline, cancellation and reaping.
The executable has no interactive mode or localized human output. Exit 0 accompanies a snapshot;
exit 1 accompanies a refusal, an unknown outcome or a failed output. Missing/malformed output is an unknown outcome,
never permission to replay a write.

The [shared preference contract](../../schemas/workbench/README.md) owns values and wire semantics.
[`orchbee-local-runtime`](../../crates/local-runtime/README.md) owns configuration-scoped
storage and native initial-source admission. Native Session calls that same owner directly.

The separate `--system-shell-v1` mode accepts exactly a closed empty JSON object and returns
one JSON executable path or null. It calls the shared native resolver without launching a shell.
Desktop resolves System shell per new terminal; custom profiles bypass discovery.

## Configuration scope

Scope is frozen from `ORCHBEE_CONFIG_FILE`, when explicitly provided, or the platform user
configuration location: `%APPDATA%/Orchbee/config.toml`,
`$XDG_CONFIG_HOME/Orchbee/config.toml` (falling back to `$HOME/.config`), or
`$HOME/Library/Application Support/Orchbee/config.toml` on macOS. A separate private directory is
derived from the configuration filename; two explicit files in one parent remain isolated.
Windows native-shell migration may resolve an explicitly configured executable using `PATH`;
it never executes that program. No Workspace, Session, remote device or browser origin is widened
into this device scope.

## Maintenance

Build with `cargo build --locked -p orchbee-settings`. Contract tests live with the shared protocol,
storage tests with the runtime owner, and process-boundary tests in this application's `tests/`.
Any request, value or failure change updates both language bindings and the shared conformance
fixtures. Desktop's build-settings-bridge.mjs builds the explicit native Rust host target in release mode,
stages the executable and SHA-256 manifest under out/settings-bridge, and packaging copies that
complete resource directory. Development compilation builds the same matching artifact.
