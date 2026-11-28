# Frontend Menus

## Ownership

This standard governs Dropdown Menus, Context Menus, Menubars, submenus, and their triggers across
runnable frontends.

| Responsibility                                                | Owner                                       |
| ------------------------------------------------------------- | ------------------------------------------- |
| Action identity, availability, ordering, and invocation       | The feature action model                    |
| Menu roles, focus movement, dismissal, and collision handling | `@orchbee/ui` primitives                    |
| Trigger disclosure and owning-row interaction state           | The narrowest feature component             |
| Geometry, icon sizes, colors, surfaces, and focus indication  | Repository frontend standards and tokens    |
| Product labels, accessible names, and shortcut presentation   | `@orchbee/i18n` and the owning action model |

Use the action projection contract in [`actions.md`](actions.md) when one operation is available
from a button, an ellipsis menu, a Context Menu, a shortcut, or a command surface. Feature code does
not implement parallel mutations for those invocation paths.

## Triggers and placement

- Give every icon-only trigger an accessible name independently of its Tooltip. Follow
  [`tooltips.md`](tooltips.md) when a visible action hint is useful.
- In a left sidebar, an add or ellipsis trigger opens its anchored menu from the trigger's physical
  right edge with their block-start edges aligned. Only when the full reference menu width and edge
  clearance cannot fit on the right may it fall back to the trigger's block-end edge with their
  inline-end edges aligned, placing the surface toward the lower left. Collision handling may
  still shift the selected placement to keep the surface visible.
- Outside a left sidebar, an add or ellipsis trigger opens its anchored menu from the trigger's
  block-end edge. It aligns to the inline-end edge unless the owning feature's current authority
  explicitly requires inline-start alignment. Collision handling may flip or shift the surface to
  keep it visible.
- A Context Menu opens at the pointer or long-press location. Suppress the platform menu only
  inside a trigger that owns a product Context Menu.
- Opening a Context Menu does not activate, select, expand, or navigate the owning object unless
  that change is an explicit action.
- Drive hover, focus, and open presentation from CSS and primitive state attributes. Do not mirror
  pointer hover in feature state or leave a stale open-state class after dismissal.

| Trigger state                                     | Required presentation                                                       |
| ------------------------------------------------- | --------------------------------------------------------------------------- |
| Ellipsis row at rest                              | Ellipsis hidden                                                             |
| Owning row hovered or focus enters the row        | Row uses its hover surface; ellipsis appears with the muted icon role       |
| Ellipsis directly hovered, focused, or menu open  | Ellipsis uses the primary icon role                                         |
| Ellipsis or matching Context Menu open            | Owning row and trigger retain their open presentation until the menu closes |
| Empty section with an add action                  | Add trigger remains visible                                                 |
| Populated section at rest                         | Add trigger hidden                                                          |
| Pointer or focus within the populated section     | Add trigger visible                                                         |
| Add menu open, including pointer movement into it | Add trigger remains visible until the menu closes                           |

## Content and geometry

- Dropdown, Context, Menubar, and submenu surfaces use `--ob-ref-size-menu` (`250px`) and shrink
  only when the available viewport is narrower. Consumers do not assign one-off menu widths.
- A menu that lists a whole catalog heads it with `MenuSearch` from `@orchbee/ui`, which holds the
  keyboard and scrolls the rows at `--ob-ref-size-menu-list`.
- Use the shared overlay surface, radius, shadow, padding, and collision behavior from
  `@orchbee/ui`; feature styles do not redraw the menu container.
- A portaled menu opened from a modal, or from a surface on `--ob-ref-layer-overlay`, participates
  in `--ob-ref-layer-modal` through the shared positioner-layer override. The feature passes a CSS
  Module class to the primitive; it does not assign a one-off `z-index` to the popup.
- Every actionable command has a leading semantic `ProductIcon`. A checkbox or radio selection
  indicator may replace it only when the command has no separate action metaphor.
- Group labels and separators are structural and do not receive decorative action icons.
- A context menu holds only the frequent Actions of its target; lower-frequency Actions stay
  discoverable in Action search.
- A File Bar or file overlay menu instead follows its VS Code counterpart: the items VS Code shows
  there, in VS Code's groups and order, with a leading icon only where VS Code shows one. A parity
  test holds the menu to VS Code at a pinned commit and names each VS Code item the menu leaves out,
  with the reason. The other rules here still apply.
- When one action model has a dropdown trigger and a context-menu trigger, both menus show the same
  entries in the same order, and a destructive entry asks for the same confirmation by pointer and
  by keyboard.
- Localize labels and accessible names. A visible shortcut uses the action's platform-formatted
  binding rather than a glyph embedded in translated copy.

| Slot                     | Size                             | Contract                                                        |
| ------------------------ | -------------------------------- | --------------------------------------------------------------- |
| Leading command icon     | `--ob-ref-size-icon-md` (`16px`) | Same aligned slot for commands and submenu triggers             |
| Label                    | Menu foreground                  | Does not change color between default, hover, and active states |
| Shortcut or metadata     | Trailing aligned slot            | Muted at rest; never replaces the accessible action name        |
| Submenu disclosure arrow | `--ob-ref-size-icon-sm` (`14px`) | Trailing only; not treated as the command's semantic icon       |

Use separators to express meaningful groups, including separation of destructive commands when
their consequence requires it. Do not add lines around individual menu items.

## Interaction states

| State                                      | Surface and foreground contract                                                        |
| ------------------------------------------ | -------------------------------------------------------------------------------------- |
| Default                                    | Transparent item surface; normal label; muted leading icon                             |
| Hover, keyboard highlight, or active       | Shared interaction surface; label unchanged; leading icon promoted to label foreground |
| Open submenu trigger                       | Same presentation as hover or active                                                   |
| Pointer inside an open submenu             | Parent submenu trigger retains its open presentation                                   |
| Disabled                                   | Disabled foreground; no hover or pressed promotion                                     |
| Destructive default and interaction states | Danger roles from the shared primitive; never arbitrary red overrides                  |

- `ArrowUp` and `ArrowDown` move between enabled items; `Home` and `End` move to the first and last
  enabled items.
- `Enter` or `Space` invokes a command or opens a submenu. Logical forward and backward arrow keys
  open and close submenus according to writing direction.
- `Escape` closes the deepest open menu first. Closing the root menu restores focus to an explicit
  trigger; a Context Menu restores focus without changing the owning object's semantic state. A trigger revealed only by hover cannot take restored focus once the pointer has left it, so the owning row's always-visible focus target takes it instead.
- Pointer movement from a submenu trigger into its submenu does not clear the parent highlight or
  close the submenu because the trigger is no longer physically hovered.
- Focus indication remains fully visible after dismissal; do not repair clipping with arbitrary
  `z-index` or local focus-ring offsets.

## Verification

A menu change keeps these outcomes; [`TESTING.md`](../TESTING.md) decides the tests.

- One action model serves the ellipsis trigger and `contextmenu`, and right-click has no navigation
  side effect.
- The keyboard alone traverses, invokes, and dismisses; `Escape` closes each path and nested menus
  one level at a time; focus returns to the trigger.
- A parent trigger keeps its open state while its submenu is active, and a section disclosure keeps
  its open menu until dismissal and cleans up after it.
- A menu opens fully visible: on the right in the sidebar when it fits and at the lower left
  otherwise, and above an owning dialog, where it stays pointer operable.
