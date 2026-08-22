# Lint

## 命令

| 命令                                           | 职责                                                 |
| ---------------------------------------------- | ---------------------------------------------------- |
| `pnpm verify`                                  | 运行 `docs/VERIFICATION.md` 定义的完整合并门禁       |
| `pnpm lint`                                    | 运行所有必需的代码、文档、样式、依赖、项目和体验检查 |
| `pnpm lint:code`                               | 以面向 Agent 的输出运行会阻塞的 oxlint 诊断          |
| `pnpm lint:code:warnings`                      | 审计会阻塞的诊断和不阻塞的 oxlint warning            |
| `pnpm lint:documentation`                      | 验证每个仓库 Markdown 文档对的路径、凭据和结构       |
| `pnpm lint:styles`                             | 验证 Token 注册表和自有前端样式契约                  |
| `pnpm lint:storage`                            | 验证浏览器存储访问、所有权、key 形态和版本           |
| `pnpm lint:dependencies`                       | 运行 Knip 严格依赖、文件、导出、catalog 和清单分析   |
| `pnpm lint:projects`                           | 验证每个基于 Schema 的工作区项目契约                 |
| `pnpm lint:experience`                         | 验证体验组件 Schema、所有权、路径和关系              |
| `pnpm lint:fix`                                | 应用安全的 oxlint 修复；剩余诊断需要修改代码         |
| `pnpm typecheck`                               | 运行语法感知 lint 无法替代的 TypeScript 诊断         |
| `pnpm test:lint-rules`                         | 测试 `orchbee/*` 规则和中英文规范契约                |
| `pnpm test:documentation-pairing`              | 运行文档配对检查与确认流程的负向测试                 |
| `pnpm test:style-contract`                     | 运行前端样式契约检查器的回归测试                     |
| `pnpm test:storage-contract`                   | 运行浏览器存储契约检查器的回归测试                   |
| `pnpm test:experience-contract`                | 运行体验目录与查找契约的回归测试                     |
| `pnpm docs:confirm-pairing -- <markdown-path>` | 完成语义审查后仅更新显式指定的凭据                   |

## 强制执行边界

| 关注点        | 契约                                                                                                                                                                                                                                                                                                                                                    |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 规则权威      | 仓库规范定义显式配置规则和 `orchbee/*` 规则；已安装的 oxlint 版本管理上游分类规则。                                                                                                                                                                                                                                                                     |
| 文档与配置    | 文档中标记为已强制执行的每条规则都必须在 `.oxlintrc.json` 中启用；`pnpm test:lint-rules` 同时验证文档到配置和配置到文档的覆盖。                                                                                                                                                                                                                         |
| 规范结构      | `pnpm test:lint-rules` 强制执行 `docs/zh-CN/` 一一对应、结构、内联标识符、本地化链接、`docs/` 下路径准入，以及具体 `apps/<name>` 和 `packages/<name>` 引用存在性。                                                                                                                                                                                      |
| Markdown 内容 | `pnpm lint:documentation` 会发现已跟踪和未被忽略的候选 `.md` 文件，强制执行 `docs/zh-CN/` 镜像或同目录 `.zh-CN.md` 命名契约，并将每个中英文文档对与 `scripts/documentation-pairing.json` 中的 SHA-256 凭据比较。完成语义审查后，`pnpm docs:confirm-pairing -- <markdown-path>` 只更新指定的文档对；凭据能发现未经确认的内容修改，但无法判断翻译忠实度。 |
| 严重性        | 错误使 `pnpm lint` 失败；常规 lint 输出会省略不阻塞的 warning，以保持自动化反馈简洁。审查受影响包文件中的 warning 时，运行 `pnpm lint:code:warnings`。                                                                                                                                                                                                  |
| 依赖          | `pnpm lint:dependencies` 运行 Knip `--strict`；直接导入未声明依赖、未使用的文件、依赖、导出或类型、无法解析的导入、重复导出、循环依赖和过期 catalog 条目都会失败。                                                                                                                                                                                      |
| Knip 范围     | 在 `knip.json#workspaces` 中列出所有可运行脚本、构建配置、测试套件和公共包入口；绝不能忽略由入口遗漏导致的发现。                                                                                                                                                                                                                                        |
| 类型感知      | 包审查加入 `oxlint-tsgolint` 前，类型感知 oxlint 规则保持未启用；lint 不会检测悬空 Promise 或来自第三方的不安全 `any` 传播。                                                                                                                                                                                                                            |
| 样式          | `pnpm lint:styles` 强制执行 [`docs/frontend/design-tokens.md`](frontend/design-tokens.md)：注册表、生成的 TypeScript 清单、CSS Module、Tailwind 边界、原始值、响应式阈值、JSX class 和内联样式。生成的公共 Token 名称必须等于 CSS 注册表；完整 Tailwind 映射必须精确覆盖已安装的公共主题、指向已注册的 Orchbee Token，并保留编译时字面量断点和容器值。  |
| 样式例外      | `pnpm lint:styles` 的 shadcn 编写例外按路径限制在检入的生成源码和支持文件中。                                                                                                                                                                                                                                                                           |
| 浏览器存储    | `pnpm lint:storage` 强制执行 [`docs/frontend/browser-storage.md`](frontend/browser-storage.md)：只有应用组合入口可以访问 `localStorage` 或 `sessionStorage`；`orchbee:` key 字面量保留在所属的 `*-storage.ts` 模块中，并使用带版本的语法。                                                                                                              |
| 体验目录      | `pnpm lint:experience` 强制执行 [`docs/frontend/experience-components.md`](frontend/experience-components.md)：Schema 新鲜度、包本地所有权、所有者范围内路径存在、命名 Story 导出及其生成 ID、组件 ID 唯一，以及组合与废弃关系可解析。                                                                                                                  |
| 策略变更      | 绝不能为了让现有代码通过而弱化规则；修复代码，或者就项目策略变更取得人工批准。                                                                                                                                                                                                                                                                          |

