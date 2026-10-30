# TUI 共享组件

本参考列出本 crate 的共享组件层：
[TUI 组件](../../../../../docs/zh-CN/tui/components.md#所有权与准入)所定义的基础控件和重复组合。
二者都位于[模块分层](module-layers.zh-CN.md#组件)契约的 `presentation` 层。

## 位置

| 层级     | 模块                   | 拥有                                                   |
| -------- | ---------------------- | ------------------------------------------------------ |
| 基础控件 | `tui_render::controls` | 单个控件的固定单元格几何、交互样式和已绘制矩形         |
| 重复组合 | `tui_render::modal`    | 浮层框架，以及基于基础控件的模态层次、布局、动作和光标 |

- 基础控件只引用更低的层。重复组合引用基础控件、布局几何和更低的层。二者都不引用业务表面、功能状态或
  `tui_render` 的其余部分。
- 组件绘制已准入的状态并返回已绘制的矩形。它们从不根据文案或字形推导意图、推断可用性或启动功能副作用；
  类型化目标、可用性和文案由功能所有者提供。
- 重复组合把绘制出的区域记录到帧命中图 `tui_pointer::hit_map`。区域携带各所有者绘制时的见证，所以
  命中图留在 `presentation`。控件状态类型位于 `values`，在 `tui_render::controls::control_state` 下。

## 基础控件

| 组件           | 条目                                                                                                                                                           |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 控件状态       | `ControlSurface`、`FocusState`、`PointerPresentation`、`NavigationState`、`CurrentState`、`CheckedState`、`Availability`、`OperationState`、`InteractionState` |
| 指针目标       | `paint_pointer_target`                                                                                                                                         |
| 动作           | `ActionKind`、`ActionSpec`、`action_rect`、`close_rect`、`render_action`                                                                                       |
| 图标动作       | `IconActionGlyph`、`IconActionSpec`、`icon_action_rect`、`frame_close_rect`、`icon_action_style`、`render_icon_action`、`render_frame_icon_action`             |
| 按键提示       | `KeyHint`、`measure_key_hints`、`render_key_hints`、`measure_overlay_hints`、`render_overlay_hints`                                                            |
| 查询输入框     | `QueryField`、`render_query_field`、`render_divider`                                                                                                           |
| 行交互         | `RowContentBand`、`RowFocusCue`、`RowTracks`、`paint_row_interaction`                                                                                          |
| 文本行与模式栏 | `text_row`、`mode_bar`                                                                                                                                         |

## 重复组合

| 组合     | 条目                                                                                                                  |
| -------- | --------------------------------------------------------------------------------------------------------------------- |
| 浮层框架 | `OverlayFrame`、`overlay_surface`、`titled_overlay`、`overlay_block`、`overlay_title`、`overlay_heading`              |
| 模态层次 | `paint_modal_frame`、`title_line`、`description_line`、`status_line`                                                  |
| 模态布局 | `ModalLayout`、`SingleModalLayout`、`modal_layout`、`single_modal_layout`、`wrapped_text_fits`、`unwrapped_text_fits` |
| 模态动作 | `ModalAction`、`pointer_presentation`、`paint_modal_action`、`paint_modal_close`、`modal_key_label`                   |
| 模态光标 | `set_modal_cursor`                                                                                                    |

## 维护

- 只有当第二个功能表面需要相同的几何与交互语义时，才把一种模式提升为基础控件或重复组合；在此之前它留在
  所属功能中。
- 直接修改共享组件，不要把它复制进功能模块。需要不同语义的功能在这里提出新的变体。
- 新增、重命名、移动或删除共享组件时，在同一变更中以两种语言更新本参考中对应的行。其渲染证据位于
  `tui_render/controls/tests.rs` 或 `tui_render/modal/tests.rs`。

<!-- documentation-pairing: english-sha256=dc68874f02373cc42575361565e6bcff21302a0f087bd17f9e71414aa3960b8a zh-cn-sha256=00a29d74ad16b26200961daebbdb4b42708a93a771a7c1841a4aee3270eba1c2 -->
