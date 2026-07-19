# Lint

## 命令

| 命令                       | 职责                                               |
| -------------------------- | -------------------------------------------------- |
| `pnpm verify`              | 运行 `docs/VERIFICATION.md` 定义的完整合并门禁     |
| `pnpm lint`                | 运行所有必需的代码、样式、依赖和项目 lint 检查     |
| `pnpm lint:code`           | 运行 oxlint 分类、显式规则和 `orchbee/*` 项目规则  |
| `pnpm lint:styles`         | 验证 Token 注册表和自有前端样式契约                |
| `pnpm lint:dependencies`   | 运行 Knip 严格依赖、文件、导出、catalog 和清单分析 |
| `pnpm lint:projects`       | 验证每个基于 Schema 的工作区项目契约               |
| `pnpm lint:fix`            | 应用安全的 oxlint 修复；剩余诊断需要修改代码       |
| `pnpm typecheck`           | 运行语法感知 lint 无法替代的 TypeScript 诊断       |
| `pnpm test:lint-rules`     | 测试 `orchbee/*` 规则和中英文文档契约              |
| `pnpm test:style-contract` | 运行前端样式契约检查器的回归测试                   |

## 强制执行边界

- 项目文档定义每条显式配置的规则和每条 `orchbee/*` 规则；上游分类规则仍由已安装的 oxlint 版本管理。
- 文档中标记为已强制执行的每条规则都必须在 `.oxlintrc.json` 中启用；`pnpm test:lint-rules` 同时验证从文档到配置的方向以及从配置到文档的覆盖关系。
- `pnpm test:lint-rules` 要求每份权威英文文档在 `docs/zh-CN/` 中都有一一对应的文件，并拒绝结构、内联标识符或本地化链接漂移。语义翻译的忠实度仍由人工审查负责。
- 错误诊断会使 `pnpm lint` 失败；警告诊断保持可见但不阻塞，直到受影响的包文件经过审查。
- `pnpm lint:dependencies` 使用 Knip `--strict`：每个导入的包都必须由导入它的工作区直接声明，未使用的文件、依赖、导出、类型，无法解析的导入，重复导出、循环依赖和过期的 catalog 条目都会使命令失败。
- 在 `knip.json#workspaces` 中保留所有可运行脚本、构建配置、测试套件和公共包入口；绝不能仅仅因为分析中漏掉了某个入口点就忽略发现的问题。
- 在包审查期间加入 `oxlint-tsgolint` 之前，类型感知的 oxlint 规则并未启用；绝不能假设 lint 当前能够检测悬空 Promise 或来自第三方的不安全 `any` 传播。
- `pnpm lint:styles` 强制执行 [`docs/frontend/design-tokens.md`](frontend/design-tokens.md) 中定义的注册表、生成的 TypeScript 清单、CSS Module、Tailwind 边界、原始值、响应式阈值、JSX class 和内联样式规则。它证明生成的公共 Token 名称与 CSS 注册表完全一致，生成的完整 Tailwind 兼容映射完整覆盖已安装的公共主题、指向已注册的 Orchbee Token，并保留编译时所需的字面量断点和容器值。它对 shadcn 编写的例外仍按路径限制在检入的生成源码和支持文件中。
- 绝不能为了让现有代码通过而弱化规则；修复代码，或者就项目策略变更取得人工批准。

## 生成的源码

`packages/ui/src/components/ui/*.tsx` 下检入的 shadcn 快照及其生成的 `use-mobile.ts` Hook 与上游注册表保持接近，使 CLI 刷新保持易于审查。按路径限定的覆盖会让这些源码免受会引发大范围机械重写的仓库编写约定约束：显式模块边界返回类型、断言风格、遮蔽风格、相等性风格、嵌套三元表达式风格、组件目录分组、TSX 中直接调用 React Hook、TSX 单函数与 Hook 单文件边界、`use-mobile.ts` 的唯一符号文件名匹配，以及同级测试要求。自动 JSX 运行时也使 `react/react-in-jsx-scope` 不适用于此处。

正确性规则仍对生成的源码生效，包括 React Hooks、JSX key、模块边界、跨工作区导入、不安全 TypeScript 构造和内联抑制禁令。`ChartStyle` 是唯一的额外例外，用于其由上游控制的 style 元素；[`docs/frontend/react.md`](frontend/react.md#正确性与安全性) 定义了该约束。

不要把生成源码的覆盖扩展到 Orchbee 自有模块。审查每次上游刷新，保留聚焦的兼容性调整，并测试 Orchbee 所拥有的行为或集成。

## 自定义规则归属

| 规则                                        | 权威来源                                              |
| ------------------------------------------- | ----------------------------------------------------- |
| `orchbee/component-directory`               | `docs/frontend/react.md#component-directory-boundary` |
| `orchbee/filename-naming-convention`        | `docs/TESTING.md#source-and-test-paths`               |
| `orchbee/module-boundaries`                 | `docs/MODULE-BOUNDARIES.md#module-boundaries`         |
| `orchbee/no-cross-workspace-source-imports` | `docs/ARCHITECTURE.md#package-boundaries`             |
| `orchbee/no-inline-suppressions`            | `docs/CODESTYLE.md#inline-suppressions`               |
| `orchbee/no-react-hooks-in-tsx`             | `docs/frontend/react.md#component-hook-boundary`      |
| `orchbee/one-function-per-tsx`              | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/one-hook-per-file`                 | `docs/frontend/react.md#hook-file-boundary`           |
| `orchbee/require-unit-test`                 | `docs/TESTING.md#required-unit-tests`                 |
| `orchbee/zustand-scope`                     | `docs/frontend/react.md#zustand-and-context`          |

每次自定义规则变更都要同时更新其实现、权威来源、诊断和回归测试。
