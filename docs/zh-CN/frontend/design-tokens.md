# 前端设计 Token

## 所有权和使用

`@orchbee/design-tokens` 拥有 Orchbee CSS 界面共享的可枚举视觉决策。其权威 CSS 注册表是 `packages/design-tokens/src/tokens.css`；`packages/design-tokens/src/design-token-names.generated.ts` 是供包测试与仓库样式工具使用的内部 TypeScript 清单，不是产品 JavaScript API。包测试和 `pnpm lint:styles` 会拒绝过期的生成输出。

GUI/TUI 共同角色与主题对应关系遵循 [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md)。本注册表
实现 GUI 映射；原生单元格主题按 [`tui/visual-foundations.md`](../tui/visual-foundations.md)
保留自己的表示形式。

在自有组件样式之前导入一次 `@orchbee/design-tokens/tokens.css`。已经导入 `@orchbee/ui/styles.css` 的应用会通过 UI 包的样式入口获得注册表，不得再次导入。

CSS 语法不是设计决策。`grid`、`auto`、`hidden`、`currentColor` 和 `none` 等关键字，零值、相对百分比、网格分数和结构性整数计数保持为字面量。颜色、非零长度、排版值、圆角、边框、阴影、不透明度、层级和动效值使用已注册 Token。

本文档定义 Token 架构和强制执行规则。针对产品场景选择正确的排版、文本层级、间距、圆角、控件尺寸、图标尺寸、边框和表面变量时，使用 [`visual-foundations.md`](visual-foundations.md)。

## Token 层级

使用语法 `--ob-<tier>-<category>-<role>[-<variant-or-state>]`，其中每一段都是小写 kebab-case。

| 层级   | 名称前缀      | 职责                                               | 依赖方向             |
| ------ | ------------- | -------------------------------------------------- | -------------------- |
| `ref`  | `--ob-ref-*`  | 具体调色板、间距、排版、圆角、阴影、动效和层级尺度 | 仅具体 CSS 值        |
| `sys`  | `--ob-sys-*`  | 主题感知的界面含义，例如表面、文本、操作和状态     | `ref` Token          |
| `comp` | `--ob-comp-*` | 有意公开的组件样式或运行时几何槽位                 | `ref` 或 `sys` Token |

按稳定角色命名 Token，不要按当前值命名：

```css
/* ❌ 按值命名且未注册的局部变量。 */
--gray-200: #e5e5e5;

/* ✅ 已注册的尺度和语义角色。 */
--ob-ref-color-neutral-200: oklch(0.922 0 0);
--ob-sys-color-border-default: var(--ob-ref-color-neutral-200);
```

自有组件 CSS 的每种颜色都使用 `--ob-sys-color-*`，界面文本角色使用 `--ob-sys-typography-*`。当选择是结构性的时，可以直接使用 [`visual-foundations.md`](visual-foundations.md) 中批准的非颜色 `ref` 子集，例如 `--ob-ref-space-4` 或 `--ob-ref-radius-ui-md`。不要在组件 CSS 中使用参考颜色，因为这会绕过主题语义；不要从单独的字体参考重新构造已发布的排版角色。

只有当已经实现的可复用组件必须公开稳定自定义点，或接收无法枚举的运行时几何值（例如用户调整后的面板宽度）时，才添加 `comp` Token。使用或覆盖前，先在中央注册表和公共清单中声明 Token；不要创建组件局部自定义属性。第三方组件库的内部 Token 清单不是预先声明未使用 Orchbee 组件 Token 的理由。

## 主题契约

`:root` 提供浅色系统颜色的回退映射。`[data-theme='light']` 在显式浅色边界重新应用该映射，`[data-theme='dark']` 应用深色映射。主题边界只覆盖系统颜色和 `color-scheme`；参考 Token 和组件 Token 保持稳定。继承的自定义属性把所选映射限定在该边界的子树中，因此组件 CSS 绝不根据当前主题分支。

初始注册表只接受 `light` 和 `dark`。应用组合边界拥有文档的主题属性。只有当一个具名区域的完整子树有意使用另一套主题时，该区域的所有者才可以在包装元素上设置 `data-theme`；叶组件绝不能为局部样式设置它。只有当具体产品使用方需要时才添加其他主题，并覆盖系统 Token，而不是参考或组件 Token。

`packages/ui/src/styles.css` 在 `:root` 和两个显式主题边界都把 shadcn 生成的 `--background`、`--primary` 和 `--ring` 等变量映射到 Orchbee 系统 Token。在每个边界重新声明 alias，会让它们间接引用的 `var(--ob-sys-*)` 按该边界的系统 Token 值计算。它的 Tailwind `dark` variant 以 `[data-theme='dark']` 为目标；`.dark` 不是受支持的主题 API。这些无前缀变量是检入的 shadcn 源码使用的兼容性 API，不是自有组件的另一套 Token 词汇。