## 生成的源码

| 范围               | 契约                                                                                                                                                                                                                                                                           |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 快照               | 让 `packages/ui/src/components/ui/*.tsx` 和生成的 `use-mobile.ts` 与上游 shadcn 注册表保持接近，使 CLI 刷新易于审查。                                                                                                                                                          |
| 编写例外           | 按路径限定的覆盖豁免显式模块边界返回类型、断言、遮蔽、相等性和嵌套三元表达式风格、组件分组、TSX 中直接调用 React Hook、TSX 额外运行时声明、TSX 单函数与 Hook 单文件边界、`use-mobile.ts` 的唯一符号文件名匹配及同级测试。自动 JSX 运行时使 `react/react-in-jsx-scope` 不适用。 |
| 仍生效的正确性规则 | React Hooks、JSX key、模块边界、跨工作区导入、不安全 TypeScript 构造和内联抑制禁令仍然生效。                                                                                                                                                                                   |
| `ChartStyle`       | 其上游控制的 style 元素是唯一额外例外；[`docs/frontend/react.md`](frontend/react.md#正确性与安全性) 负责该约束。                                                                                                                                                               |
| Orchbee 自有模块   | 绝不能把生成源码覆盖扩展到这些模块。                                                                                                                                                                                                                                           |
| 上游刷新           | 审查每次刷新，保留聚焦的兼容性调整，并测试 Orchbee 所拥有的行为和集成。                                                                                                                                                                                                        |

## 自定义规则归属

| 规则                                           | 权威来源                                              |
| ---------------------------------------------- | ----------------------------------------------------- |
| `orchbee/component-directory`                  | `docs/frontend/react.md#component-directory-boundary` |
| `orchbee/filename-naming-convention`           | `docs/TESTING.md#source-and-test-paths`               |
| `orchbee/module-boundaries`                    | `docs/MODULE-BOUNDARIES.md#module-boundaries`         |
| `orchbee/no-cross-workspace-source-imports`    | `docs/ARCHITECTURE.md#package-boundaries`             |
| `orchbee/no-extra-runtime-declarations-in-tsx` | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/no-inline-suppressions`               | `docs/CODESTYLE.md#inline-suppressions`               |
| `orchbee/no-react-hooks-in-tsx`                | `docs/frontend/react.md#component-hook-boundary`      |
| `orchbee/no-react-use-callback`                | `docs/frontend/react.md#callback-identity`            |
| `orchbee/one-function-per-tsx`                 | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/one-hook-per-file`                    | `docs/frontend/react.md#hook-file-boundary`           |
| `orchbee/require-unit-test`                    | `docs/TESTING.md#required-unit-tests`                 |
| `orchbee/zustand-scope`                        | `docs/frontend/react.md#zustand-and-context`          |

每次自定义规则变更都要同时更新其实现、权威来源、诊断和回归测试。
