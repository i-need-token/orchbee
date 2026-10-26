# CLAUDE.md

> 本项目 AI Agent 工作指南。进行任何变更前请先阅读本文。

**这是什么项目**：Orchbee——一款现代终端，用于运行和编排 AI Agent，并连接桌面端、Web 端和移动端。

`CLAUDE.md` 是地图，不是百科全书。它只作为入口索引（保持在约 80 行以内），而不是知识库。仓库规范位于 `docs/`；包行为和维护参考由所属包保存。

## 硬性规则

- **英文是权威版本；每份仓库 Markdown 文档都必须双语提供。** 英文使用不带后缀的 `.md` 文件名。`docs/` 下的规范通过 `docs/zh-CN/` 下的相同路径配对；其他每份项目自有 Markdown 文件都与同目录的 `.zh-CN.md` 文件配对。发生冲突时以英文为准，运行时配置、诊断和工程权威来源使用英文版本。
- **双语文档绝不漂移。** 每次新增、删除、移动、结构调整、示例、链接或内容修改 Markdown 时，都必须在同一个变更中更新两种语言。完成语义审查后，使用 `pnpm docs:confirm-pairing -- <markdown-path>` 确认每个受影响的文档对；必需的 lint 检查会拒绝缺失、结构不同或未经确认的文档对。
- **产品文案必须本地化。** 每条面向产品用户的消息都通过本地化系统提供，并同时交付 `en-US` 和 `zh-CN`；locale 资源属于产品资产，可以使用对应目标语言。
- **文档权威来源遵循所有权。** 管理某项事实的仓库规范、包 README、包本地参考、提案或研究记录必须在该事实变化的同一个变更中更新。仓库规范不得盘点单个包的业务行为、源码树、界面组成或路线图；未来设计归入 `proposals/`。
- **每个已跟踪文件都必须具有持久的仓库价值。** 添加文件前，先确定其未来使用方、权威职责、重复用途和维护生命周期；任务历史和一次性工作产物不属于仓库。
- **自主完成已授权工作。** 遵循 `docs/DEVELOPMENT.md`：编辑前完成风险分级，把明确请求视为对范围内完整结果的授权，并连续完成实现、检查和常规修复。升级新的保留决策，只暂停依赖它的工作。
- **日常验证不要求全量构建。** 普通修改和合并使用 `docs/VERIFICATION.md` 中按影响范围选择的本机门禁。只有变更涉及的构建、打包或平台契约需要时，才通过显式命令执行完整应用构建和交叉编译；不要仅为合并受检输入未变的代码而重复门禁。
- **非必要不新增单元测试。** Rust、Zig 和 TypeScript 采用 `docs/TESTING.md` 中的准入与删除规则：新增前说明实质证据缺口，清理低价值伴随测试，同时保留必要的行为与回归证据。
- **尽早处理结构异味。** 当范围内变更暴露明显的增长、耦合、所有权歧义或边界侵蚀风险时，在它扩散前解决；如果补救会实质性扩大范围，继续前先说明风险。
- **`AGENTS.md` 和 `AGENTS.zh-CN.md` 是指向各自 `CLAUDE` 对应文件的符号链接。** 只能编辑 `CLAUDE.md` 和 `CLAUDE.zh-CN.md`。
- **绝不能为规避错误而修改 lint 配置。** 修复代码，不要修改配置。变更 `.oxlintrc.json` 必须取得明确的人工批准。
- **Desktop 开发诊断必须定位开发应用身份。** 检查 `pnpm desktop` 时，必须按精确路径或 checkout 专属 bundle identifier 定位生成的 `Orchbee Dev.app`。除非人类在当前任务中明确要求操作已安装的正式应用，否则绝不能打开、聚焦、退出、替换 `/Applications/Orchbee.app`，也不能把它作为回退目标。
- **会抢占焦点的 Desktop 原生窗口系统测试只能手动运行。** 除非人类在当前请求中明确授权该项测试，否则绝不能调用 `test:system:manual:native-window` 或设置 `ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL`；聚合、完整、CI 和自主验证必须排除它。参见 `apps/desktop/README.md#manual-native-window-test`。

