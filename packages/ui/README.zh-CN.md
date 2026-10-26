# @orchbee/ui

Orchbee 用于可重用视觉图元的私有包，包括由 Base UI 和产品图标边界支持的 shadcn/ui 组件。

## 适用范围

该包拥有可重用的视觉基元及其可访问性保护的 Base UI 组合。它不拥有主题、设计令牌、会话、代理、终端窗格、路由、远程状态或特定于平台的效果。机器可读范围合约位于 [`project.json`](project.json) 中。

## 使用

在应用程序入口处导入一次包样式表，并通过包API使用组件。

```tsx
import '@orchbee/ui/styles.css';
import { useState } from 'react';
import { Button, Dialog, DialogContent } from '@orchbee/ui/components';

export function Example() {
	const [open, setOpen] = useState(false);
	return (
		<>
			<Button variant='outline' onClick={() => setOpen(true)}>
				New session
			</Button>
			<Dialog open={open} onOpenChange={setOpen}>
				<DialogContent>Create a session here.</DialogContent>
			</Dialog>
		</>
	);
}
```

显式 `@orchbee/ui/components` 入口只导出有消费者的 shadcn Base UI 与 Orchbee 视觉原语；`@orchbee/ui/styles.css` 提供其样式。这是仅有的两个公共入口，该包没有根入口。组件保留所依赖的标准变体、`className` 组合及 Base UI 属性。新增组件与插槽必须有已准入用途，不能只为可能的未来需求扩大库存。

`Tabs` 还公开了用于全高应用工作区域的 `workspace` 变体。由 `TabsBar`、工作区 `TabsList`、每个触发器一个 `TabsItem` 以及相邻项目之间的独立一像素 `TabsDivider` 组成。 Tab 触发器不绘制自己的分隔符。 `TabsBar` 是 `35px` 高，由 `34px` 内容区域和 `1px` 底部边框组成，并且在语义选项卡列表旁边可能包含产品拥有的控件。每个项目的宽度最多为 `200px`，并且当空间受限时会均匀收缩。未选定的选项卡使用较亮的基础表面，悬停会添加较亮的表面而不更改标签颜色，选定的选项卡使用较暗的选定表面。 UI 软件包提供可访问的 Base UI 交互和响应式镶边；产品包仍然负责选项卡数据和语义操作，例如选择、创建和关闭。

下拉菜单、上下文菜单和子菜单表面共享 `250px` 菜单宽度，只在可用视口更窄时收缩。产品消费者应保留该默认值，不另设一次性菜单宽度。内容、触发器与交互契约遵循仓库[前端菜单标准](../../docs/frontend/menus.md)。

Portaled 浮动内容在模态框之外保留生成的默认层级。模态框消费者通过受支持的 `positionerClassName` Override 传入映射到 `--ob-ref-layer-modal` 的 CSS Module Class，使 Portal 位于所属 Dialog 之上，而无需为 Popup 设置一次性 `z-index`。

默认 Dialog 与 Alert Dialog 表面使用共享的 `480px` 宽度（受视口约束），并使用没有外部轮廓的阴影。他们的操作页脚共享内容表面并依赖于间距而不是分隔线或着色的页脚带。

## 保留开发消费者

[依赖保留记录](dependency-retentions.json) 拥有 `Tabs`、`TabsBar`、`TabsContent`、
`TabsDivider`、`TabsItem`、`TabsList`、`TabsTrigger` 经 `src/components/ui/tabs.tsx`、
`src/components/ui/index.ts`、`src/components/index.ts` 三层出口产生的精确 strict 诊断。
真实当前消费者是 [Tabs Story](stories/tabs.stories.tsx)：`Default` 和 `Workspace` 实际渲染
所导入的组件。不声称已有产品 Host。上文已准入的工作区组合是保留用途；UI 与未来 Web/Desktop
组合负责人通过 ORC-201 承担接入，本阶段不启动迁移。

清单用指纹绑定已审阅的 Story、导出链和发现及解析输入。任何输入变化都遵循
[保留检查契约](../../docs/zh-CN/LINTING.md#保留开发消费者)；只更新 hash 不构成用途变化的批准。
继续保留原有 build、typecheck、Storybook、行为及 `test:exports` 证据。真实生产消费或用途撤销
时，在同一变更中删除对应精确记录。诊断消失时删除过期记录；清单耗尽后删除文件及其门禁登记。

## 产品图标

产品代码通过包 API 消耗 `ProductIcon` 并使用其 Orchbee 拥有的名称白名单。它不得直接导入 Codicons CSS、供应商类名称、字体字符或资源路径。包样式表加载图标字体一次，而 `ProductIcon` 保持每个字形装饰性，以便所属控件提供可访问的名称。 `ProductIcon` 和生成的图元图标默认使用共享的 `14px` 小图标尺寸；非默认字形大小是显式 `ProductIcon` 组件变体，而不是消费者覆盖。紧凑的节标题操作使用记录的 `13px` 节操作变体。前导下拉菜单和上下文图标使用 `16px` 菜单前导变体，而尾随披露箭头仍保留 `14px`。

产品允许列表包含 `sidebar-left`、`add`、`arrow-left`、`close`、`ellipsis`、`more-vertical`、`pin`、`search`、`settings` 与 `terminal`，分别由对应 Codicons 字形支持。生成的 shadcn 源在生成代码兼容边界内保留其上游 Lucide 依赖。参见仓库[前端样式标准](../../docs/frontend/styling.md)与[第三方声明](../../THIRD_PARTY_NOTICES.md)。

可调整大小的组件外观为需要通过 group 命令式（imperative）API 约束实时布局的产品适配器导出 `ResizablePanelGroupHandle`。

## 刷新组件

封装级 [`components.json`](components.json) 引脚 Base UI Nova 风格，并将官方 CLI 定向到 `src/components/ui`。

通过包的 `component:add` 脚本，仅刷新已提交或按[体验组件准入规则](../../docs/frontend/experience-components.md)明确准入的组件名称。不整体导入注册表；Story 本身不构成产品准入依据。审核 CLI 差异，保留 Orchbee 兼容调整及已收窄的导出，不恢复无消费者插槽。不要手写 shadcn 已提供的原语。

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

<!-- documentation-pairing: english-sha256=0c7b4a9e9057fc84b49070cc051e980cc7e2e315fabfcffb21a9b57a81335e13 zh-cn-sha256=458d11742bb163496928c046406f96bc214214a4486270846ceb86a40a03d1a8 -->
