# Workbench Cross-Language Contract

## Authority

[`v1/workbench.schema.json`](v1/workbench.schema.json) is the language-neutral authority for the
version-1 read-only Workbench projection kernel. The TypeScript binding in
`@orchbee/workbench-client-protocol` and the Rust binding in `orchbee-workbench-protocol` are
independent implementations. Neither binding may add accepted values, omit invariants, or redefine
field meaning.

Version 1 uses decoded JSON values governed by JSON Schema 2020-12. Every object is closed, every
collection and string is bounded, identities are typed non-zero 128-bit lowercase hexadecimal
strings, and revisions are canonical unsigned-64 decimal strings. A transport must reject an
encoded intent larger than 16 KiB or an encoded snapshot or delta larger than 1 MiB before JSON
decoding. Transport framing, duplicate-key handling, authentication, authorization, redaction, and
version negotiation remain outside this slice.

[`v1/workbench-conformance-v1.json`](v1/workbench-conformance-v1.json) is the contract-authored
cross-language oracle. Valid and invalid documents exercise strict parsing and semantic invariants;
transition cases exercise exact delta results and typed rejections. An implementation must consume
every case without rewriting the expected result from its own parser or reducer.

## Projection

A `workbench.snapshot` is a complete authorized product projection for one Site revision. It
contains:

- ordered Projects and their ordered Workspaces;
- exactly one standalone Terminal Context and one Terminal Context for every Workspace;
- ordered Tab groups and Tabs;
- complete binary Panel/Split topology for every Tab;
- one primary and zero or more child Terminal Presentations per Panel; and
- an optional opaque Worker/Terminal binding for a Presentation.

The projection contains labels but no directories, paths, terminal content, terminal lifecycle
policy, Environment, Checkout, Worktree, Agent, Launch Profile, Archive, SSH, lease, fence, Host
epoch, attachment, or persistence facts. It also contains no Client-local active location, active
Tab, focus, disclosure, selection, maximization, split weight, viewport, geometry, search, window,
DOM, React, or pixel state. A missing Presentation binding means only that this projection exposes
no attachable Terminal identity; a Client must not infer a lifecycle reason.

Project, Workspace, and Terminal Context arrays use ascending identity order. Tab groups and Tabs
retain their explicit product order. Pinned Tabs form one leading ungrouped prefix, and every Tab
group's members are contiguous. Every referenced context exists exactly once and every context is
referenced exactly once. Identity uniqueness, tree depth, node quantity, membership, and reference
invariants apply to the complete snapshot in addition to JSON Schema validation.

## Intent and delta

`workbench.snapshot.read` is the only admitted version-1 semantic intent. It requests the complete
authorized projection and owns no transport, response envelope, pagination, command-line grammar,
or mutation behavior.

A `workbench.delta` names one exact Site, base revision, greater result revision, and an unordered
atomic set of aggregate changes. A change may replace complete navigation, upsert one complete
Terminal Context, or remove one complete Terminal Context. At most one navigation replacement and
one change for each Terminal Context identity are allowed. An empty change set may advance the
projection revision when authorized Site facts changed outside this projection.

`applyWorkbenchDelta` performs these steps without side effects:

1. reject a Site mismatch, base-revision mismatch, or non-increasing result revision;
2. reject duplicate navigation or Terminal Context targets;
3. stage every aggregate replacement and sort Terminal Contexts by identity;
4. validate the complete candidate snapshot; and
5. return the complete next snapshot only after every check succeeds.

Any rejection leaves the caller's snapshot unchanged. Recovery obtains a complete snapshot; this
slice defines no replay buffer, retry, persistence, or transport request.

## Limits

| Value                     | Version-1 limit            |
| ------------------------- | -------------------------- |
| Encoded read intent       | 16 KiB                     |
| Encoded snapshot or delta | 1 MiB                      |
| Product label             | 512 Unicode code points    |
| Projects                  | 256 per snapshot           |
| Workspaces                | 256 per Project            |
| Terminal Contexts         | 1,024 per snapshot         |
| Tab groups                | 64 per Terminal Context    |
| Tabs                      | 256 per Terminal Context   |
| Child Presentations       | 16 per Panel               |
| Panel/Split depth         | 16 nodes from a Tab root   |
| Panel/Split nodes         | 4,096 per Terminal Context |
| Delta changes             | 1,024 per delta            |

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol test
cargo test -p orchbee-workbench-protocol --locked
pnpm verify
```

These checks prove schema/binding conformance and the pure delta refinement only. They do not prove
a Server, transport, authorization policy, persistence model, renderer, CLI command, or mobile
implementation.