## 核心文档索引

- **架构**：`docs/ARCHITECTURE.md` — 系统架构、依赖方向和数据流
- **术语表**：`docs/GLOSSARY.md` — 产品概念、规范名称、身份边界和本地化术语
- **跨端界面设计**：`docs/INTERFACE-DESIGN.md` — GUI/TUI 共同语义、快捷键迁移、视觉角色、交互模式和双端交付
- **应用**：`docs/APPLICATIONS.md` — 可运行工作区的位置、形态和生命周期
- **模块边界**：`docs/MODULE-BOUNDARIES.md` — 语义模块设计、源码树可见性、入口点、组合和环
- **包**：`docs/PACKAGES.md` — 包命名、导出、构建、依赖和版本控制
- **项目**：`docs/PROJECTS.md` — 机器可读的工作区范围与职责契约
- **代码风格**：`docs/CODESTYLE.md` — 注释、命名和文件结构约定
- **文档**：`docs/DOCUMENTATION.md` — 添加、移动、重构文档或修改已记录的工程契约前阅读
- **仓库卫生**：`docs/REPOSITORY-HYGIENE.md` — 已跟踪文件准入、临时产物、生成文件和交接前审查
- **Lint**：`docs/LINTING.md` — 命令、强制执行边界和自定义规则所有权
- **验证**：`docs/VERIFICATION.md` — 权威的本地和 CI 质量门禁
- **开发**：`docs/DEVELOPMENT.md` — 范围授权、风险分级、连续交付和收敛
- **建模与正确性**：`docs/MODELING-AND-CORRECTNESS.md` — 模型优先的行为设计和基于风险的正确性证据
- **TypeScript**：`docs/TYPESCRIPT.md` — 零妥协类型安全和禁用模式
- **Rust**：`docs/RUST.md` — Cargo 工作区位置、crate 形态、依赖准入、平台门控、所有权、安全性、并发、进程边界、本地化、契约和验证
- **Zig**：`docs/ZIG.md` — 原生命名、资源所有权、安全性、并发和测试
- **测试**：`docs/TESTING.md` — 测试准入、删除、布局、执行和回归证据
- **前端包边界**：`docs/frontend/package-boundaries.md` — 前端工作区位置、语义所有权、依赖方向和平台端口
- **前端 React**：`docs/frontend/react.md` — 组件所有权、文件边界、状态、Effect、Hook、Zustand 和测试
- **前端体验组件**：`docs/frontend/experience-components.md` — 已审查的交互词汇、包本地 Manifest、发现、组合约束和生命周期
- **前端对话框与表单**：`docs/frontend/dialogs-and-forms.md` — 中断预算、表面准入、问题预算、取消、焦点和证据
- **前端设置**：`docs/frontend/settings.md` — 设置项准入、视觉组合、作用域、控件、提交、重置和证据
- **前端视觉基础**：`docs/frontend/visual-foundations.md` — 字体、层级、间距、圆角、尺寸、边框和表面选择规则
- **前端设计 Token**：`docs/frontend/design-tokens.md` — Token 所有权、层级、命名、主题、语义覆盖和变更
- **前端样式**：`docs/frontend/styling.md` — CSS Module、生成的 shadcn 边界、图标、样式表和验证
- **前端 Storybook**：`docs/frontend/storybook.md` — 宿主所有权、Story 位置、导航、编写和浏览器验证
- **前端本地化**：`docs/frontend/localization.md` — locale 所有权、消息目录、格式化、回退和验证
- **前端菜单**：`docs/frontend/menus.md` — 菜单触发器、几何、内容、交互状态、键盘行为和验证
- **前端 Tooltip**：`docs/frontend/tooltips.md` — Provider 所有权、延迟、内容、可访问性和验证
- **前端 Action**：`docs/frontend/actions.md` — 语义 Action、注册、可用性、同步和异步执行、取消、并发和验证
- **前端键盘快捷键**：`docs/frontend/keyboard-shortcuts.md` — Action 所有权、绑定、平台显示、冲突和验证
- **前端浏览器存储**：`docs/frontend/browser-storage.md` — 本地持久化所有权、键版本控制、验证、隐私和验证
- **TUI 视觉基础**：`docs/tui/visual-foundations.md` — 颜色角色、主题契约、文本层级、框架、字形和显示宽度
- **TUI 组件**：`docs/tui/components.md` — 组件归属、复用、控件状态、动作可辨识性、列表、编辑器、选择器及共享 chrome
- **TUI 动作**：`docs/tui/actions.md` — 语义身份、可用性、调用、执行、并发、取消和结果
- **TUI 设置**：`docs/tui/settings.md` — 设置元数据、作用域、值状态、预览、提交、重置及持久化反馈
- **TUI 布局**：`docs/tui/layout.md` — 区域、尺寸层级、降级顺序、覆盖层几何、截断和尺寸变化
- **TUI 键盘输入**：`docs/tui/keyboard-input.md` — 按键解码、绑定模型、输入归属、编辑约定和发现
- **TUI 鼠标输入**：`docs/tui/mouse-input.md` — 捕获策略、命中测试、手势词汇、透传、滚轮归一化和选择
- **TUI 覆盖层与状态**：`docs/tui/overlays-and-status.md` — 表面选择、覆盖层组合、确认、状态行、通知和等待指示
- **TUI 终端环境**：`docs/tui/terminal-environment.md` — 宿主准入、模式生命周期、能力探测、输出纪律和恢复
- **TUI 验证**：`docs/tui/verification.md` — 终端界面的结构前提以及渲染、输入、环境和宿主证据
- **CLI 命令面**：`docs/cli/command-surface.md` — 语法权威、命令路径、选项、target、帮助、补全、破坏性命令、Agent 可发现性和环境变量
- **CLI 输出**：`docs/cli/output.md` — 流划分、格式、机器文档、退出码、文本渲染、上限、流式记录和本地化
- **CLI 进程**：`docs/cli/process.md` — 输入信任边界、解析先于效果、截止时间、副作用、请求身份、信号和运行时发现
- **CLI 验证**：`docs/cli/verification.md` — 命令行面的结构前提以及语法、效果、退出码、文本、上限和进程证据

