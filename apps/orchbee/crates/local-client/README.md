# Orchbee local client

Private Client Connection transport for the [Orchbee native application](../../README.md).
Only that application and its sibling internal crates consume this crate.

## Production boundary

[`src/lib.rs`](src/lib.rs) lists the exported boundary. `LocalClient` admits a verified peer and
submits typed protocol exchanges with the caller's absolute deadline and optional cancellation.
Origins retain connection identity and freshness, create lanes, and admit requests and subscriptions.
Clients, origins, clocks, shutdown handles, subscriptions, and retained carriers keep their state
private. Typed receipts and failures preserve submission and outcome uncertainty.

Machine profiles, SSH process creation, runtime discovery, observation policy, CLI output, and
Workbench interaction stay with the application. The crate does not select features or replay effects.

## Module direction

The library root declares one implementation module, `local_client`, and inline facades that only
re-export it. `local_client` owns the connection core that its children `agent`, `cancellation`,
`carrier`, `multiplex`, `subscription`, and `workbench` share. Children use the parent's shared
items and never import each other, except `subscription -> multiplex`, because subscriptions share
the connection actor's cancellation and fresh-snapshot recovery. Reverse and other lateral imports
remain forbidden.

The whole crate is the transport beneath the Workbench Client's `workflow` layer, so it keeps this
lateral list instead of a [module-layer](../workbench-client/docs/module-layers.md) assignment.
`pnpm lint:rust-modules` reports any module cycle.

## Lifetime and evidence

The transport owns cancellation, multiplexing, connection health, queues, and lane shutdown.
`RetainedCarrier` takes an already-started byte relay's stop flag and join handle; its final owner
stops and joins that relay. Connection shutdown and logical lane shutdown remain separate operations.

Adjacent private tests retain admission, fragmentation, deadline, typed exchange, cancellation,
queue, correlation, health, and cleanup evidence. Raw connection constructors and chunk limits stay
with those tests.

The `test-support` feature exposes `test_support`: bounded fake Servers and multiplex peers,
deadline-free test admission, controlled transport clocks, a before-write gate, and wake marker
errors. Only development dependencies within the application enable it, directly or through the
[Workbench Client](../workbench-client/README.md) `test-support` feature, so release builds and the
production boundary never include it. Application and Workbench Client tests use it to drive real
admission and exchange paths; application code still consumes only the production boundary.

## Maintenance

The crate inherits the repository toolchain, lints, and shared dependency versions. Follow
[Rust ownership and testing rules](../../../../docs/RUST.md) and the
[repository verification gate](../../../../docs/VERIFICATION.md) from the repository root.
Keep exported operations tied to real application consumers and preserve private test ownership.
