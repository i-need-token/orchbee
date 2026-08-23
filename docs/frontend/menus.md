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
- An add or ellipsis trigger opens its anchored menu from the trigger's block-end edge. It aligns
  to the inline-end edge unless the owning feature's current authority explicitly requires
  inline-start alignment. Collision handling may flip or shift the surface to keep it visible.
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
- Use the shared overlay surface, radius, shadow, padding, and collision behavior from
  `@orchbee/ui`; feature styles do not redraw the menu container.
- Every actionable command has a leading semantic `ProductIcon`. A checkbox or radio selection
  indicator may replace it only when the command has no separate action metaphor.
- Group labels and separators are structural and do not receive decorative action icons.
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
  trigger; a Context Menu restores focus without changing the owning object's semantic state.
- Pointer movement from a submenu trigger into its submenu does not clear the parent highlight or
  close the submenu because the trigger is no longer physically hovered.
- Focus indication remains fully visible after dismissal; do not repair clipping with arbitrary
  `z-index` or local focus-ring offsets.

## Verification

- Unit coverage fixes menu entry order, grouping, icons, availability, destructive treatment, and
  the `16px` leading / `14px` trailing icon variants.
- Integration coverage opens the same action model from ellipsis and `contextmenu`, verifies that
  right-click has no navigation side effect, and closes each path with `Escape`.
- Submenu coverage verifies that the parent trigger keeps its open-state classes while the submenu
  is active.
- Trigger coverage verifies empty and populated section disclosure, open-menu persistence, and
  cleanup after dismissal.
- Keyboard coverage verifies traversal, invocation, nested dismissal, and focus restoration.
- Run `pnpm verify` after changing a menu primitive, shared menu model, trigger behavior, or this
  standard.
