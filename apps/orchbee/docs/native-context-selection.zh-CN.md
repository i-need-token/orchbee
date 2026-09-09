# 原生上下文选择

## 范围与归属

原生 Client 在已连接的本机 Site 中选择独立 Terminal Context 或 Workspace。Server 拥有导航、
成员、revision 和终端生命周期；Client 拥有活动位置与选择。本参考文档维护
[原生 Tab Session](native-tab-session.zh-CN.md) 的上下文选择扩展，随 Session reducer、传输、
导航渲染及其正确性证据一起更新。

`Ctrl+B [` 和 `Ctrl+B ]` 按 Server 顺序选择上一个、下一个位置：独立上下文，然后是每个 Project
中的 Workspace。边界循环。标签仅用于展示；Project、Workspace 和 context ID 确立身份。
进入其他位置选择其首个 Tab，没有 Tab 则进入 Empty。不保留各位置上次所选 Tab 的历史。
进入和离开 Empty 都不会创建终端。已有 Tab 控制作用于当前上下文；显式 `c` 使用 Server
已有的目录准入，绝不回退到 home 目录。裸命令启动保留既有 standalone Open 和首帧就绪契约。

Project/Workspace 创建和首目录配置属于 [Workspace 设置](native-workspace-setup.zh-CN.md)，
它复用这里的上下文安装 owner。Panel 布局、远程访问、持久化和生产恢复不在范围内。
该未发布 cohort 整体启用和回滚。改变 Client 位置不会修改 canonical 导航或销毁保留的执行。

## 状态与转换契约

输入为已验证的 Workbench snapshot、显式本地方向、当前 Session 状态、确切 attachment 身份
及既有单调操作预算。导航与当前上下文投影必须来自同一 snapshot revision。非活动上下文的
不支持布局不会使当前上下文无效。选择不支持的目标是确定的本地失败，保留原位置。

| 转换              | 守卫与结果                                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 准入位置选择      | 稳定 Session 没有未解决的 mutation、Close 确认/收敛或成员操作时，保留确切目标身份及操作 generation；否则报告忙。          |
| 读取目标 snapshot | 改变 attachment 前重新验证目标的 canonical owner、Site 和 revision。确定不可用或不支持的目标保留原位置。                  |
| 清理旧 attachment | 隔离旧输入、帧和 grant，然后在安装不同选择前 detach 确切的原 Viewer。不授权终端 Dispose 或 Start。                        |
| 安装目标          | 通过既有成员 owner 和 mailbox 事务原子发布新上下文、成员、provenance 和可空的 desired attachment。Empty 不伪造 Tab 身份。 |
| Attach 所选 Tab   | 使用确切保留 binding 及既有 setup、首帧和 controller geometry 规则。旧 generation 的迟到结果不能成为当前结果。            |
| 交接后失败        | 保留已知事实并采用既有 deadline/retirement 规则；绝不重试不确定的副作用或合成清理成功。                                   |

已知 Create provenance 必须在离开和返回上下文后保留。Snapshot binding 不能把已知 Pending、
failed、unavailable 或 outcome-unknown Create 变为 attach 许可。保留的 provenance 受协议
context 和 Tab 上限约束，只能依据已验证的成员证据修剪。Pending Close 保留原确切目标，
并在既有结算规则允许前阻止位置变化。当前上下文的刷新及 Create/Move/Rename/Close 结果
处理不得悄悄返回 standalone。

## 正确性证据

`NativeContextSelection` 覆盖一个 Site、独立上下文及两个 Workspace、两个保留 binding、
一个空 Workspace、四个操作 epoch、两个导航 revision 和五种已知 Create 结果。38 项必需
动作见证包括双向经过 Empty、边界循环、忙时拒绝、先验证 snapshot 再清理、原子安装、
旧 epoch 和重新进入后的 provenance。TLC 在无 state constraint 或 symmetry 的条件下检查
188,409 个不同状态。Expiry 的弱公平性支持该抽象内最终进入 stable/retired。既有 Switch、
成员移除和 refresh 模型保留详细的传输、首帧、geometry 和 cleanup reserve 义务；新模型
不证明历史修剪或任意运行时的推进。

通过 `node scripts/tla/run-models.mjs --models native-context-selection` 运行已注册模型。
Snapshot 验证细化模型的 read 步骤，成员 owner 和 mailbox 事务细化 cleanup/install，
carrier trace 细化 snapshot/detach/attach 命令。本地位置选择没有 Create、Start、Dispose
或 canonical 导航写入命令。

确定性测试必须覆盖同名不同身份、导航 revision 变化、不支持的非活动成员、目标拒绝、
双向 Empty 转换、provenance 保留、mutation/Close 忙时准入和过期结果。Carrier 测试必须观察
确切 Detach 先于 Attach，且没有导航修改或隐式 Create。Linux 原生进程证据必须执行 Workspace
选择、保留 standalone 重入、已配置的空 Workspace 中的显式创建、其他 Client 的独立选择，
以及 Workspace Close-to-Empty 与确切 shell 退出。渲染测试覆盖双语、终端文本清洗和有界视口。
这些检查证明本地开发行为，不证明打包或跨平台就绪。