## 运行时注册表和 Tailwind 主题变量

Orchbee 注册表在 `:root` 下使用标准自定义属性，因为它是与框架无关的运行时 API。标准自定义属性可以由主题 selector、媒体查询、应用宿主和非 Tailwind 使用方覆盖，而不会创建任何 CSS class。

Tailwind 的 `@theme` 指令职责不同：顶层主题变量声明指示 Tailwind 创建匹配的 utility 或 variant。不要通过 `@theme` 发布 `--ob-*`；这样做会公开第二套 utility class 编写 API，绕过 CSS Modules 和语义使用规则，并阻止 `[data-theme='dark']` 等限定 selector 作用域的系统映射。

只在已检查的 Tailwind 兼容边界内使用 `@theme inline`：

- `packages/ui/src/styles.css` 公开生成组件所需的聚焦 shadcn 词汇。
- `packages/design-tokens/src/tailwind-compat.css` 公开完整但当前未启用的迁移词汇。

这些兼容声明把 Tailwind namespace 映射到 Orchbee Token；它们不拥有 Token 值。新的 Orchbee 自有组件直接使用 `var(--ob-*)`，绝不需要生成的 utility class。

## 语义覆盖

系统层级是一套有限的跨组件语言。提出新角色前，选择最接近的现有角色；组件名称不属于系统 Token 名称。

| 系列   | 必需角色矩阵                                                                                                                                             |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 表面   | canvas、inactive、subtle、raised、overlay、inverse、hovered、pressed、selected、selected-hover、disabled、scrim                                          |
| 文本   | primary、heading、secondary、label、muted、placeholder、disabled、inverse，以及链接 rest/hover/focus/pressed                                             |
| 图标   | primary、secondary、muted、inverse、hover、disabled                                                                                                      |
| 边框   | subtle、default、strong、focus、selected、disabled                                                                                                       |
| 滚动条 | 透明轨道，以及滑块的 rest、hover 和 pressed                                                                                                              |
| 操作   | primary 和 secondary 的 rest/hover/pressed/text，加上共享 disabled surface/text/border                                                                   |
| 状态   | info、success、warning 和 danger；每个状态都有 rest、hover、pressed、surface、surface-hover、border、border-hover、text、text-hover 和 text-pressed 角色 |
| 品牌   | accent、accent-hover 和 on-accent，用于品牌行动入口与身份强调；不能代替状态角色                                                                          |
| 数据   | 五种分类序列颜色                                                                                                                                         |
| 排版   | body sm/md/lg/xl、display sm/md/lg、control sm、label xs/sm/md/lg、heading xs/sm/md/lg/xl 和 code sm/md                                                  |
| 其他   | 链接装饰 rest/hover/focus、语义动效 duration/easing、焦点环几何、disabled/muted opacity 和 raised/overlay shadow                                         |

每个 `--ob-sys-color-*` Token 都有显式的 `[data-theme='light']` 和 `[data-theme='dark']` 映射；`:root` 共享浅色映射作为回退。包测试强制两份清单相同，并确保主题作用域不包含参考 Token 或组件 Token。状态和排版矩阵也经过回归测试，因此删除某个状态不会留下表面有效但不完整的注册表。

通过 `font` 简写使用排版角色，使字体系列、字号、字重和行高保持为一个决策：

```css
.title {
	font: var(--ob-sys-typography-heading-md);
	color: var(--ob-sys-color-text-heading);
}

.description {
	font: var(--ob-sys-typography-body-sm);
	color: var(--ob-sys-color-text-secondary);
}
```

间距、尺寸、圆角、断点、动效、层次和层级基础保留在参考层级，因为它们的值与上下文无关。不要创建以 margin 和 padding 等 CSS 属性命名的独立系统 alias；组件从同一个有限间距尺度中选择。

自有 transition 和 animation 使用 `--ob-sys-motion-duration-*` 和 `--ob-sys-motion-easing-*`，不要直接使用动效参考。在 `prefers-reduced-motion: reduce` 下，注册表把 fast、normal 和 slow 语义 duration 映射到 instant 参考。keyframe 名称和参与 transition 的属性仍属于组件实现细节。

## Tailwind 替换就绪度

参考层级有意覆盖已安装 Tailwind CSS 版本的完整默认主题词汇，即使 Orchbee 自有组件尚未使用其中大多数词汇。当前 Tailwind 4.3.3 基线包含 19 个 namespace 系列中的 419 个公共 `@theme` 变量：

| 能力组      | Tailwind namespace                                                                   | 变量数 |
| ----------- | ------------------------------------------------------------------------------------ | -----: |
| 颜色        | `color`                                                                              |    288 |
| 排版        | `font`、`font-weight`、`text`、`tracking`、`leading`                                 |     54 |
| 布局        | `spacing`、`breakpoint`、`container`、`radius`、`perspective`、`aspect`、`max-width` |     35 |
| 效果        | `shadow`、`inset-shadow`、`drop-shadow`、`blur`                                      |     27 |
| 动效/默认值 | `ease`、`animate`、`default`                                                         |     15 |
| 总计        |                                                                                      |    419 |

