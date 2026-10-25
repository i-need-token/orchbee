# Lint

## 命令

| 命令                                           | 职责                                                                                                     |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `pnpm verify`                                  | 运行 `docs/VERIFICATION.md` 定义的完整合并门禁                                                           |
| `pnpm lint`                                    | 运行完整的必需仓库 lint 流程                                                                             |
| `pnpm lint:code`                               | 以面向 Agent 的输出运行会阻塞的 oxlint 诊断                                                              |
| `pnpm lint:code:warnings`                      | 审计会阻塞的诊断和不阻塞的 oxlint warning                                                                |
| `pnpm lint:rust`                               | 验证固定 Cargo workspace，并在 member 存在时运行 locked Clippy 工作                                      |
| `pnpm lint:documentation`                      | 验证每个仓库 Markdown 文档对的路径、凭据和结构                                                           |
| `pnpm lint:styles`                             | 验证 Token 注册表和自有前端样式契约                                                                      |
| `pnpm lint:storage`                            | 验证浏览器存储访问、所有权、key 形态和版本                                                               |
| `pnpm lint:dependencies`                       | 运行完整与严格 Knip 分析，再判定明确保留的开发消费者                                                     |
| `pnpm lint:projects`                           | 验证每个基于 Schema 的工作区项目契约                                                                     |
| `pnpm lint:experience`                         | 验证体验组件 Schema、所有权、路径和关系                                                                  |
| `pnpm lint:fix`                                | 应用安全的 oxlint 修复；剩余诊断需要修改代码                                                             |
| `pnpm typecheck`                               | 运行语法感知 lint 无法替代的 TypeScript 诊断                                                             |
| `pnpm test:lint-rules`                         | 测试 `orchbee/*` 规则和中英文规范契约                                                                    |
| `pnpm test:dependency-scope`                   | 使用隔离的正向和负向 fixture 验证完整与严格 Knip 图的覆盖                                                |
| `pnpm test:rust-runner`                        | 通过隔离 Cargo 夹具测试 Rust target 覆盖、失败传播、依赖继承/默认 feature 以及自有代码的 unsafe/导入策略 |
| `pnpm test:documentation-pairing`              | 运行文档配对检查与确认流程的负向测试                                                                     |
| `pnpm test:ghostty-source`                     | 测试 Ghostty 来源工具的精确 pin 与干净工作树准入                                                         |
| `pnpm test:style-contract`                     | 运行前端样式契约检查器的回归测试                                                                         |
| `pnpm test:storage-contract`                   | 运行浏览器存储契约检查器的回归测试                                                                       |
| `pnpm test:experience-contract`                | 运行体验目录与查找契约的回归测试                                                                         |
| `pnpm docs:confirm-pairing -- <markdown-path>` | 完成语义审查后仅更新显式指定的凭据                                                                       |

## 强制执行边界

