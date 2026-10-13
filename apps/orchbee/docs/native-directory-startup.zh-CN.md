# 原生 Windows 目录启动

## 准入与所有权

本机 Windows 裸启动使 `cd <directory>; orchbee` 在该目录打开第一个终端。自动创建要求全局导航中没有 Workspace、所有终端 context 中都没有 Tab、没有有效保存位置，且没有全部 checkpoint 损坏后的恢复提示。空的组织性 Project 不阻止准入，也不会成为新 Workspace 的归属。已有空 Workspace 仍会阻止自动创建。已有 Session 使用[启动选择](native-context-selection.zh-CN.md)；改变启动目录不增建 Workspace。显式 Context 和 Agent 目标保持原义。Linux 启动及[交互式 Workspace 设置](native-workspace-setup.zh-CN.md)另有 owner。

`local_startup` 只捕获一次 cwd，并共用原始 20 秒绝对截止时间。确定性的启动策略消费已验证快照、可选保存位置、恢复准入和可选绝对 UTF-8 目录。目录输入无效或缺失时，在启动前拒绝，不改选 home。Workspace 名称为有界的 512-scalar basename 或本地化根目录后备名称。Worker 探测字面目录并启动已配置默认 shell。该策略不创建文件系统目录或 Git worktree，也不自动启动 AI 程序。

Server 拥有 canonical 导航和终端准入。启动精确提交一次带修订围栏的 `WorkspaceLaunch`，使用 `WorkspacePlacement::Ungrouped {}`、cwd、默认尺寸，无显式 launch 或 profile。不执行 ProjectCreate，不保留 Project 回执，也不存在 Project 阶段刷新。Workspace、Directory、context、Tab、Panel 和 presentation 标识来自 launch 回执，其 `project_id` 可空。调用证据不替代 canonical 状态，也不授予终端输入权限。

## 流程与失败证据

| 转换                                             | 副作用与保留证据                                            |
| ------------------------------------------------ | ----------------------------------------------------------- |
| 已有 Workspace/Tab、保存位置或损坏恢复后备       | 走既有启动选择，不自动启动 Workspace                        |
| 符合条件的空 Session（允许空 Project）           | 准备 cwd 和名称；跳过 standalone Open                       |
| Launch                                           | 在初始快照修订和原截止时间内只提交一次                      |
| 明确拒绝或未交接                                 | 无已知提交，停止；不 rebase 或重试                          |
| 交接结果未知                                     | 保留原始 launch 请求标识，停止后续请求和重放                |
| 已知终端 pending、failed、unavailable 或 unknown | 保留已知 Workspace 回执，拒绝自动 attach                    |
| 已知终端 started                                 | 读取不早于回执修订的同 Session 快照，验证精确返回作用域     |
| 成员验证通过                                     | 通过既有 Viewer/controller/geometry/首帧 owner 引导精确成员 |
| 后续读取、attach、首帧或截止失败                 | 保留已知回执，不创建替代资源，不回退其他目录                |
| 首次绘制成功                                     | 既有偏好记录创建后的选择；其他 Client 保持各自焦点          |

作用域验证核对 Workspace/context/Tab/Panel/主 presentation 标识、全局 Directory ID 与 cwd，以及 Directory Worker 与实时绑定。拒绝过期或外部 Session 快照、成员缺失和未 started 的启动。未分组启动后新增或改变 Project 不替代 Workspace 身份。初始 launch 回执必须描述请求的未分组 placement。

完成仍要求首帧就绪。取消只停止等待，不停止 Server 已接受工作。两种语言都会在启动返回失败前报告已知 Workspace ID 或未知请求 ID；`orchbee workspace list` 可检查保留状态。没有新的启动专用 wire、checkpoint 或偏好格式，也不自动重放 pending 或未知启动。

## 证据与边界

`src/terminal_session/startup_directory/tests.rs` 覆盖空 Project 准入、已有空 Workspace 拒绝、路径/名称规则、精确作用域及两种语言的保留证据。`src/terminal_session/connection/native/tests/startup_directory.rs` 检查 Bootstrap 前只提交一次 launch、不执行 standalone Open 或 ProjectCreate、读取失败后保留已知回执，以及未知交接后不继续或重放。

使用匹配的原生依赖构建运行所属 `startup_directory` Rust 测试。这些测试覆盖有界实现轨迹；真实终端显示和完整 cohort 验收属于原生端到端测试，不宣称穷尽并发交错或覆盖其他平台。