`@orchbee/design-tokens/tailwind-compat.css` 逐一枚举已安装主题。它由 Tailwind 打包的 `theme.css` 生成，把每个公共主题变量映射到 Orchbee 参考 Token，并作为迁移基础设施导出。`@orchbee/ui/styles.css` 不导入该文件：shadcn 保留其聚焦兼容映射，直到有意迁移切换该边界。

覆盖目标是 Tailwind 的公共 `@theme` 契约，而不是其内部 `--tw-*` 运行时状态。`--tw-shadow`、`--tw-ring-color` 和 `--tw-translate-x` 等变量是由 utility 生成的实现细节；它们不是可枚举设计决策，并会随着拥有它们的 utility 一同消失。

断点和容器尺寸是唯一映射例外。CSS 自定义属性不能用于媒体和容器查询条件，因此 `--breakpoint-*` 和 `--container-*` 在 Tailwind 兼容文件中必须保持字面量。匹配的 `--ob-ref-breakpoint-*` 和 `--ob-ref-container-*` Token 仍然让词汇可枚举。`pnpm lint:styles` 会验证三个来源——已安装的 Tailwind 字面量、兼容字面量和 Orchbee 参考 Token——完全相同。

完整调色板存在于参考层级中，以实现完整替换。它们不会扩大自有组件契约：组件颜色仍然必须使用语义 `--ob-sys-color-*` Token。

同一限制适用于间距、圆角、排版参考、控件尺寸和图标尺寸 Token。注册表中存在某个值，只证明它可枚举，并不使其成为已批准的产品选择。自有子集及其场景规则位于 [`visual-foundations.md`](visual-foundations.md)。

## 自有组件使用

- CSS Module、`className`、Tailwind、JSX 展开、内联 style 和生成源码边界遵循 [`styling.md`](styling.md)。
- 颜色使用 `--ob-sys-color-*`，完整排版角色使用 `--ob-sys-typography-*`。
- 只使用 [`visual-foundations.md`](visual-foundations.md) 批准的非颜色参考 Token 子集。
- 已注册 `--ob-comp-*` 属性只用于公共定制或运行时几何。
- 绝不使用未注册自定义属性或已注册 Token 的回退值。

## 响应式阈值

CSS 自定义属性无法为 `@media` 或 `@container` 条件提供值。来自 `@orchbee/design-tokens` 的 `responsiveBreakpoints` 是有限例外，当前定义 `sm`、`md`、`lg`、`xl` 和 `2xl` 阈值。TypeScript 映射由匹配的 `--ob-ref-breakpoint-*` 声明生成，因此 CSS、TypeScript 和 Tailwind 兼容值只有一个权威来源。使用相应导出值作为 CSS 字面量阈值；`pnpm lint:styles` 会拒绝其他 `px` 或 `rem` 查询值。

不要仅仅因为存在某类设备就使用断点。优先使用固有布局、flex/grid 约束和容器查询；只有当具体布局转换无法使用现有词汇时，才添加新阈值。

## Token 变更

添加或修改 Token 前：

1. 识别反复出现的跨组件角色，并确认现有 Token 无法表达它。
2. 选择最低有效层级和基于角色的名称。
3. 添加 CSS 声明，为系统颜色添加每个必需主题映射，并运行 `pnpm generate:design-token-inventory`；绝不能手工编辑生成的 TypeScript 清单。
4. 在同一个变更中更新使用方和任何受影响的 Story。
5. 运行 `pnpm lint:styles`、`pnpm --filter @orchbee/design-tokens test` 和根级 `pnpm verify` 门禁。

修改 Tailwind 版本后，运行 `pnpm generate:tailwind-token-compat`。如果 Tailwind 添加了主题变量，生成器会拒绝未知 namespace，或者 `pnpm lint:styles` 会报告缺失 Orchbee 目标 Token。有意识地添加该参考 Token、重新生成兼容文件、更新本文档和测试中的 419 变量基线，并运行完整验证门禁。不要手工编辑生成的兼容输出。

不要仅为保留尚未发布的名称而添加 alias。以原子方式重命名实验性 Token 及其所有使用方。一旦 Token 成为稳定已发布包的一部分，遵循 [`docs/PACKAGES.md`](../PACKAGES.md#版本管理) 中的包兼容性规则。

<!-- documentation-pairing: english-sha256=a187cda51cda608cd929a74ee3a56a45f99a59231cedf632caf9e561ed0036b8 zh-cn-sha256=bae4114671e7939107c1d1c74b0934a01d8ad3924d2a0ebc2e9946a700a949d3 -->