| 关注点        | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 规则权威      | 仓库规范定义显式配置规则和 `orchbee/*` 规则；已安装的 oxlint 版本管理上游分类规则。                                                                                                                                                                                                                                                                                                                                                                 |
| 文档与配置    | 文档中标记为已强制执行的每条规则都必须在 `.oxlintrc.json` 中启用；`pnpm test:lint-rules` 同时验证文档到配置和配置到文档的覆盖。                                                                                                                                                                                                                                                                                                                     |
| 规范结构      | `pnpm test:lint-rules` 强制执行 `docs/zh-CN/` 一一对应、结构、内联标识符、本地化链接、`docs/` 下路径准入，以及具体 `apps/<name>` 和 `packages/<name>` 引用存在性。                                                                                                                                                                                                                                                                                  |
| Markdown 内容 | `pnpm lint:documentation` 会发现已跟踪和未被忽略的候选 `.md` 文件，强制执行 `docs/zh-CN/` 镜像或同目录 `.zh-CN.md` 命名契约，并将每个中英文文档对与 `scripts/documentation-pairing.json` 中的 SHA-256 凭据比较。完成语义审查后，`pnpm docs:confirm-pairing -- <markdown-path>` 只更新指定的文档对；凭据能发现未经确认的内容修改，但无法判断翻译忠实度。                                                                                             |
| 严重性        | 错误使 `pnpm lint` 失败；常规 lint 输出会省略不阻塞的 warning，以保持自动化反馈简洁。审查受影响包文件中的 warning 时，运行 `pnpm lint:code:warnings`。                                                                                                                                                                                                                                                                                              |
| 依赖          | 依赖命令保留完整和严格 Knip 分析及原始结果。未登记的未使用文件、导出或类型，以及全部依赖、解析、重复导出或过期 catalog 错误都会失败。精确开发保留遵循[保留开发消费者](#保留开发消费者)。                                                                                                                                                                                                                                                            |
| Knip 范围     | 包清单、工具插件或 `knip.json#workspaces` 必须能发现每个可运行脚本、构建配置、测试套件和公共包入口。只补充遗漏的真实入口；冗余入口和过期配置提示都会失败。共享 fixture 是经测试消费者访问的项目文件，不能将其伪装成入口。仅生产模式排除测试辅助文件时，完整图仍须分析它们；`pnpm test:dependency-scope` 验证可达性、孤立文件拒绝和严格依赖隔离。 私有包启用 `includeEntryExports`，入口导出也必须有消费者；不能通过假入口或测试专用外观隐藏死 API。 |
| 类型感知      | 包审查加入 `oxlint-tsgolint` 前，类型感知 oxlint 规则保持未启用；lint 不会检测悬空 Promise 或来自第三方的不安全 `any` 传播。                                                                                                                                                                                                                                                                                                                        |
| Rust          | `pnpm lint:rust` 验证精确工具链与锁定的 Cargo metadata，要求共享第三方依赖通过 `workspace = true` 继承，并拒绝未直接或经继承设置 `default-features = false` 的第三方声明，包含未使用的 workspace 声明。随后对每个 target 和 feature 运行 workspace Clippy，并拒绝已启用的 warning。[`docs/RUST.md`](RUST.md#lint-策略与执行边界) 定义其余评审边界。                                                                                                 |
| 样式          | `pnpm lint:styles` 强制执行 [`docs/frontend/design-tokens.md`](frontend/design-tokens.md)：注册表、生成的 TypeScript 清单、CSS Module、Tailwind 边界、原始值、响应式阈值、JSX class 和内联样式。生成的公共 Token 名称必须等于 CSS 注册表；完整 Tailwind 映射必须精确覆盖已安装的公共主题、指向已注册的 Orchbee Token，并保留编译时字面量断点和容器值。                                                                                              |
| 样式例外      | `pnpm lint:styles` 的 shadcn 编写例外按路径限制在检入的生成源码和支持文件中。                                                                                                                                                                                                                                                                                                                                                                       |
| 浏览器存储    | `pnpm lint:storage` 强制执行 [`docs/frontend/browser-storage.md`](frontend/browser-storage.md)：只有应用组合入口可以访问 `localStorage` 或 `sessionStorage`；`orchbee:` key 字面量保留在所属的 `*-storage.ts` 模块中，并使用带版本的语法。                                                                                                                                                                                                          |
| 体验目录      | `pnpm lint:experience` 强制执行 [`docs/frontend/experience-components.md`](frontend/experience-components.md)：Schema 新鲜度、包本地所有权、所有者范围内路径存在、命名 Story 导出及其生成 ID、组件 ID 唯一，以及组合与废弃关系可解析。                                                                                                                                                                                                              |
| 策略变更      | 绝不能为了让现有代码通过而弱化规则；修复代码，或者就项目策略变更取得人工批准。                                                                                                                                                                                                                                                                                                                                                                      |

## 依赖图边界

精确 workspace 配置会替换匹配的 workspace 通配配置；省略的字段采用 Knip 默认值，而不会继承
通配配置。为私有 workspace 显式声明 `entry` 和 `project`，再添加 `includeEntryExports` 设置。
私有包在完整图和严格模式中都保持该设置开启。

在 `project` 中用 `!` 标记真实生产源码模式，让严格分析既记录入口，也记录实现文件内部的消费者。
通过所属 Storybook 配置和插件在开发图中发现 Story；其 `entry` 和 `project` 模式不属于生产模式。
测试和 Story 消费者不能在严格模式中让未使用的生产模块或私有入口 API 保持存活。只有工具使用的
生成清单继续留在完整图中，不能为它们伪造运行时消费者。

当来源文件参与被分析的图时，已解析的内部 type-only 导入也是消费者。根据实际 JavaScript 和声明
消费者区分运行时与开发依赖声明；仅凭一条严格模式诊断，不能断定某个依赖只用于类型。

`pnpm test:dependency-scope` 使用当前 workspace 图和插件设置，包括 `includeEntryExports`。
隔离应用通过非入口实现消费私有值和类型，包括打包后的子路径；其 Storybook host 实际验证基于配置的发现。负向见证
拒绝未消费的私有入口、严格模式中只有测试消费者的入口、孤儿辅助文件、只有测试或 Story 消费的
生产模块，以及借用的运行时依赖。这些 fixture 验证的是图策略，不代表仓库当前依赖检查结果。

## 保留开发消费者

`pnpm lint:dependencies` 调用 `node scripts/dependency-retentions.mjs`。它先用已安装的 Knip CLI
分析完整图，再以 `--strict` 分析，使用 `--reporter json` 和由
`scripts/dependency-retentions-reporter.config.mjs` 配置的第二个证据 reporter。
这些报告选项不改变分析范围或私有入口检查旗标。证据封装包含实际报告类别、计数、配置提示和
标签提示；Knip 内置 JSON reporter 单独使用时不包含这些提示。

完整图必须正常结束、原始退出码为零且没有诊断。完整图非零时立即失败，不执行 strict。
strict 原始退出码为零时，只有不存在保留记录才接受。strict 原始退出码为一时，只有全部诊断
逐条精确等于已批准、归包所有的 `(kind, file, name)` 记录，且每条记录仍有对应诊断，才能作出
独立的综合接受判定。仅单独的 `files`、`exports`、`types` 记录可准入。依赖、无法解析的导入、
其他类别、未知字段、重复项、提示、配置变化、解析错误、进程错误、信号和超时都失败。
数量、glob、ignore 和人工入口不能授权接受。原始 stdout、stderr、exit/signal/error 结果与综合
结果分开保存在打印出的仓库外日志目录中，包括综合接受原始 strict 退出码一的情况。

各包在 `dependency-retentions.json` 和 README 中拥有其已准入用途、真实开发消费者、下一次
集成负责人及解除条件。消费者文件存在不足以成立：已审阅完整源码的指纹绑定真实的渲染、调用、
断言用途、导入导出链及发现和解析配置。门禁在分析前后核对指纹，输入缺失或变化都失败。
更新摘要必须是包含重新语义审查和图证据的显式可审变更，不能自动更新基线。这些检查保持已经
审阅的用途，不证明新的语义，也不替代 build、typecheck、Story、行为与出口验证。
既有依赖范围负向见证仍然必需。

接入真实生产消费者或撤销用途时，必须在同一变更中删除对应记录。诊断消失，包括 strict 退出码
为零但仍有记录，都作为过期保留失败。删除已耗尽的记录、文件及其显式门禁登记。保留不授权
新公共入口、依赖或产品迁移。调用任务拥有原始日志，在最后一个诊断、审查或恢复消费者结束后回收。

## 生成的源码

| 范围               | 契约                                                                                                                                                                                                        |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 快照               | 让 `packages/ui/src/components/ui/*.tsx` 和生成的 `use-mobile.ts` 与上游 shadcn 注册表保持接近，使 CLI 刷新易于审查。                                                                                       |
| 编写例外           | 按路径限定的覆盖豁免显式模块边界返回类型、断言、遮蔽、相等性和嵌套三元表达式风格、组件目录及组件/Hook 导出边界、`use-mobile.ts` 的唯一符号文件名匹配。自动 JSX 运行时使 `react/react-in-jsx-scope` 不适用。 |
| 仍生效的正确性规则 | React Hooks、JSX key、模块边界、跨工作区导入、不安全 TypeScript 构造和内联抑制禁令仍然生效。                                                                                                                |
| `ChartStyle`       | 其上游控制的 style 元素是唯一额外例外；[`docs/frontend/react.md`](frontend/react.md#正确性与安全性) 负责该约束。                                                                                            |
| Orchbee 自有模块   | 绝不能把生成源码覆盖扩展到这些模块。                                                                                                                                                                        |
| 上游刷新           | 审查每次刷新，保留聚焦的兼容性调整，并测试 Orchbee 所拥有的行为和集成。                                                                                                                                     |

## 自定义规则归属

| 规则                                        | 权威来源                                              |
| ------------------------------------------- | ----------------------------------------------------- |
| `orchbee/component-directory`               | `docs/frontend/react.md#component-directory-boundary` |
| `orchbee/filename-naming-convention`        | `docs/TESTING.md#source-and-test-paths`               |
| `orchbee/module-boundaries`                 | `docs/MODULE-BOUNDARIES.md#module-boundaries`         |
| `orchbee/no-cross-workspace-source-imports` | `docs/ARCHITECTURE.md#package-boundaries`             |
| `orchbee/no-inline-suppressions`            | `docs/CODESTYLE.md#inline-suppressions`               |
| `orchbee/no-react-use-callback`             | `docs/frontend/react.md#callback-identity`            |
| `orchbee/component-export-boundary`         | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/hook-export-boundary`              | `docs/frontend/react.md#hook-file-boundary`           |
| `orchbee/zustand-scope`                     | `docs/frontend/react.md#zustand-and-context`          |

每次自定义规则变更都要同时更新其实现、权威来源、诊断和回归测试。
