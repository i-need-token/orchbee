# 配置权威

| 字段     | 值                                                                                                                                                                                                                                                              |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 已接受，2026-10-24                                                                                                                                                                                                                                              |
| 负责人   | 原生 Client、Server、Worker 与设置边界的维护者                                                                                                                                                                                                                  |
| 现行权威 | [`windows-cli-configuration.md`](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md)、[`native-settings.md`](../../apps/orchbee/docs/native-settings.zh-CN.md)、[`user-preferences.md`](../../crates/workbench-protocol/docs/user-preferences.zh-CN.md) |
| 决策权威 | 产品决策人于 2026-10-24 接受存储分工、Server 权威、订阅分发与 Client 最小读取集                                                                                                                                                                                 |
| 退出条件 | 下文每个交付步骤均已实施，其当前行为已写入现行权威，并删除本提案                                                                                                                                                                                                |

英文规范原文：
[configuration-authority.md](configuration-authority.md)。

## 决策

- TUI 或 Desktop 设置界面能修改的设置都归 `preferences.json`；只能手工编辑的设置归 `config.toml`。同一项设置不得同时出现在两个存储中；重叠的键连同读取代码从不拥有它的存储中删除。
- Server 是唯一的运行时读取方。它读取两个存储，校验并合并默认值，经共享承载连接上的状态订阅向 Client 发布每个 revision 的一份生效快照。Worker 从 Server 分发中获得 launch 所需的用户意图，并自行解析系统 Shell 发现等设备事实。被拒绝的候选保留上一份有效快照，并发布诊断。
- Worker 不读取任何一个存储。
- Client 只为下文的最小读取集直接读取 `config.toml`；其他 Client 消费方都使用 Server 快照。
- Desktop 继续通过 `orchbee-settings` 读写 `preferences.json`。Server 通过偏好采样截止时间感知这些写入。
- 在 Server 与 Client 最小读取集共用 `user_settings` 期间，它留在 `orchbee-local-runtime`。

## 存储归属

| 存储               | 设置                                                                                                                                                                                                                                                                                                              |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `preferences.json` | `locale`、`applicationTheme`、`terminalDisplayTheme`、`customForeground`、`customBackground`、`copyOnSelect`、`trimTrailingBlankRowsForAgents`、`agentNotificationMode`、`completionReminderMode`、`completionReminderMinimumSeconds`、`soundEnabled`、`scrollbackBytes`、`launchProfiles`                        |
| `config.toml`      | `terminal.new_cwd`、`terminal.focus_new_panel`、`worktrees.directory`、`update.channel`、`update.manifest_check`、包含自定义命令在内的 `keys.*`、`ui.window_title`、`ui.sidebar.agents.*`、`ui.sidebar.spaces.*`、`ui.toast.delivery`、`ui.toast.delay_seconds`、`ui.toast.overlay.position`、`ui.sound.agents.*` |

## 重叠处理

| 设置         | 归属                                                                                                              | 删除                                                                                                                                                |
| ------------ | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| 界面主题     | `applicationTheme`                                                                                                | 设置中只占位、从不解析赋值的主题字段，Configuration `themeName`，以及 `config check` 与 `server config` 回执中的 `themeName`                        |
| 默认启动程序 | `launchProfiles`                                                                                                  | `terminal.default_shell`、`terminal.shell_mode`、它们的私有启动参数、Server 默认 shell 准入，以及 Configuration 回执中的 `defaultShell`/`shellMode` |
| 通知         | 按含义拆分：Agent 提醒方式与全局声音仍是共享偏好；手动投递、延迟、浮层位置与按 Agent 的声音覆盖仍在 `config.toml` | 无                                                                                                                                                  |

## Client 最小读取集

| 读取方            | 设置                     | 原因                                               |
| ----------------- | ------------------------ | -------------------------------------------------- |
| 本地 Session 激活 | 配置文件位置及文件有效性 | 拒绝无法读取的文件，并冻结 Server 只读取一次的位置 |
| 更新通道与更新    | `update.channel`         | 发布相关操作不依赖 Session                         |
| `config check`    | 整个文件                 | 校验用户编辑的文件，不应用任何运行时策略           |

## 交付

| 步骤 | 结果                                                                                                                                           | 状态                                                                                           |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 1    | 删除两处重叠；Worker 不再读取 `config.toml`。                                                                                                  | 已交付                                                                                         |
| 2    | Server 在启动时和每次重载时各读取一次 `config.toml`；Agent manifest 重载与进程观测使用已提交的配置。                                           | 已交付                                                                                         |
| 3    | Server 采样冻结的 `config.toml` 位置，文件变化时按显式重载的语义重载并发布。                                                                   | 已交付                                                                                         |
| 4    | Server 准入从 `preferences.json` 捕获 launch 默认值；Worker 不再打开它。                                                                       | 已交付                                                                                         |
| 5    | Configuration 快照携带 `terminal.focus_new_panel`、`terminal.new_cwd` 与 `worktrees.directory`；TUI 启动、Worktree 与 CLI 创建的读取改用快照。 | 部分交付：快照携带 `focusNewPanel` 与 `worktreesDirectory`，原生 Client 不再读取 `config.toml` |
| 6    | 现行权威描述已交付的行为；删除本提案。                                                                                                         | 未开始                                                                                         |

每个步骤都通过受影响的 Rust 检查和现有配置测试。本 issue 在 `pnpm verify:changed` 与
`pnpm verify:rust-native` 通过后关闭。

### 剩余工作

- 第 5 步仍让 CLI 的 Workspace、Tab、Panel 与 `worktree` 创建为 `terminal.new_cwd` 和
  `worktrees.directory` 读取 `config.toml`。迁移需要在快照中加入 `terminal.new_cwd`，并让每个创建命令
  在首个副作用前请求当前快照；此后这些命令的配置错误改由 Server 报告。

<!-- documentation-pairing: english-sha256=b8d36ba8309f3b39777171264d3536fe9f9213e58eb3b20e375553628b0a149b zh-cn-sha256=6fa5cdc94de392ddce47597374d98fc527707bcbcc8f3fb45c3fa0123fdea03f -->
