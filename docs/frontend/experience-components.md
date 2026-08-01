# Frontend Experience Components

## Purpose and scope

This standard makes reviewed product-interface knowledge discoverable before implementation and
constrains high-complexity UI work to deliberate, repeatable compositions. It governs browser and
desktop-facing React interfaces across applications and packages. It does not replace primitive,
React, package-boundary, Storybook, localization, Action, or product-domain authorities.

| Term                 | Contract                                                                                                                                            |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Primitive            | A domain-neutral framework component such as Dialog, Menu, Input, or Button that owns accessibility and low-level interaction                       |
| Experience component | A reviewed composition that owns a recurring interaction structure, lifecycle, hierarchy, or form pattern without owning one complete business task |
| Business component   | A reviewed product-semantic composition that owns a coherent business task, prepared state, and semantic actions                                    |
| Manifest             | The package-local discovery record that summarizes one experience or business component and points to its evidence and implementation               |
| Canonical component  | The default reviewed component for its declared intents and supported scope                                                                         |

The catalog is selective. Do not register every primitive, leaf component, layout wrapper, or
private implementation fragment. Register a component when its reviewed structure is useful to a
future interface decision and accidental recomposition would plausibly create inconsistency.

## Admission signals

Run the [discovery workflow](#discovery-workflow) before implementation when a proposed interface
has any of these signals:

- combines several primitives into one user-visible task or stateful surface;
- opens a Dialog, Menu, Context Menu, Popover, Drawer, Command Surface, or nested overlay;
- owns confirmation, cancellation, validation, loading, failure, keyboard, focus-restoration, or
  open-state behavior;
- repeats an established row, resource editor, navigation hierarchy, search, picker, inspector, or
  management workflow;
- coordinates domain state or semantic Actions across multiple controls; or
- introduces enough visible elements that their hierarchy, ordering, spacing, or responsive
  behavior requires design judgment.

A single primitive with authority-determined copy and no additional lifecycle may remain local and
does not require a Manifest. Apparent implementation size does not waive discovery when one of the
signals applies.

## Ownership and placement

Classification never overrides semantic ownership.

| Artifact                          | Required owner and location                                                                                                                               |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Domain-neutral primitive          | `@orchbee/ui`, following [`package-boundaries.md`](package-boundaries.md)                                                                                 |
| Domain-neutral experience module  | `@orchbee/ui` only after a real consumer and an independently meaningful API justify promotion                                                            |
| Product experience component      | The narrowest application or product-capability package that owns its semantic context                                                                    |
| Business component                | The application or product-capability package that owns the business concept, prepared state, and Actions                                                 |
| Manifest                          | `<owner>/docs/experience/<component-name>.json`                                                                                                           |
| Story                             | The owning workspace's top-level `stories/`, following [`storybook.md`](storybook.md)                                                                     |
| Cross-repository schema and query | [`schemas/experience-component.schema.json`](../../schemas/experience-component.schema.json) and `scripts/experience-catalog.mjs`; they own no runtime UI |

Do not create a repository-wide `experience`, `business-components`, `shared-components`, or
equivalent runtime bucket. A Manifest makes knowledge centrally queryable without moving source
out of its owner. Private visibility remains private: another workspace must never deep-import a
private component merely because its Manifest is discoverable. When another owner needs the same
semantic capability, apply the normal extraction and public-boundary decision before moving the
complete ownership unit.

Package-local Manifest records are English current-authority references. They remain with their
owning package and therefore do not enter the repository-wide Chinese standards mirror.

## Discovery workflow

Before composing an admitted interface:

1. Describe the user intent and the semantic owner without naming an implementation.
2. Retrieve the compact catalog with `pnpm experience:list` or narrow it deterministically with
   `pnpm experience:find`. Prefer exact `--intent`, `--kind`, `--owner`, and `--status` filters;
   free terms use lexical matching and never vector similarity.
3. Run `pnpm experience:show <id>` only for plausible matches, then inspect the referenced Story,
   use and avoid conditions, invariants, dependencies, source, and owning references.
4. Select exactly one disposition: **reuse**, **extend**, **replace**, or **new**.
5. Apply the selected component through its supported boundary. Do not reconstruct its visual
   result from screenshots or copy private fragments into a new owner.

| Disposition | Required treatment                                                                                                                                                            |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reuse       | Use the canonical component and its semantic API without adding a local visual fork                                                                                           |
| Extend      | Add the smallest coherent semantic capability to the existing owner; update its Manifest, Stories, tests, references, and consumers                                           |
| Replace     | Obtain decision-owner acceptance for the changed interaction, migrate consumers, mark the old Manifest deprecated with `deprecatedBy`, and remove it when no consumer remains |
| New         | Confirm no admitted component satisfies the intent, obtain acceptance for the new visible structure, implement it at the narrowest owner, and register it in the same change  |

Discovery is progressive disclosure, not RAG. The compact summary lets an agent reason over the
available vocabulary; only a plausible component expands into detailed context. Do not load all
source or all Stories before choosing a candidate.

## Composition constraints

- Prefer a canonical experience or business component over a fresh composition when its declared
  `useWhen` conditions match and no `avoidWhen` condition applies.
- Do not directly assemble an admitted workflow from raw Dialog, Menu, Form, Overlay, or navigation
  primitives until discovery and the reuse/extend/replace/new disposition are complete.
- A new admitted composition is incomplete until its Manifest and evidence are present. Do not
  hide an unreviewed product pattern behind a generic local component name.
- Component APIs express product intent and controlled variants. Avoid arbitrary child topology,
  render-prop escape hatches, one-off slot graphs, or unrestricted style overrides that let each
  consumer redesign the component.
- Keep domain transitions and Actions authoritative outside visual primitives. An experience
  component coordinates presentation and lifecycle; it does not create parallel business writers.
- Reuse design tokens and shared primitives inside the owning component. A Manifest does not permit
  local copies of primitive geometry, focus behavior, localization, or accessibility contracts.
- When the existing canonical design is insufficient, challenge the existing owner first. Improve
  all affected consumers when the better design is general; keep a truly owner-specific difference
  at the narrower owner.

## Direct reorder vocabulary

- A directly sortable ordered collection uses squeeze motion on its ordering axis: the dragged item
  follows the pointer, and crossing a sibling's midpoint animates every intervening sibling into the
  vacated slot. Crossing back reverses the displacement.
- Vertical collections constrain sorting motion to the block axis; horizontal collections constrain
  it to the inline axis. Sorting must not introduce scrolling or overflow on the cross axis.
- Pointer cancellation and Escape restore the pre-drag order and disclosure state. Release completes
  the order through the owning semantic transition or Action. Completion must not replay the prior
  displacement transform against the committed layout.
- Do not replace squeeze motion with an insertion marker for same-container linear ordering. An
  insertion marker requires a reviewed exception when displacement cannot express the destination,
  such as cross-container transfer or a nested tree position.
- A canonical sortable experience records pointer-following, midpoint crossing, sibling
  displacement, reversal, completion, and cancellation in its Manifest, Story evidence, and tests.

## Manifest contract

Each Manifest conforms to
[`schemas/experience-component.schema.json`](../../schemas/experience-component.schema.json).

| Field            | Meaning                                                                                        |
| ---------------- | ---------------------------------------------------------------------------------------------- |
| `$schema`        | Relative path to the repository Manifest schema                                                |
| `id`             | Stable repository-wide identifier, namespaced by capability                                    |
| `kind`           | `experience` or `business`                                                                     |
| `status`         | Lifecycle state defined below                                                                  |
| `name`           | Short human-readable name                                                                      |
| `description`    | Compact retrieval summary that explains the owned interaction or task                          |
| `owner`          | Owning workspace package name                                                                  |
| `visibility`     | `private` or `public`; discovery never widens source visibility                                |
| `intents`        | Stable kebab-case user or implementation intents used by deterministic lookup                  |
| `implementation` | Existing source roots that own the component                                                   |
| `stories`        | Story file, named export, stable generated Story ID, and the states or interactions to inspect |
| `uses`           | Other registered components composed by this component                                         |
| `useWhen`        | Conditions under which this component is the intended choice                                   |
| `avoidWhen`      | Conditions that require another component or a new decision                                    |
| `invariants`     | Interaction, state, accessibility, ownership, or effect rules that reuse must preserve         |
| `references`     | Current-authority package references needed for deeper understanding                           |
| `deprecatedBy`   | Required replacement ID for a deprecated Manifest; forbidden for other states                  |

Paths are repository-relative and stay inside the owning workspace. Component IDs are unique.
Every `uses` and `deprecatedBy` ID resolves in the catalog, and the `uses` composition graph remains
acyclic. A canonical component always references at least one deterministic Story.

## Lifecycle and review

| Status       | Meaning and allowed use                                                                                             |
| ------------ | ------------------------------------------------------------------------------------------------------------------- |
| `candidate`  | Implemented exploration with an explicit owner; discoverable for evaluation but never the default for new consumers |
| `reviewed`   | Accepted for its stated use conditions but not yet the repository's default vocabulary for that intent              |
| `canonical`  | Reviewed default for matching intents and scope; new consumers reuse or explicitly justify another disposition      |
| `deprecated` | Retained only while consumers migrate; `deprecatedBy` identifies the supported replacement                          |

The decision owner accepts user-visible structure, supported variants, invariants, and canonical
status under [`../DEVELOPMENT.md`](../DEVELOPMENT.md). Implementation may choose reversible private
names and helper placement. A status change, materially broader use condition, new escape hatch,
or changed invariant is a governed behavior or boundary change and updates all affected evidence
and consumers in the same change.

## Story and evidence contract

- A canonical Manifest references a deterministic Story owned by the same workspace. Its `states`
  explain which state or interaction provides the relevant evidence.
- Private components may be demonstrated through their public composition root; do not export a
  private component solely for Storybook.
- A Story is inspection evidence, not the only correctness model. Domain transitions, cancellation,
  async behavior, Actions, and invariants retain their owned unit, integration, model, or property
  tests.
- Update Story IDs and Manifest references atomically. Story IDs are automation interfaces under
  [`storybook.md`](storybook.md).
- Credentials, live services, current time, randomness, and machine-specific state never become
  catalog prerequisites.

## Verification

Run from the repository root:

```sh
pnpm experience:list
pnpm experience:find --intent manage-resource
pnpm experience:show workbench.project-manager-dialog
pnpm lint:experience
pnpm test:experience-contract
pnpm storybook:build
pnpm verify
```

`lint:experience` validates schema freshness, package ownership, path existence, named Story exports
and generated IDs, unique component IDs, and catalog relationships. The query commands sort by ID
and return deterministic results. The root `lint`, `test`, and `verify` gates include the catalog
checks.
