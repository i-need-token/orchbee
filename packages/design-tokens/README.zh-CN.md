# @orchbee/design-tokens

Orchbee CSS 界面共享的可枚举设计决策。

GUI/TUI 共同语义与主题对应关系遵循
[`docs/INTERFACE-DESIGN.md`](../../docs/zh-CN/INTERFACE-DESIGN.md)。本包实现 CSS Token；
原生终端主题实现共同角色的自身映射。

## 使用

通过应用程序样式表或兼容层导入 CSS 注册表一次：

```css
@import '@orchbee/design-tokens/tokens.css';

.panel {
	padding: var(--ob-ref-space-4);
	color: var(--ob-sys-color-text-primary);
	background: var(--ob-sys-color-surface-raised);
}
```

需要 CSS 变量引用的 JavaScript 消费者可以使用类型化库存：

```ts
import { designToken, responsiveBreakpoints } from '@orchbee/design-tokens';

const foreground = designToken('--ob-sys-color-text-primary');
const compactViewport = responsiveBreakpoints.sm;
```

`designTokenNames` 导出是完整的公共 CSS 自定义属性清单。[`src/design-token-names.generated.ts`](src/design-token-names.generated.ts) 根据 [`src/tokens.css`](src/tokens.css) 生成该清单与 `responsiveBreakpoints`；包测试和仓库样式检查会拒绝过时输出。

## Tailwind 兼容性

该包还导出 `@orchbee/design-tokens/tailwind-compat.css`。这项生成但默认未启用的资产枚举已安装 Tailwind CSS 4.3.3 主题中的全部 419 个公共 `@theme` 变量，并将其映射到 Orchbee 参考 Token。它是迁移基础设施，不是第二套创作 API；`@orchbee/ui` 当前不导入它。

采用完全兼容层的消费者必须提供运行时令牌注册表和 Tailwind 构建时主题映射：

```css
@import 'tailwindcss';
@import '@orchbee/design-tokens/tokens.css';
@import '@orchbee/design-tokens/tailwind-compat.css';
```

Tailwind 的内部 `--tw-*` 实用程序状态故意位于设计令牌库存之外。断点和容器映射使用检查的文字值，因为 CSS 自定义属性不能出现在查询条件中。

运行时注册表有意使用标准自定义属性。 Tailwind 的 `@theme` 指令是一个编译时 API，它创建实用程序和变体，因此它仅属于 Tailwind 和 shadcn 兼容性边界。直接通过 `@theme` 发布 `--ob-*` 将创建第二个实用程序类创作 API，并防止系统映射被限制在 `[data-theme='dark']` 等选择器下。

## 主题

参考标记（`--ob-ref-*`）是稳定的视觉尺度。系统令牌（`--ob-sys-*`）赋予这些值接口含义并在显式 `[data-theme='light']` 和 `[data-theme='dark']` 边界下进行更改； `:root` 提供浅色（light）后备。组件消耗系统颜色标记，因此主题更改不会改变组件 CSS。

系统颜色枚举表面、内容、边框、操作、状态和交互状态。每个系统颜色都有明确的明暗映射。语义排版标记结合了正文、标签、标题和代码角色的系列、大小、粗细和行高：

```css
.title {
	font: var(--ob-sys-typography-heading-md);
	color: var(--ob-sys-color-text-heading);
}
```

创作的动作使用语义持续时间和缓动标记。当用户请求减少运动时，注册表将非即时语义持续时间映射到即时。

CSS 自定义属性无法在媒体查询条件中求值。只能把导出的 `responsiveBreakpoints` 值用作字面量 `@media` 或 `@container` 阈值；仓库样式检查会强制执行少量明确例外。

创作、命名和组件令牌规则在 [`docs/frontend/design-tokens.md`](../../docs/frontend/design-tokens.md) 中定义。

## 开发

```sh
pnpm --filter @orchbee/design-tokens build
pnpm --filter @orchbee/design-tokens typecheck
pnpm --filter @orchbee/design-tokens test
pnpm --filter @orchbee/design-tokens test:exports
pnpm generate:design-token-inventory
pnpm generate:tailwind-token-compat
```
