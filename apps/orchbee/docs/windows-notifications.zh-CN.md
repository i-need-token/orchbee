# Windows CLI 通知

`notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]` 向已有 Session 提交。标题/正文规范化会合并空白、移除控制字符，并截断为 80/240 个 Unicode 标量；规范化后标题为空则在提交前失败。省略位置时使用各 Client 配置（默认右下），声音默认 none。线协议同时限制规范字符串的标量数和 UTF-8 字节数。

## 交付模型

现有 Server Workbench 服务拥有临时通知注册表。监听者属于认证连接的 generation，五秒未轮询则过期；每个监听者最多一个待取消息，消息也限时五秒。最多 32 个监听者、256 个请求回执，回执保留 60 秒。成功 Show 向所有当前可接收的在线主 TUI 监听者排入通知，并开始一秒限流。disabled、busy、rate-limited、no-foreground-client 不投递。保留期内精确重放返回原结果而不再次排入；相同身份的不同请求失败。传输不确定后不重试变更。成功回执表示入队，不保证实际显示或声音播放。监听者只取一次；断连和过期释放待处理状态。Session 重启不保留通知。

Client 自有的可取消连接独立于输入/绘制进行轮询，只有 UI 所有者安装通知。启用卡片投递时，单个五秒临时卡片出现在内容区的指定角落，不盖住状态行，不捕获键盘/鼠标，不移动焦点，不改变布局。卡片模式下，已有提示、对话框和覆盖层会暂停接收；竞态下丢弃到达的卡片，不替换用户拥有的界面。空闲到期也触发重绘。显示文字转义终端及方向格式控制字符，按终端显示宽度裁剪。成功绘制后可请求一次安全 Windows 声音：done 使用信息声，request 使用感叹声，none 静音。系统声音策略可能抑制播放。外部投递的声音请求独立于视觉成功；system 模式请求 Windows 通知区域气泡。任何投递均不激活原生窗口，不跳转到 Agent 目标。

## 边界与证据

Herdr 参照为 `e0507237ad8d41f60aece10d00ef29ce2339ca34`，尤其是 `server/headless/notifications.rs`：Client 交付策略与 Server 回退策略不同。原生自定义通知遵循 Client 投递策略及省略位置继承。自动 Agent 通知在下文规定；剪贴板策略、自定义音频和目标跳转仍属独立对齐工作。私有 cohort 匹配升级/回退，不新增持久化格式、依赖版本或无界队列。

定向证据覆盖文本规范化边界、独立协议校验、注册表过期/重放/扇出/背压、实际 Windows CLI 管道、关闭、Unicode 四角渲染、空闲过期及输入归属保持。不代表已认证 provider、实际 Windows 视觉/声音或最终全量回归验收。

## 自定义通知投递配置

Windows `[ui.toast] delivery` 选择 `off`（默认）、`orchbee`（`herdr` 为导入别名）、`terminal` 或 `system`。`[ui.toast.herdr] position` 用既有四个 CLI 位置名称选择默认角落。CLI 省略位置时使用各接收 Client 的配置，显式位置优先。启动使用 Client 配置；显式 Session 配置发布将有界规范通知配置与键位一起应用，致命来源错误或畸形私有发布同时保留旧策略。其他 Agent/剪贴板/声音偏好不属于本批自定义通知范围。

Off 关闭视觉提示，显式请求的自定义声音仍独立生效。没有 Client 时，Server 的 off 策略返回 `disabled`，否则返回 `no-foreground-client`。连接中的 Client 即使使用 off，也可以将传输接纳报告为 `shown`。策略变化丢弃未投递效果和卡片，不重放此前消息。

终端投递使用检测到的 Ghostty/iTerm/WezTerm OSC 9 或 Kitty OSC 99，必要时进行 tmux 转义。只有 TUI 生命周期所有者在完整帧之后写入，恢复开始后不再写入；未知宿主显示非致命本地化提示。系统投递拥有一个隐藏、不激活的 Windows 通知窗口/线程、有界收件箱及通知区域图标。既有 winsafe 安全 GUI 封装负责窗口创建和消息派发，不编写 unsafe 代码、不启动外部辅助进程。100 毫秒定时器检测关闭，十秒后释放图标，并在关闭窗口前删除图标；Drop 取消并 join。队列接纳不确认 OS 绘制，OS 策略可能抑制气泡。物理宿主/视觉/声音验收仍后置。

## 自动物理 Agent 通知

Windows 主 TUI Client 可以为已检测到的终端 Agent 状态变化发提醒。第一阶段只使用完整的
物理观察；仅有规范观察的 Agent 不进入模型，也不会凭推断合并规范与物理观察。通知只带
规范化的已检测 Agent 标签及有界 workspace、Tab、pane 上下文，绝不包含 Agent 提示、
回复、报告消息或终端内容。

自动策略与通知共用有界配置 profile，但与手动 CLI 投递使用不同设置：

```toml
[ui.toast]
delivery = "off"                 # 手动 `notification show`；保留现有默认值
agent_delivery = "orchbee"       # 自动 Agent 弹窗；默认值
delay_seconds = 1                # 0 到 3600

[ui.sound]
enabled = false                  # 自动 Agent 声音；默认值

[ui.sound.agents]
codex = "off"                    # default、on 或 off
```

