# Frontend Experience Components

## Purpose and scope

This standard governs how browser and Desktop React interfaces reuse reviewed compositions. Common
patterns and their GUI/TUI correspondence follow [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md).

| Term                 | Contract                                                                                                                                            |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Primitive            | A domain-neutral framework component such as Dialog, Menu, Input, or Button that owns accessibility and low-level interaction                       |
| Experience component | A reviewed composition that owns a recurring interaction structure, lifecycle, hierarchy, or form pattern without owning one complete business task |
| Business component   | A reviewed product-semantic composition that owns a coherent business task, prepared state, and semantic actions                                    |

There is no catalog file. The components, their Stories and their tests are the record: search the
owning workspace's `src/` and `stories/` for an existing composition before writing a new one.

## Ownership and placement

Classification never overrides semantic ownership.

| Artifact                         | Required owner and location                                                                               |
| -------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Domain-neutral primitive         | `@orchbee/ui`, following [`package-boundaries.md`](package-boundaries.md)                                 |
| Domain-neutral experience module | `@orchbee/ui` only after a real consumer and an independently meaningful API justify promotion            |
| Product experience component     | The narrowest application or product-capability package that owns its semantic context                    |
| Business component               | The application or product-capability package that owns the business concept, prepared state, and Actions |
| Story                            | The owning workspace's top-level `stories/`, following [`storybook.md`](storybook.md)                     |

Do not create a repository-wide `experience`, `business-components`, `shared-components`, or
equivalent runtime bucket. Another workspace never deep-imports a private component. When another
owner needs the same semantic capability, apply the normal extraction and public-boundary decision
before moving the complete ownership unit.

## Composition constraints

- Reuse an existing composition when it fits the intent, through its supported boundary. Do not
  reconstruct its visual result from screenshots or copy private fragments into a new owner.
- When the existing composition is insufficient, extend its owner with the smallest coherent
  semantic capability and move every affected consumer. Keep a truly owner-specific difference at
  the narrower owner.
- A new user-visible structure, a replaced interaction, or a new variant that users can tell apart
  is a product decision: it needs the decision owner's acceptance under
  [`../DEVELOPMENT.md`](../DEVELOPMENT.md).
- Apply [`dialogs-and-forms.md`](dialogs-and-forms.md) before choosing an interrupting surface or
  collecting input. An existing Dialog composition does not prove modal admission or justify every
  field in it.
- Component APIs express product intent and controlled variants. Avoid arbitrary child topology,
  render-prop escape hatches, one-off slot graphs, or unrestricted style overrides that let each
  consumer redesign the component.
- Keep domain transitions and Actions authoritative outside visual primitives. An experience
  component coordinates presentation and lifecycle; it does not create parallel business writers.
- Reuse design tokens and shared primitives inside the owning component; never copy primitive
  geometry, focus behavior, localization, or accessibility contracts locally.
- Private components may be demonstrated through their public composition root; do not export a
  private component solely for Storybook.

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
