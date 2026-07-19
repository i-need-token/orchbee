# 前端样式

## 样式边界

Orchbee 自有组件使用 [`docs/frontend/design-tokens.md`](design-tokens.md) 中定义的可枚举 Token 系统和 CSS Modules。不要在具有产品语义的组合、应用布局或新的自有组件实现中使用 Tailwind utility。

从 shadcn/ui 的 Base UI 注册表安装可复用 primitive，并在 `@orchbee/ui` 中拥有生成的开放组件源码。不要手写 shadcn 已经提供的 primitive。检入的 `components.json` 固定 Base UI Nova 样式，使每次 CLI 调用都使用相同的组件系列。

生成的 shadcn 组件保留其 Tailwind CSS v4 utility class 和标准可组合 `className` API。Tailwind 例外仅限于 `packages/ui/src/components/ui`、其生成的 `use-mobile.ts` Hook、`packages/ui/src/lib/cn.ts` 和 `packages/ui/src/styles.css` 兼容层。不要把其 utility 复制到自有组件中，也不要仅仅为了重命名生成的 props 而引入第二套 primitive 实现。

`packages/ui/src/styles.ts` 是样式导出的 source-build 入口：它先于 `packages/ui/src/styles.css` 导入设计 Token 注册表。CSS 文件将生成的 shadcn 变量词汇映射到 Orchbee 系统 Token，保留为检入的 shadcn 源码生成 utility 的 `@theme inline` 声明，通过 Tailwind 的 `@source` 指令显式注册 `./components`，并拥有生成组件所需的最小全局基础层。显式源码注册是必需的，因为 source-built 应用会从 UI 工作区之外编译此样式表；不得依赖 Tailwind 的自动发现边界来找到包内组件文件。两个文件都不拥有 Token 名称或具体主题值。Orchbee 的运行时注册表仍然使用标准 `:root` 自定义属性，因为自有组件直接使用变量，而且系统主题映射必须保持可由 selector 覆盖。

`@orchbee/design-tokens/tailwind-compat.css` 是已安装 Tailwind 默认主题到 Orchbee 参考 Token 的另一份完整映射。它存在的目的是让未来迁移可以在替换值来源时保留 Tailwind 的可枚举词汇。它已导出并经过编译测试，但有意不由 `@orchbee/ui` 导入；采用它属于单独的兼容性边界变更，不是附带的 Token 更新。

## 产品图标

Orchbee 自有产品控件从 `@orchbee/ui/components` 使用 `ProductIcon`。产品代码使用 Orchbee 自有的图标名称白名单，绝不直接导入供应商样式表、供应商 class 名、字体字符或物理图标资产。`@orchbee/ui` 负责把这些名称映射到已安装的 Codicons，并通过包的样式入口加载其样式表。这个边界使更换图标提供方时只需局部修改，也避免让供应商标识符成为产品 API。

`ProductIcon` 仅作装饰并继承 `currentColor`；所属控件负责提供无障碍名称和分配语义化图标颜色 Token。使用视觉基础规范中可枚举的图标尺寸，不得使用任意尺寸。生成的 shadcn 源码可以在生成代码兼容边界内保留上游 Lucide 导入；不得把该例外扩展到 Orchbee 自有产品组件。

新增或更换产品图标时，必须更新 `ProductIcon` 的名称白名单与映射、对应的聚焦测试；行为变化时更新所属包文档；提供方或许可证变化时更新第三方声明。必须在真实浏览器中验证结果，因为字体加载、视觉粗细和对齐无法仅靠类型或单元检查确认。

## 样式表导入

使用 shadcn 组件的应用在其组合入口导入一次兼容样式表：

```ts
import '@orchbee/ui/styles.css';
```

不使用 `@orchbee/ui` 的界面在自身样式之前直接导入 Token 资产：

```ts
import '@orchbee/design-tokens/tokens.css';
```

`apps/storybook` 为 UI Story 导入 UI 样式表。Storybook 布局脚手架不得重新定义组件外观，也不得作为隐式主题。Story 所有权和浏览器验证契约位于 [`docs/frontend/storybook.md`](storybook.md)。

## 引入 shadcn 组件

UI 包保留官方 Base UI 组件注册表的完整快照。使用以下命令刷新该快照：

```sh
pnpm --dir packages/ui exec shadcn add --all --overwrite
```

CLI 读取 `packages/ui/components.json`，并把组件源码写入 `packages/ui/src/components/ui`。当生成的变更需要审查时，在刷新前使用其 `--dry-run`、`--view` 和 `--diff` 选项。

刷新后：

1. 除非仓库兼容性需要聚焦调整，否则保持生成的实现和 Tailwind utility 不变。
2. 恢复 `packages/ui/src/styles.css` 作为到 `@orchbee/design-tokens` 的兼容映射，包括显式的 `@source './components'` 注册；绝不能接受重新生成的原始主题值成为第二个事实来源。
3. 审查每个覆盖差异，并保留 TypeScript、声明输出和模块边界调整。
4. 通过 `@orchbee/ui/components` 显式导出每个注册表组件；不要公开物理源码路径。
5. 测试 Orchbee 所依赖的调整或语义；不要逐个组件复制上游组件套件。
6. 运行 UI 包检查和仓库验证门禁。

具有产品语义的组合属于所属应用或产品能力包，生成的组件仍然是可复用 primitive。产品组件可以把语义 props 传给 shadcn primitive，但不得使用 primitive 的 `className` 样式绕过入口添加 Tailwind utility。

## 验证

修改 Token、生成的组件样式、依赖、导出或构建配置后，运行 Token 和 UI 包检查：

```sh
pnpm lint:styles
pnpm --filter @orchbee/design-tokens... build
pnpm --filter @orchbee/design-tokens typecheck
pnpm --filter @orchbee/design-tokens test
pnpm --filter @orchbee/design-tokens test:exports
pnpm --filter @orchbee/ui... build
pnpm --filter @orchbee/ui typecheck
pnpm --filter @orchbee/ui test
pnpm --filter @orchbee/ui test:exports
```

更新 Tailwind 依赖后，还要重新生成当前未启用的完整映射，并让样式契约验证精确覆盖：

```sh
pnpm generate:tailwind-token-compat
pnpm lint:styles
```

外部参考验证日期：2026-07-19。

- [Tailwind CSS 主题变量](https://tailwindcss.com/docs/theme)区分会生成 utility 和 variant 的顶层 `@theme` 声明与不会生成它们的常规 `:root` 自定义属性。
- [shadcn/ui 手动安装](https://ui.shadcn.com/docs/installation/manual)记录 Base UI 样式标识符、Tailwind 导入、依赖、alias 和 `components.json` 契约。
- [shadcn/ui Base UI 支持](https://ui.shadcn.com/docs/changelog/2026-01-base-ui)记录 Base UI 组件 variant 和共享 shadcn 抽象。
- [shadcn/ui `components.json`](https://ui.shadcn.com/docs/components-json)定义 CLI 的持久生成配置。
- [Rslib CSS 支持](https://rslib.rs/guide/advanced/css)记录 Tailwind CSS v4 插件和提取出的库样式。
- [VS Code Codicons](https://github.com/microsoft/vscode-codicons)发布产品图标边界背后使用的图标字体、CSS class、源 SVG 和 CC BY 4.0 许可证。