`agent_delivery` 接受 `off`、`orchbee`（导入别名 `herdr`）、`terminal` 或 `system`。
它不会改变既有 `delivery` 默认值，也不改变 `notification show` 的显式声音。
`[ui.sound]` 只管理自动声音。全局 `enabled = false` 是硬关闭；启用声音后，按 Agent
配置的 `off` 会静音对应检测标签，`default` 或 `on` 允许内置提示音。没有 Agent 默认静音。
Agent 键使用小写检测器 ID，最长 64 个 ASCII 字母、数字、点或连字符，首尾必须为字母或数字。
done 使用 Windows 信息提示音，request 使用 Windows 感叹提示音。本契约不包含自定义
文件、解码依赖、辅助进程或点击跳转。

### 领域模型与转移

| 字段 | 契约                                                                                                                                                                                                                           |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题 | 用户尚未看见后台终端 Agent 转移时，Client 需要发一次延迟本地弹窗，并在启用时响一次，且不改变 Agent 或 Workbench 状态。                                                                                                         |
| 输入 | 完整物理轮次、完整启动绑定、语义序号、已检测 Agent 标签、`idle`／`working`／`blocked`／`done`／`error`／`unknown`、精确 Client seen 确认、单调时间、已接受配置修订及 sink 可用性。轮次缺失或不可读表示没有证据，不能推断转移。 |
| 数据 | Client 为每个 presentation 最多拥有一个当前观察和一个候选。候选身份由完整 presentation／generation／Host epoch／Host terminal 绑定以及 sequence、kind、Agent 标签、到期时间、首次可用的成功绘制序号和配置修订组成。            |
| 输出 | `done` 或 `request` 投递命令，或显式 pending／cancelled／suppressed／no-op 状态。命令分别说明是否启用弹窗和内置声音。sink 接纳后的失败不致命，也不重试。                                                                       |
| 依赖 | 纯 reducer 接收逻辑时间和成功绘制事实。attention owner 提供精确观察及 seen 事件。通知状态提供有界 sink 准入。只有 TUI 生命周期 owner 可以写 OSC、提交系统气泡、绘制卡片或调用声音 API。                                        |
| 归属 | Server 继续拥有物理报告和配置发布。各 Client 拥有自身观察确认及 scheduler。自动事件不使用手动 Server 通知注册表。                                                                                                              |

| 前一个精确状态                      | 当前精确状态     | 结果                             |
| ----------------------------------- | ---------------- | -------------------------------- |
| 首次观察、替换绑定或 Agent 标签变化 | 任意             | 安装基线；不建候选               |
| `working`                           | `idle` 或 `done` | 调度 `done`                      |
| `idle` 或 `working`                 | `blocked`        | 调度 `request`                   |
| `unknown`                           | 任意             | 不建候选                         |
| 其他转移或重复 sequence             | 任意             | 不建新候选；状态变化会取消旧候选 |

既有 `working` → `idle` 通知仍表示进入安静状态；其 `done` 投递类型不证明回合成功，也不改变 Agent 状态。显式 `done` 报告可使用同一投递类型。错误报告不引入新的通知策略。

候选只有在延迟结束且后续至少有一次完整绘制成功后才可投递，因此零延迟也给可见 pane
一次确认机会。投递时 reducer 重新核对完整绑定、sequence、状态、Agent 标签、配置修订及
未读状态。精确绘制成功后，该 Client 同时抑制弹窗与声音；其他 Client 不受影响。
这项仅通知使用的抑制只针对精确版本；外层焦点明确丢失时，它不会把 attention badge 标为
seen。移除、替换、状态变化、确认已读、有效配置替换或 Client 退出都会取消候选。手动卡片、
对话框、提示或其他用户所有 surface 会使 sink 繁忙；有界自动候选等待，绝不替换该
surface。一个已接纳候选最多产生一次效果，Client 退出后不持久化。

Session 将键位、侧栏设置及完整通知 profile 原子发布。启动或 reload 来源不可读、过大、
非 UTF-8 或 TOML 畸形时保留旧 profile。无效通知叶字段使用安全默认值、产生警告并参与
完整候选的原子发布；畸形私有线协议 profile 仍在 Client 安装前拒绝。有效且不同的修订会
取消全部旧自动候选及尚未绘制的通知效果；后续状态转移使用新 profile。自动弹窗默认使用
应用内投递，自动声音默认关闭。运行时同时关闭两者再 reload 即可立即回滚。回滚旧二进制
前还必须移除 `agent_delivery`、`delay_seconds` 和 `[ui.sound]`。

### 正确性声明与证据

所属实现测试覆盖基线/替换静默、准确 Client/身份/序号/类型去重、拒绝过期候选、Client 局部确认、静音策略、无效重载保留、有效重载取消和退役后无效果。

在 `src/terminal_session/agent_notifications/tests.rs`、`src/terminal_session/agent_notifications/tests/generated.rs`、`src/notifications/tests.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。
