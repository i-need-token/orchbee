# Browser Storage

## Scope

Browser storage is a device-local persistence boundary, not a general database. Use it only for small, non-sensitive interface preferences or recoverable layout state whose loss does not affect authoritative product behavior.

This standard governs `localStorage`, `sessionStorage`, their keys and codecs, and any storage capability with the same synchronous string interface. IndexedDB, filesystem storage, remote persistence, caches, credentials, and authoritative domain records require their own owner and contract.

## Storage selection

| Requirement                                       | Storage boundary                   |
| ------------------------------------------------- | ---------------------------------- |
| Durable device-local preference across restarts   | `localStorage`                     |
| Temporary value for one browser-tab lifetime      | `sessionStorage`                   |
| Structured, large, transactional, or queryable    | IndexedDB or an owned data service |
| Cross-device or authoritative product state       | An owned backend capability        |
| Credential, token, secret, or terminal transcript | Never browser key-value storage    |

Do not choose browser storage merely because it is convenient. The feature owner documents why the value is local, disposable, bounded, and safe to expose to same-origin script.

## Ownership boundary

- The feature or package that owns the persisted concept owns its key, schema version, codec, validation, fallback, migration, and removal lifecycle.
- Define each key once in a responsibility-focused `*-storage.ts` owner module. Components, Hooks, tests, and callers do not repeat the literal.
- Only an application composition entry reads the browser's `localStorage` or `sessionStorage` global. It injects the narrow `getItem` and `setItem` capability into the feature owner.
- Presentational components never access browser storage. The owning client state or custom Hook may coordinate the injected capability and expose prepared values or semantic actions.
- Keep keys distributed with their feature owners. Do not create a package-wide miscellaneous key registry that separates a key from the schema and lifecycle it identifies.

## Key contract

Every persistent Orchbee key uses this exact shape:

```text
orchbee:<owner>:<record>:v<schema>
```

- `orchbee` is the fixed product namespace.
- `<owner>` is the kebab-case workspace or feature owner.
- `<record>` is a kebab-case persisted concept, not a component implementation name.
- `<schema>` is a positive integer. During the current unpublished phase, retain only the current shape at `v1`; incompatible changes after release require an explicit version and data-lifecycle decision.
- Use colons only as segment separators. Do not add environment, locale, user-provided data, timestamps, or opaque identifiers without an explicit partitioning requirement.

```text
orchbee:editor:panel-width:v1       # valid neutral example
editor-panel-width                  # missing namespace and version
orchbee:Editor:panelWidth:v1        # invalid casing
orchbee:editor:panel-width:latest   # version is not an integer
```

Before release, retain no old-key readers or migration. Strict validation must reject development leftovers outside the current shape, using the owner's documented defaults and issue reporting. If an old `:v1` shape would still pass current validation, change the `<record>` name to prevent accidental reuse; do not automatically delete old data. Never silently reinterpret an old payload under an unchanged key.

## Value contract

- Store every value as the JSON string produced by `JSON.stringify` from an object, even for one field. Do not store ad hoc raw strings, numbers, booleans, or comma-delimited values.
- `JSON.parse` returns `any`; deserialization does not establish runtime type safety. Pass every `JSON.parse(...)` result directly to the feature-owned Zod schema before accessing a field.
- Every browser-storage codec owns a strict Zod object schema beside its key and codec. Use `parse` or `safeParse` to reject missing fields, unexpected fields, invalid numbers, unsupported enum values, and values outside current product constraints.
- TypeScript declarations, type assertions, handwritten type guards, generated serializers, and knowledge that Orchbee originally wrote the value do not validate data recovered from storage and do not replace Zod parsing.
- Persist stable domain units. A pixel layout preference stores an integer pixel value; it does not store a viewport-dependent percentage and relabel it later.
- Do not serialize React state, class instances, services, callbacks, translated copy, or third-party internal data shapes.
- Keep payloads small and bounded. Browser quota is not a capacity budget.

## Read and write lifecycle

- Never read browser storage during module evaluation.
- Resolve an injected storage capability at the client composition boundary. Server rendering and tests use the feature's deterministic default unless they explicitly inject a fake capability.
- Read once when the owning client state is initialized. Do not read on every render.
- Write only after a committed semantic change. For drag interactions, persist after pointer release or the equivalent completed keyboard action, not on every movement event.
- Do not write default values merely because a component mounted. A stored record should indicate an actual user choice or an intentional migration.
- A storage event from another tab is not automatically authoritative. Opt into cross-tab synchronization only when the feature contract defines conflict and ordering behavior.

## Failure and privacy

- Treat missing, malformed, obsolete, unavailable, and quota-rejected storage as expected boundary failures. Fall back safely without blocking application startup.
- A composition fallback never converts unavailable durable storage into a successful durable write. Owners that distinguish saved from session-only behavior must observe the read or write failure through their injected capability.
- A failed device-local preference write may be ignored when the current in-memory behavior remains correct. Product-critical persistence requires a different capability with visible diagnostics.
- Never store passwords, access tokens, API keys, authentication artifacts, private terminal output, agent transcripts, or authoritative session state in browser key-value storage.
- Assume same-origin script and browser tooling can read every stored value. Obfuscation is not encryption.
- Logout and account switching must not leak account-scoped values. Prefer not to persist account-scoped data locally; otherwise include an explicitly validated partition and cleanup lifecycle.

## Verification

- `pnpm lint:storage` rejects direct browser-storage access outside application composition entries, scattered `orchbee:` literals, keys that do not match the versioned grammar, and storage deserialization that does not pass `JSON.parse(...)` directly to Zod.
- Owner-module unit tests cover the canonical key, valid round trips, malformed input, range validation, unavailable storage, and write failures.
- Integration tests verify capability injection, default restoration, and that writes occur only after a completed user interaction.
- Update the owning package documentation whenever a public preference, default, constraint, key version, or migration behavior changes.