## 项目结构

```
apps/           # 可独立运行的产品与开发应用
crates/         # 为真实跨 crate 使用方准入的可复用 Rust 库
packages/       # 共享 JavaScript 和 TypeScript 库、指南和包本地参考
docs/           # 仓库级规范和跨工作区架构契约
proposals/      # 审查中的未来设计；绝不是当前权威来源
schemas/        # 机器可读的仓库契约 Schema
```

## 技术栈

- **前端**：React + TypeScript 7.0（严格模式，`noUncheckedIndexedAccess`、`erasableSyntaxOnly`、`verbatimModuleSyntax`）
- **路由**：TanStack Router
- **客户端状态**：Zustand
- **UI**：使用 Base UI 原语的 shadcn/ui
- **原生工具链**：Rust 1.97.0 + Zig 0.16.0 和固定版本的 Ghostty VT
- **工具运行时**：Node.js（>=22）
- **Lint**：oxlint（使用 typescript、unicorn、oxc、react、import 插件）
- **格式化**：oxfmt + rustfmt + `zig fmt`
- **Schema**：Zod 4（在边界 parse，内部信任）
- **Monorepo**：pnpm workspaces（`apps/*`、`packages/*`）+ 一个根 Cargo workspace（`apps/*`、`crates/*` Rust member）

<!-- documentation-pairing: english-sha256=84e1c03e7c491c9e8731f953e4ed401af2dd84bf19f75afcbdcda621936bfa13 zh-cn-sha256=9a2360f893959a8c8fdda150f62dc39ddb55107d73924dc9745de8f73f0f9749 -->
