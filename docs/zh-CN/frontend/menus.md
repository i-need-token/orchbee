# 前端菜单

## 归属

本规范治理所有可运行前端中的 Dropdown Menu、Context Menu、Menubar、子菜单及其触发器。

| 职责                                 | Owner                          |
| ------------------------------------ | ------------------------------ |
| 动作身份、可用性、顺序和调用         | 功能动作模型                   |
| 菜单角色、焦点移动、关闭和碰撞处理   | `@orchbee/ui` Primitive        |
| 触发器披露和所属行的交互状态         | 最窄的功能组件                 |
| 几何、图标尺寸、颜色、表面和焦点提示 | 仓库前端规范与 Token           |
| 产品 Label、可访问名称和快捷键展示   | `@orchbee/i18n` 与所属动作模型 |

当一个操作可从 Button、省略号菜单、Context Menu、快捷键或 Command Surface 调用时，使用
[`actions.md`](actions.md) 中的动作投影契约。功能代码不得为这些调用路径实现彼此平行的状态修改。

## 触发器与位置

- 每个纯图标触发器都要独立拥有可访问名称。需要可见动作提示时遵循
  [`tooltips.md`](tooltips.md)。
- 在左侧边栏中，加号或省略号触发器从其物理右侧打开锚定菜单，并让两者的 Block-start 边缘
  对齐。只有右侧无法容纳完整的菜单参考宽度与边缘间距时，才允许回退到触发器的 Block-end
  边缘，并让两者的 Inline-end 边缘对齐，使菜单落到左下方。碰撞处理仍可移动选定的位置以保持
  菜单表面可见。
- 在左侧边栏之外，加号或省略号触发器从其 Block-end 边缘打开锚定菜单。除非所属功能的当前
  权威明确要求 Inline-start 对齐，否则菜单与 Inline-end 边缘对齐。碰撞处理可以翻转或移动菜单
  表面以保持可见。
- Context Menu 在 Pointer 或长按位置打开。只有拥有产品 Context Menu 的触发区域才抑制平台菜单。
- 打开 Context Menu 不得激活、选择、展开或导航所属对象，除非该变化本身是显式动作。
- Hover、Focus 和 Open 的展示由 CSS 与 Primitive 状态属性驱动。不得在功能状态中镜像 Pointer
  Hover，也不得在关闭后遗留过期的 Open State Class。

| 触发器状态                            | 必需展示                                            |
| ------------------------------------- | --------------------------------------------------- |
| 省略号所属行处于默认状态              | 隐藏省略号                                          |
| 所属行 Hover 或焦点进入该行           | 行使用 Hover Surface；省略号以 Muted Icon Role 显示 |
| 省略号被直接 Hover、聚焦或菜单打开    | 省略号使用 Primary Icon Role                        |
| 省略号菜单或匹配的 Context Menu 打开  | 所属行和触发器持续保留 Open 展示，直到菜单关闭      |
| 带加号动作的空 Section                | 加号触发器持续显示                                  |
| 有内容的 Section 处于默认状态         | 隐藏加号触发器                                      |
| Pointer 或焦点位于有内容的 Section 内 | 显示加号触发器                                      |
| 加号菜单打开，包括 Pointer 移入菜单   | 加号触发器持续显示，直到菜单关闭                    |

## 内容与几何

- Dropdown、Context、Menubar 和子菜单表面使用 `--ob-ref-size-menu`（`250px`），只有可用视口
  更窄时才缩小。使用方不得分配一次性菜单宽度。
- 使用 `@orchbee/ui` 共享的 Overlay Surface、圆角、阴影、Padding 和碰撞行为；功能样式不得重画
  菜单容器。
- 从模态框打开的 Portaled Menu 通过共享的 Positioner Layer Override 参与
  `--ob-ref-layer-modal`。功能组件向 Primitive 传递 CSS Module Class；不得给 Popup 分配一次性的
  `z-index`。
