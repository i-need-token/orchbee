# TUI Shared Components

This reference places the crate's shared component layer: the primitives and recurring
compositions that
[TUI components](../../../../../docs/tui/components.md#ownership-and-admission) defines. Both sit
in the `presentation` layer of the [module layers](module-layers.md#components) contract.

## Placement

| Level                 | Module                 | Owns                                                                                       |
| --------------------- | ---------------------- | ------------------------------------------------------------------------------------------ |
| Primitive             | `tui_render::controls` | Fixed cell geometry, interaction styling, and the painted rectangle of one control         |
| Recurring composition | `tui_render::modal`    | The overlay frame and the modal hierarchy, layout, actions, and cursor built on primitives |

- A primitive references only lower layers. A composition references primitives, layout geometry,
  and lower layers. Neither references a business surface, feature state, or the rest of
  `tui_render`.
- Components paint admitted state and return painted rectangles. They never derive an intent from
  copy or glyphs, infer availability, or start a feature effect; the feature owner supplies the
  typed target, availability, and copy.
- Compositions record their painted regions into the frame hit map, `tui_hit_map`. It stays
  in `presentation` because regions carry their owners' painted witnesses. Control-state types sit in
  `values` under `tui_render::controls::control_state`.

## Maintenance

- Promote a pattern into a primitive or composition only when a second feature surface needs the
  same geometry and interaction semantics; until then it stays with its feature.
- Change a shared component in place instead of copying it into a feature. A feature that needs
  different semantics proposes a new variant here.
- A shared component's rendering evidence lives in `tui_render/controls/tests.rs` or
  `tui_render/modal/tests.rs`.
