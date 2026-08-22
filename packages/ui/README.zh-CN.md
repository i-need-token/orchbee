# @orchbee/ui

Orchbee 用于可重用视觉图元的私有包，包括由 Base UI 和产品图标边界支持的 shadcn/ui 组件。

## 适用范围

该包拥有可重用的视觉基元及其可访问性保护的 Base UI 组合。它不拥有主题、设计令牌、会话、代理、终端窗格、路由、远程状态或特定于平台的效果。机器可读范围合约位于 [`project.json`](project.json) 中。

## 使用

在应用程序入口处导入一次包样式表，并通过包API使用组件。

```tsx
import '@orchbee/ui/styles.css';
import { Button, Dialog, DialogContent, DialogTrigger } from '@orchbee/ui/components';

export function Example() {
	return (
		<Dialog>
			<DialogTrigger render={<Button variant='outline' />}>New session</DialogTrigger>
			<DialogContent>Create a session here.</DialogContent>
		</Dialog>
	);
}
```

根条目当前保留 `Button` 兼容性导出。显式 `@orchbee/ui/components` 条目导出完整签入的 shadcn Base UI 组件集和 Orchbee 创作的视觉图元。组件保留其标准变体、`className` 成分和基础 UI 道具。

`Tabs` 还公开了用于全高应用工作区域的 `workspace` 变体。由 `TabsBar`、工作区 `TabsList`、每个触发器一个 `TabsItem` 以及相邻项目之间的独立一像素 `TabsDivider` 组成。 Tab 触发器不绘制自己的分隔符。 `TabsBar` 是 `35px` 高，由 `34px` 内容区域和 `1px` 底部边框组成，并且在语义选项卡列表旁边可能包含产品拥有的控件。每个项目的宽度最多为 `200px`，并且当空间受限时会均匀收缩。未选定的选项卡使用较亮的基础表面，悬停会添加较亮的表面而不更改标签颜色，选定的选项卡使用较暗的选定表面。 UI 软件包提供可访问的 Base UI 交互和响应式镶边；产品包仍然负责选项卡数据和语义操作，例如选择、创建和关闭。

下拉菜单、上下文菜单、菜单栏和子菜单表面共享 `250px` 菜单宽度，只在可用视口更窄时收缩。产品消费者应保留该默认值，不另设一次性菜单宽度。内容、触发器与交互契约遵循仓库[前端菜单标准](../../docs/frontend/menus.md)。

默认对话框和警报对话框表面使用共享的 `480px` 宽度，受视口和没有外部轮廓的阴影约束。他们的操作页脚共享内容表面并依赖于间距而不是分隔线或着色的页脚带。

## 产品图标

产品代码通过包 API 消耗 `ProductIcon` 并使用其 Orchbee 拥有的名称白名单。它不得直接导入 Codicons CSS、供应商类名称、字体字符或资源路径。包样式表加载图标字体一次，而 `ProductIcon` 保持每个字形装饰性，以便所属控件提供可访问的名称。 `ProductIcon` 和生成的图元图标默认使用共享的 `14px` 小图标尺寸；非默认字形大小是显式 `ProductIcon` 组件变体，而不是消费者覆盖。紧凑的节标题操作使用记录的 `13px` 节操作变体。前导下拉菜单、上下文和菜单栏图标使用 `16px` 菜单前导变体，而尾随披露箭头仍保留 `14px`。

产品允许列表包含 `sidebar-left`、`add`、`arrow-left`、`close`、`ellipsis`、`more-vertical`、`pin`、`search`、`settings` 与 `terminal`，分别由对应 Codicons 字形支持。生成的 shadcn 源在生成代码兼容边界内保留其上游 Lucide 依赖。参见仓库[前端样式标准](../../docs/frontend/styling.md)与[第三方声明](../../THIRD_PARTY_NOTICES.md)。

可调整大小的组件外观为需要通过组命令 API 约束实时布局的产品适配器导出 `ResizablePanelGroupHandle`。

## 刷新组件

封装级 [`components.json`](components.json) 引脚 Base UI Nova 风格，并将官方 CLI 定向到 `src/components/ui`。

```sh
pnpm --dir packages/ui exec shadcn add --all --overwrite
```

仓库维护官方 Base UI registry 的完整快照，而不是按需添加 primitive。接受刷新前必须审核 CLI diff，因为可能需要重新应用有针对性的 Orchbee 兼容调整。不要手写 shadcn 已提供的 primitive。

快照使用 `@orchbee/foundation/react`，让生成的事件回调满足仓库的稳定标识与最新渲染值契约。刷新受影响的 registry 组件时须保留这一集中适配，除非上游已经提供相同契约。

该快照还将下拉菜单和上下文菜单几何图形映射到共享的 Orchbee 菜单宽度标记。它还省略了上游工具提示箭头，因此每个共享工具提示表面都保留 Orchbee 视觉契约，并将生成的图元图标默认值标准化为 `14px`。刷新这些基元时保留这些集中的适应。

生成的 Tailwind 实用程序仍然是组件源的一部分。 [`src/styles.css`](src/styles.css) 包含 Tailwind/shadcn 导入，将 shadcn 的兼容性变量映射到 `@orchbee/design-tokens`，并拥有这些实用程序所需的基础层，包括应用程序范围的 CSS 原生滚动条规范化。它没有定义 Orchbee 令牌词汇表。完整的工作流程位于 [`docs/frontend/styling.md`](../../docs/frontend/styling.md) 中。

该样式表还拥有显式的 Tailwind `@source './components'` 注册。刷新 shadcn 时保留该注册：源构建的应用程序从 UI 工作区外部编译样式表，因此仅自动源发现无法保证在开发期间发出生成的组件实用程序。

Tailwind CSS、shadcn CLI 样式表和 `tw-animate-css` 是构建时依赖项。 Rslib将它们编译成发布的`styles.css`；消费者在运行时不需要这些工具。

## 开发

```sh
pnpm --filter @orchbee/ui... build
pnpm --filter @orchbee/ui typecheck
pnpm --filter @orchbee/ui test
pnpm --filter @orchbee/ui test:exports
```

通过仓库 Storybook 浏览每个已提交组件：

```sh
pnpm storybook
```

UI 故事存在于 [`stories`](stories) 中，并由 `apps/storybook` 消耗。 `src/components/ui`中的每个组件模块都有一个匹配的Story文件； Storybook 应用程序测试强制执行该库存合同。 Storybook 是开发基础设施，不是该包公共导出的一部分。