- 每个可操作 Command 都带有前置语义 `ProductIcon`。只有 Command 没有独立动作隐喻时，Checkbox
  或 Radio 选择指示器才可以替代它。
- Group Label 和 Separator 是结构，不添加装饰性动作图标。
- Label 和可访问名称都要本地化。可见快捷键使用动作中按平台格式化的 Binding，不得把符号嵌入
  翻译文案。

| Slot                     | 尺寸                              | 契约                                          |
| ------------------------ | --------------------------------- | --------------------------------------------- |
| 前置 Command Icon        | `--ob-ref-size-icon-md`（`16px`） | Command 与 Submenu Trigger 使用同一个对齐槽位 |
| Label                    | Menu Foreground                   | Default、Hover 和 Active 状态之间不改变颜色   |
| Shortcut 或 Metadata     | 尾部对齐槽位                      | 默认使用 Muted；绝不替代动作的可访问名称      |
| Submenu Disclosure Arrow | `--ob-ref-size-icon-sm`（`14px`） | 只出现在尾部；不视为 Command 的语义图标       |

使用 Separator 表达有意义的分组；当破坏性 Command 的后果需要时，也要与其他命令分组。不要给单个
Menu Item 添加框线。

## 交互状态

| 状态                            | Surface 与 Foreground 契约                                    |
| ------------------------------- | ------------------------------------------------------------- |
| Default                         | 透明 Item Surface；普通 Label；Muted 前置图标                 |
| Hover、键盘 Highlight 或 Active | 共享交互 Surface；Label 不变；前置图标提升为 Label Foreground |
| 打开的 Submenu Trigger          | 与 Hover 或 Active 使用相同展示                               |
| Pointer 位于打开的 Submenu 内   | 父级 Submenu Trigger 持续保留 Open 展示                       |
| Disabled                        | Disabled Foreground；不提升 Hover 或 Pressed 状态             |
| Destructive 的默认与交互状态    | 使用共享 Primitive 的 Danger Role；绝不添加任意红色覆盖       |

- `ArrowUp` 和 `ArrowDown` 在可用 Item 之间移动；`Home` 和 `End` 移到第一个与最后一个可用 Item。
- `Enter` 或 `Space` 调用 Command 或打开 Submenu。逻辑前进与后退方向键按照书写方向打开和关闭
  Submenu。
- `Escape` 先关闭最深层的菜单。关闭根菜单后把焦点恢复到显式触发器；Context Menu 恢复焦点时不得
  改变所属对象的语义状态。
- Pointer 从 Submenu Trigger 移入 Submenu 时，不得因为触发器不再被物理 Hover 而清除父级高亮或
  关闭 Submenu。
- 关闭后焦点提示仍要完整可见；不得用任意 `z-index` 或局部 Focus Ring Offset 修补裁剪。

## 验证

- 单元覆盖固定菜单项顺序、分组、图标、可用性、破坏性处理，以及 `16px` 前置 / `14px` 尾部
  图标 Variant。
- 集成覆盖从省略号和 `contextmenu` 打开同一动作模型，验证右键没有导航副作用，并用 `Escape`
  关闭两条路径。
- Submenu 覆盖验证子菜单 Active 时，父级触发项持续保留 Open State Class。
- Trigger 覆盖验证空与有内容 Section 的披露、菜单打开期间的持续显示，以及关闭后的状态清理。
- 侧栏位置覆盖验证空间充足时从右侧打开，以及右侧空间受限时回退到左下方。
- 模态框覆盖验证 Portaled Menu 渲染在所属 Dialog 之上，并保持 Pointer 可操作。
- 键盘覆盖验证遍历、调用、嵌套关闭和焦点恢复。
- 修改菜单 Primitive、共享菜单模型、触发器行为或本规范后运行 `pnpm verify`。

<!-- documentation-pairing: english-sha256=4cd6cdd91cac8f47e12150e58d8fc4796bd427d43761e5f6c49ee04542debdb2 zh-cn-sha256=94b0b4bd8aeb8f1c99eaaf56f14a965c65185af907acc04ec261263930114874 -->
