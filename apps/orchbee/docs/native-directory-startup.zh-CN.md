# 原生 Windows 目录启动

## 已接受模型与所有权

本机 Windows 裸启动应让 `cd <project>; orchbee` 在该目录打开第一个终端。只有 canonical Site
snapshot 中没有 Project、Workspace 或 Tab，没有有效保存位置，且没有全部 checkpoint 损坏后
打开空状态的恢复提示时，才准入自动创建。已有 Site 使用[启动选择](native-context-selection.zh-CN.md)；
不同启动目录不增建 Workspace。显式 Context 和 Agent 目标保持原义。
Linux 启动及[交互设置向导](native-workspace-setup.zh-CN.md)属于其他 owner。

`local_startup` 只捕获一次进程 cwd，所有阶段共享原来的 20 秒绝对截止。
确定性启动目录策略消费已校验 Snapshot、可选保存位置、恢复准入及可选绝对 UTF-8 目录。
缺失或无效目录输入在 ProjectCreate 前拒绝，不选择 home。目录标签沿用已有 512 scalar
basename 与本地化根目录回退；Project 沿用本地化 Default 标签。Worker 通过已有 WorkspaceLaunch
契约检查字面目录、选择配置的默认 shell。该策略不创建文件系统目录或 Git worktree。

Server 仍是 canonical 导航与终端的唯一 owner。原生 worker 使用已有 LocalClient 有类型提交，
各发送一次受 revision 守卫的 ProjectCreate 和 WorkspaceLaunch。Project、Workspace、Directory、
context、Tab、Panel 和 presentation ID 来自 Server 回执。已知回执在任何可失败的后续读取前保留。
共享启动报告只拥有不可变调用证据，不替代 canonical 状态或授予输入。
偏好保留已有私有格式与绘制确认。

## 工作流与输出

| 输入或转换                                           | 输出与效果                                                                                                           |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 有保存位置、非空 Site 或损坏 checkpoint 的空状态回退 | 走已有启动路径；不自动创建 Project 或 Workspace。                                                                    |
| 准入的空 Site                                        | 效果前准备确切 cwd 和标签，不发送 standalone Open。                                                                  |
| Project 提交                                         | 使用捕获 revision 和原截止，只发送一个请求。                                                                         |
| 确定拒绝或未交付                                     | 停止，不自动 rebase 或重发。                                                                                         |
| 提交结果不明                                         | 保留请求身份、退役 carrier，不再提交后续 mutation。                                                                  |
| 已知 Project 提交                                    | 先保留身份，再要求同 Site、达到提交 revision 且含该确切 Project 的 snapshot。                                        |
| Workspace 提交                                       | 一次已有 WorkspaceLaunch 原子拥有 Directory、Workspace 与首终端准入。                                                |
| 已知终端 pending、failed、unavailable 或 unknown     | 保留全部结构 ID，拒绝自动附着，显示保留的 Project/Workspace 证据及检查命令。                                         |
| 已知终端 started                                     | 要求同 Site、达到回执 revision，并验证回执中确切目录/context/Tab/Panel/primary presentation、cwd 与 Worker binding。 |
| 创建成员验证成功                                     | 通过已有 Viewer、controller、geometry 和首帧 owner bootstrap 该确切成员。                                            |
| 后续读取、附着、绘制或截止失败                       | 显示已知回执；不撤销 canonical 数据、不创建替代物、不回退到其他目录。                                                |
| 首次绘制成功                                         | 已有启动偏好发布记住新选择，其他存活 Client 保持自身焦点。                                                           |

只有达到已有首帧就绪才算操作完成。取消停止等待，不撤回 Server 已接受的工作。
已知或不明结果在启动失败返回前以两种语言报告。`orchbee project list` 和
`orchbee workspace list` 检查保留的成员。pending 终端自动恢复及 TUI 一步新建/关闭
Workspace 属于后续切片。不新增 wire、checkpoint、settings 或持久偏好版本。

## 正确性证据与边界

`NativeStartupDirectory` 建模两个 Client、准入/不准入启动输入、一个共享 Site、最多一个自动
Project 和 Workspace、一个独立 revision 写入、每阶段一次交付、已知/不明结果、读取失败与首帧发布。
revision 守卫、空状态准入、已知回执保留及仅已知 Started 后附着是安全性声称。
peer 写入也抽象竞争的普通 standalone 工作。截止与取消投影为失败；字节解析、Worker 目录检查、
文件系统推进、完整 Viewer 状态及原有选择模型不在本抽象内。不声称活性或公平性，
不使用 state constraint 或 symmetry reduction 排除可达状态。

运行 `node scripts/tla/run-models.mjs --models native-startup-directory` 检查持续维护的有限模型及
所需动作覆盖。确定性策略/回执测试、真实原生 Worker/mailbox 命令顺序 trace，以及 Windows ConPTY
空 Site cwd/重开/非空 Site trace 细化所属声称。根 `pnpm verify` 负责实现回归检查。
这些同一作者开发证据不代表 Windows Terminal 手工验收、真实 provider 行为，
也不代表其他待补模型配置已完成。
