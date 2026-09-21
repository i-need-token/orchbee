# Agent 看板

## 范围与归属

Agent 看板是 [native Tab Session](native-tab-session.md) 中一个捕获输入的覆盖层，用于列出
Server 持有的 Agent 会话并跳转到 Agent 已存在的 Panel。Server 拥有一切 Agent 事实——会话
身份、provider、状态、规范新鲜度、注意状态与 presentation 绑定。看板仅是 Client 投影与
导航面：它绝不从终端像素推断 Agent 状态，绝不改写 Agent 状态，也绝不创建 Panel。本参考文
档随私有 `terminal_session/agent_board/` 模块、其进入 `client_view`、`ui`、`tui_render`、
`tui_pointer` 与 dispatcher 的窄访问点、OBAP 读取通道及它们的正确性证据一同维护。

`Ctrl+B g` 切换看板。看板打开时在 modal 与确认覆盖层及已触发的 prefix 之后接管输入：
Up/Down 与 `j`/`k` 按不透明 Agent 会话 id 选择行，`Enter` 请求跳转，`r` 请求刷新，
`Esc` 或普通背景按下关闭，点击选中、内圈滚轮滚动行。外圈滚轮、未知按键与粘贴均被吞掉；
任何字节都不会到达终端 shell。modal 或确认持有输入时拒绝打开；prefix 和弦仍然生效，
因此 `Ctrl+B g` 同样会关闭看板。名称、通知、任务发送、调度、持久化与新 Panel 创建均不在
范围内。

## 状态与迁移契约

看板在每个打开的 epoch 内运行一条通道线程：一个活跃操作加一个合并的待决意图。跳转请求
会替换待决的刷新，并通过代际推进取消进行中的跳转；在跳转待决期间的刷新请求被吞掉而不
会取代它。每一次完成的 List 尝试——成功、不完整或失败——都会启动两秒冷却，`r` 无法绕
过。

| 迁移       | 守卫与结果                                                                                                       |
| ---------- | ---------------------------------------------------------------------------------------------------------------- |
| 打开       | 无 modal/确认占用时看板接纳新 epoch 与一条通道线程；上一条通道必须已经实际退出。                                    |
| List 轮次  | 一次有界遍历：在单一绝对期限下至多六个同 revision 页、stale cursor 时一次重走、十二次交换共 12 MiB。                |
| 读取落定   | 同 revision 的完整遍历为 complete 并成为 `last_good`；预算耗尽或期限到达为 incomplete 或 failed——绝不伪装成空成功。 |
| 请求跳转   | 按不透明会话 id 选择；通道 `Get` 在当前有界 List 之后重新校验目标，具有独立期限与 128 KiB 上限。                    |
| 逐级解析   | 每一级都按当时的规范身份重新解析 context、Tab、Panel；同 context 已附着的 viewer 本地聚焦，否则走 attach/switch。   |
| 落定       | 安装精确请求的 Panel，包括非零下标；目标已退役或缺失时报 `TargetGone`，绝不回退。                                   |
| 关闭       | 作废代际、取消意图与进行中工作，随后由所有者回收线程、wake 句柄、socket 与意图；实际退出后才可重开。                |

不同 revision 的行绝不拼接。不完整读取保留标记为 stale 的 `last_good`；没有 `last_good` 时
只可展示同 revision 的行并标记 incomplete。侧栏只展示最后一次已完成读取的计数、时间戳与
完整性标记——关闭或未读绝不意味着存活或为零。Get 新鲜度与 List 新鲜度是独立域，绝不互相
替代。

## 正确性证据

`AgentBoard` 覆盖两个 Agent 会话、分布在两个 context 与 Tab 上的三个 presentation、一个
刻意的非零 Panel 目标、带一次重走的有界分页、revision 推进、一次退役、按不透明 id 的选择、
合并意图、stale Get 与 stale stage 丢弃、精确逐级落定、输入吞掉与通道回收。TLC 在无状态约
束、无对称归约下检查完整状态空间——1,083,010 个 distinct 状态——全部必需动作见证齐备，
并为读取轮次、逐级跳转与通道退出提供弱公平进展。

使用 `node scripts/tla/run-models.mjs --models agent-board` 运行已注册模型。OBAP 帧与身
份契约精化线上步骤；`NativeContextSelection`、`NativeTabSwitch` 与 local-client 模型精化
context 切换、attach 与传输接入——它们不覆盖看板本身。

确定性测试覆盖 cursor 与同 revision 行装配、完整性诚实、错误通道隔离、stale 代际跳转拒
绝、modal 输入不泄漏、非零 Panel 上的精确跨 context 落定、`TargetGone` 不误落、有界线程
关闭，以及两条已接纳通道各自独立退役。看板字节绝不进入 shell。渲染测试覆盖两个语言环
境、终端文本消毒与有界视口。
