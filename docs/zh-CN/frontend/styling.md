# 前端样式

## 所有权边界

| 边界               | 契约                                                                                                                                            |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Orchbee 自有组件   | 使用 CSS Module，以及 [`design-tokens.md`](design-tokens.md) 和 [`visual-foundations.md`](visual-foundations.md) 选择的 Token。                 |
| 生成的 shadcn 源码 | 位于 `packages/ui/src/components/ui`，保留 Tailwind CSS v4 utility 和可组合 `className`，并保持为兼容边界。                                     |
| shadcn 配置        | `packages/ui/components.json` 为每次 CLI 运行固定 Base UI Nova 注册表样式。                                                                     |
| UI 样式入口        | `packages/ui/src/styles.ts` 先导入 Token 注册表，再导入 `packages/ui/src/styles.css`。                                                          |
| UI 兼容样式表      | 把 shadcn 变量映射到 Orchbee Token，保留必需 `@theme inline` 声明，用 `@source` 注册 `./components`，并拥有包含原生滚动条统一样式的最小基础层。 |
| 完整 Tailwind 兼容 | `@orchbee/design-tokens/tailwind-compat.css` 映射已安装的公共 Tailwind 主题，但在有意迁移前不导入。                                             |

生成源码例外还包括 `packages/ui/src/hooks/use-mobile.ts`、`packages/ui/src/lib/cn.ts` 和 `packages/ui/src/styles.css`。不得把其中的 utility 复制到自有组件，也不得为重命名生成的 prop 而创建重复原语。

使用官方 shadcn 原语，不得手写等价实现；产品组合保留在生成原语之外。

标准 `:root` 自定义属性仍是运行时注册表，因为主题选择器、媒体查询、宿主和非 Tailwind 消费者都能覆盖它。兼容文件只映射值，绝不拥有 Orchbee Token 名称或主题值。

## 自有样式契约

每个自有组件使用一个 CSS Module，并从 JSX 导入。

```tsx
import styles from './FilterPanel.module.css';

export function FilterPanel() {
	return <section className={styles.root}>...</section>;
}
```

在生成的 shadcn 边界之外，不得使用：

- Tailwind 导入、指令、utility 字符串或 `tailwind-merge`；
- 替代 CSS Module 引用的字面量或模板字符串 `className`；
- 自有组件上的通用 `className` prop；
- JSX 属性展开，除非它是属性静态具名且不含样式逃生口的内联对象；
- Token 注册表覆盖的原始视觉值；
- 未注册自定义属性或注册 Token 的 `var()` 回退值；或
- 内联标准 CSS 属性。

内联 style 对象只能包含用于测量或用户控制运行时几何的已注册 `--ob-comp-*` key。具有产品语义的组合和应用布局绝不使用 Tailwind utility。

## 产品图标

- 产品控件通过 Orchbee 图标名称允许列表消费 `@orchbee/ui/components` 的 `ProductIcon`。
- 产品代码绝不导入供应商样式表、class 名称、字体字符或物理图标资产。
- `@orchbee/ui` 把名称映射到 Codicons，并通过其样式入口加载 Provider 样式表。
- `ProductIcon` 是装饰性的，继承 `currentColor`，并默认使用视觉基础中的 `14px` `icon-sm` 尺寸；所属控件提供无障碍名称和语义颜色。非默认尺寸必须使用已记录的组件 variant，并来自显式的组件级要求。
- 生成的 shadcn 源码只能在其兼容边界内保留 Lucide 导入。

图标变更要更新允许列表、映射、聚焦测试、受影响包文档，以及 Provider 或许可证变更时的第三方声明。使用浏览器验证字体加载、光学粗细和对齐。

## 样式表导入

| 消费者                      | 组合入口的唯一导入                            |
| --------------------------- | --------------------------------------------- |
| 使用 `@orchbee/ui` 的应用   | `import '@orchbee/ui/styles.css';`            |
| 不使用 `@orchbee/ui` 的界面 | `import '@orchbee/design-tokens/tokens.css';` |
| `apps/storybook` UI Story   | 公共 `@orchbee/ui` 样式表                     |

不得重复导入 Token 注册表。Storybook 脚手架不得重设组件样式或充当隐式主题；遵循 [`storybook.md`](storybook.md)。

## 引入 shadcn 组件

使用 UI 包的 `component:add` 脚本刷新明确指定的已准入组件。组件准入遵循 [体验组件规范](experience-components.md)；仅供 Story 展示不是准入理由。不整体导入官方注册表。

CLI 读取 `packages/ui/components.json` 并写入 `packages/ui/src/components/ui`；审查生成变更时使用 `--dry-run`、`--view` 和 `--diff`。

刷新后：

1. 保留生成的实现和 utility，除非仓库需要聚焦适配。
2. 恢复 `packages/ui/src/styles.css` 到 `@orchbee/design-tokens` 的映射，包括 `@source './components'`；拒绝重新生成的原始主题值。
3. 审查每次覆盖，并保留 TypeScript、声明输出和模块边界适配。
4. 通过 `@orchbee/ui/components` 只导出有已准入消费者的组件与插槽；保留收窄后的入口，不暴露物理源码路径。
5. 测试仓库适配和依赖的语义，不重复上游测试套件。
6. 运行 UI 包检查和根验证门禁。

产品组合保留在所属应用或产品能力包中。自有产品组件可以向原语传递语义 prop，但不得通过 `className` 添加 Tailwind utility。

## 验证

变更 Token、生成样式、依赖、导出或构建配置后运行：

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

更新 Tailwind 依赖后还要运行：

```sh
pnpm generate:tailwind-token-compat
pnpm lint:styles
```

主要参考：[Tailwind 主题变量](https://tailwindcss.com/docs/theme)、[shadcn 手动安装](https://ui.shadcn.com/docs/installation/manual)、[Base UI 支持](https://ui.shadcn.com/docs/changelog/2026-01-base-ui)、[`components.json`](https://ui.shadcn.com/docs/components-json)、[Rslib CSS](https://rslib.rs/guide/advanced/css) 和 [Codicons](https://github.com/microsoft/vscode-codicons)。

<!-- documentation-pairing: english-sha256=04f01c2175f6937bee1b8ddf1088429f12fa8b37321a5e21d86d2949761c8153 zh-cn-sha256=53c8c7ffebd1fff778dd2dad862189ea5b662eb537c1948969e67d56aaba0ca5 -->
