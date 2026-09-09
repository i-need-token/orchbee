# 原生 Workspace 设置

## 范围与归属

原生 Client 在已有或新建 Project 中创建 Workspace，并显式配置首个目录。Server 仍是导航和
Directory 的唯一权威；Worker 验证字面路径。本参考文档维护 Client 的交互与命令消费扩展，
依托[上下文选择](native-context-selection.zh-CN.md) 和 Server 的
[目录契约](../../orchbee-server/docs/workspace-terminal-directory.zh-CN.md)。

`Ctrl+B a` 打开 Workspace 创建流程：按 Server 顺序选择已有 Project 或“新建 Project”，
必要时输入并提交 Project 名称，再输入并提交 Workspace 名称。可用时默认选择当前 Workspace
所属 Project，否则选择“新建 Project”。选择器使用 `[` / `]` 和 Enter。每一步完整可见地绘制后，
Enter 才能提交。新 Project 已被接受后，即使取消后续 Workspace 步骤，它仍然保留。

`Ctrl+B e` 仅为上下文为空且没有主 Directory 的当前 Workspace 配置目录。输入为字面的绝对
UTF-8 路径，最多 4,096 字节且不能含 NUL；不回退 home、不做 shell 展开、不创建目录，也没有
目录浏览器。标签保留既有 512 个 Unicode scalar 契约。所有提示、拒绝和已提交结果通知均提供
双语。裸命令启动及既有 `q` / `d` 退出快捷键保留其含义。

成功创建 Workspace 后，验证其确切提交身份，再通过既有上下文清理与安装 owner 选入。
目录接受不启动 shell；显式 `c` 仍是唯一启动意图。该未发布 cohort 整体启用与回滚。回滚或
取消不撤销已经提交的 Project、Workspace 或 Directory 事实。远程访问、重新绑定、删除、
通用 Directory CRUD、持久化与发布打包不在范围内。

## 状态与命令契约

Client 保留完整 Project 身份/标签摘要（包括空 Project），以及活动 Workspace 的目录元数据，
它们与当前上下文来自同一 snapshot revision。选择器的标签和位置不能替代捕获的 Project/Workspace
ID。草稿捕获确切 Site、revision 和目标；并发本地结构操作、Close 和上下文变化不得悄悄
重新定向草稿。Server 仍拒绝旧 revision 和不可用目标。

| 状态或事件             | 结果                                                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------ |
| 打开草稿               | 稳定 Session 准入与新步骤 token；不发送线协议命令。                                        |
| 编辑草稿               | 在同一步骤 token 下编辑有界文本；普通编辑不要求新的首次绘制确认。                          |
| 改变选择器选项         | 分配新 token 并撤销显示授权，直到选中的 Project 目标完整绘制。                             |
| 取消草稿               | 释放 modal 输入所有权；保留 canonical 状态及更早的已知提交。                               |
| 完整绘制               | 仅确认确切当前草稿 token 和完整渲染内容。                                                  |
| 提交                   | 冻结一个有类型命令和一个请求身份；通过既有单 carrier 在原操作预算内排空更早接受的工作。    |
| 确定拒绝               | 保留成员与当前终端，显示失败；不重放命令或自动 rebase。                                    |
| 已知提交               | 在任何可失败的 snapshot 读取或 UI 交接前保留不可变结果。                                   |
| 提交后 snapshot 可用   | 验证确切 Site、revision 和返回身份；推进向导，或通过既有精确 Viewer 清理安装新 Workspace。 |
| 提交后 snapshot 不可用 | 显示并保留已知结果，要求显式 snapshot 恢复；绝不重新提交 mutation。                        |
| 不确定交接             | 以有类型的 outcome unknown 退役 carrier；不重试、不虚构不存在、不运行后续向导命令。        |

原始 UTF-8、括号粘贴、不完整序列和提交后的输入抑制复用同一个有界 modal decoder。
Rename 保留自身有类型的草稿和操作契约。目录字节上限不能用 Unicode scalar 数量近似。
包含 Enter 和后续输入的数据包，不能在下一步完整绘制前提交下一步。尺寸变化、旧 token、
销毁和迟到结果不能为其他草稿授权 mutation。

## 正确性证据

有界设置模型必须覆盖 Project 与 Workspace 步骤、独立目录接受、绘制权限、更早已知提交后的
取消、确定拒绝、结果丢失、提交后 snapshot 失败、显式恢复和至多一次交接。既有 Server 模型
保留 canonical 提交与目录验证义务；原生成员与输入模型保留精确清理、首帧、geometry 和
decoder 义务。

[`NativeWorkspaceSetup`](../formal/tla/NativeWorkspaceSetup.tla) 探索一个 Client 和 Site、
各一个已有及新建 Project/Workspace、三个 mutation 身份、一次 peer revision 写入、
两次 modal 打开，以及由打开、步骤变化和选择器变化共用的六个 token。
这些是有限探索边界，不是产品交互次数限制。其 registry entry 要求覆盖全部 54 个命名动作。
标签和路径投影为有效性及完整性；该模型不证明逐字节解码、完整 Worker 行为或 Viewer 清理细节。

确定性测试覆盖空 Project、重复标签、按 ID 选择、Unicode 标签与字节受限路径、同包提交、
旧绘制、忙时准入、精确命令结果兼容和已知提交保留。Carrier trace 区分命令、snapshot、清理
及 attach 的顺序。Linux 原生进程证据从全新 Site 开始，经可见原生控制创建并配置 Workspace，
显式在所选路径启动终端、返回原 standalone shell，并观察无效目录拒绝且不回退。这些是本地
开发见证，不证明安装后或跨平台就绪。
