# TUI Shared Components

This reference inventories the crate's shared component layer: the primitives and recurring
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
- Hit-map registration still goes through `tui_pointer`. It leaves the components when the hit-map
  and interaction-projection types move to `values`.

## Primitives

| Component             | Items                                                                                                                                                          |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Control state         | `ControlSurface`, `FocusState`, `PointerPresentation`, `NavigationState`, `CurrentState`, `CheckedState`, `Availability`, `OperationState`, `InteractionState` |
| Pointer target        | `paint_pointer_target`                                                                                                                                         |
| Action                | `ActionKind`, `ActionSpec`, `action_rect`, `close_rect`, `render_action`                                                                                       |
| Icon action           | `IconActionGlyph`, `IconActionSpec`, `icon_action_rect`, `frame_close_rect`, `icon_action_style`, `render_icon_action`, `render_frame_icon_action`             |
| Key hints             | `KeyHint`, `measure_key_hints`, `render_key_hints`, `measure_overlay_hints`, `render_overlay_hints`                                                            |
| Query field           | `QueryField`, `render_query_field`, `render_divider`                                                                                                           |
| Row interaction       | `RowContentBand`, `RowFocusCue`, `RowTracks`, `paint_row_interaction`                                                                                          |
| Text row and mode bar | `text_row`, `mode_bar`                                                                                                                                         |

## Recurring compositions

| Composition     | Items                                                                                                                 |
| --------------- | --------------------------------------------------------------------------------------------------------------------- |
| Overlay frame   | `OverlayFrame`, `overlay_surface`, `titled_overlay`, `overlay_block`, `overlay_title`, `overlay_heading`              |
| Modal hierarchy | `paint_modal_frame`, `title_line`, `description_line`, `status_line`                                                  |
| Modal layout    | `ModalLayout`, `SingleModalLayout`, `modal_layout`, `single_modal_layout`, `wrapped_text_fits`, `unwrapped_text_fits` |
| Modal actions   | `ModalAction`, `pointer_presentation`, `paint_modal_action`, `paint_modal_close`, `modal_key_label`                   |
| Modal cursor    | `set_modal_cursor`                                                                                                    |

## Maintenance

- Promote a pattern into a primitive or composition only when a second feature surface needs the
  same geometry and interaction semantics; until then it stays with its feature.
- Change a shared component in place instead of copying it into a feature. A feature that needs
  different semantics proposes a new variant here.
- Add, rename, move, or remove a shared component together with its row in this reference, in
  both languages. Its rendering evidence lives in `tui_render/controls/tests.rs` or
  `tui_render/modal/tests.rs`.
