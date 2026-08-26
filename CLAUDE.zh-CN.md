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
- **编辑受治理产物前先对变更分类。** 遵循 `docs/DEVELOPMENT.md`。行为、结构和后果重大的工作在编辑受治理产物前需要已接受的变更授权；出现新的保留决策时返回其所属阶段，不得静默写入。
- **尽早处理结构异味。** 当范围内变更暴露明显的增长、耦合、所有权歧义或边界侵蚀风险时，在它扩散前解决；如果补救会实质性扩大范围，继续前先说明风险。
- **`AGENTS.md` 和 `AGENTS.zh-CN.md` 是指向各自 `CLAUDE` 对应文件的符号链接。** 只能编辑 `CLAUDE.md` 和 `CLAUDE.zh-CN.md`。
- **绝不能为规避错误而修改 lint 配置。** 修复代码，不要修改配置。变更 `.oxlintrc.json` 必须取得明确的人工批准。
- **会抢占焦点的 Desktop 原生窗口系统测试只能手动运行。** 除非人类在当前请求中明确授权该项测试，否则绝不能调用 `test:system:manual:native-window` 或设置 `ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL`；聚合、完整、CI 和自主验证必须排除它。参见 `apps/desktop/README.md#manual-native-window-test`。

## 核心文档索引

- **架构**：`docs/ARCHITECTURE.md` — 系统架构、依赖方向和数据流
- **应用**：`docs/APPLICATIONS.md` — 可运行工作区的位置、形态和生命周期
- **模块边界**：`docs/MODULE-BOUNDARIES.md` — 语义模块设计、源码树可见性、入口点、组合和环
- **包**：`docs/PACKAGES.md` — 包命名、导出、构建、依赖和版本控制
- **项目**：`docs/PROJECTS.md` — 机器可读的工作区范围与职责契约
- **代码风格**：`docs/CODESTYLE.md` — 注释、命名和文件结构约定
- **文档**：`docs/DOCUMENTATION.md` — 添加、移动、重构文档或修改已记录的工程契约前阅读
- **仓库卫生**：`docs/REPOSITORY-HYGIENE.md` — 已跟踪文件准入、临时产物、生成文件和交接前审查
- **Lint**：`docs/LINTING.md` — 命令、强制执行边界和自定义规则所有权
- **验证**：`docs/VERIFICATION.md` — 权威的本地和 CI 质量门禁
- **开发**：`docs/DEVELOPMENT.md` — 变更分类、决策权、分阶段受治理写入控制和收敛
- **建模与正确性**：`docs/MODELING-AND-CORRECTNESS.md` — 模型优先的行为设计和基于风险的正确性证据
- **TypeScript**：`docs/TYPESCRIPT.md` — 零妥协类型安全和禁用模式
- **Zig**：`docs/ZIG.md` — 原生命名、资源所有权、安全性、并发和测试
- **测试**：`docs/TESTING.md` — 测试布局、执行和回归要求
- **前端包边界**：`docs/frontend/package-boundaries.md` — 前端工作区位置、语义所有权、依赖方向和平台端口
- **前端 React**：`docs/frontend/react.md` — 组件所有权、文件边界、状态、Effect、Hook、Zustand 和测试
- **前端体验组件**：`docs/frontend/experience-components.md` — 已审查的交互词汇、包本地 Manifest、发现、组合约束和生命周期
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

## 项目结构

```
apps/           # 可独立运行的产品与开发应用
packages/       # 共享库及其公共指南和包本地参考
docs/           # 仓库级规范和跨工作区架构契约
proposals/      # 审查中的未来设计；绝不是当前权威来源
research/       # 有时间边界的研究；绝不是权威来源
prompts/        # AI 驱动工作的任务规范
schemas/        # 机器可读的仓库契约 Schema
```

## 技术栈

- **前端**：React + TypeScript 7.0（严格模式，`noUncheckedIndexedAccess`、`erasableSyntaxOnly`、`verbatimModuleSyntax`）
- **路由**：TanStack Router
- **客户端状态**：Zustand
- **UI**：使用 Base UI 原语的 shadcn/ui
- **原生运行时**：Zig 0.16.0 + 固定版本的 Ghostty VT
- **工具运行时**：Node.js（>=22）
- **Lint**：oxlint（使用 typescript、unicorn、oxc、react、import 插件）
- **格式化**：oxfmt + `zig fmt`
- **Schema**：Zod 4（在边界 parse，内部信任）
- **Monorepo**：pnpm workspaces（`apps/*`、`packages/*`）
