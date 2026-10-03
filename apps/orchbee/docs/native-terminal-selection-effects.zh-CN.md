# 原生终端选择与效果

## 范围与归属

本参考治理原生 Client 的选择手势、Host 所拥有的选区投影、纯文本导出、剪贴板投递、pane
程序写剪贴板以及响铃投递。它与 `tui_pointer.rs`、`terminal_session/ui.rs`、
`terminal_session/connection/native.rs`、`tui_render.rs`、`tui_backend.rs`、`local_client.rs`，
`apps/orchbee-worker`、`apps/orchbee-server`、`crates/capability-node-protocol` 和
`crates/client-server-protocol` 中的 viewer 中继，以及 `crates/terminal-protocol` 与
`apps/terminal-host` 中的选择／效果契约共同维护。

Terminal Host 是选择端点和导出文本的唯一权威。原生 Client 拥有指针协调、可见投影、导出
组装和最终外部效果。Worker 与 Server 只精确中继记录，不解释选择文本。共享的
[`TerminalSelectionGestureCoordination`](../../../packages/terminal/formal/tla/TerminalSelectionGestureCoordination.tla)
模型拥有手势发布顺序；[`NativeTerminalSelectionEffects`](../formal/tla/NativeTerminalSelectionEffects.tla)
拥有导出完成和外部效果串行化。

选区是易失、按 attachment 隔离的状态。跨 pane 选择、读取剪贴板、持久化屏幕或选区、搜索和
copy mode、新增键盘绑定，以及三击选行不属于本契约。

## 集成边界

原生 Client 通过现有 mailbox 发布选择命令，将 Host 端点投影到实时与保留历史行，并组装完整且已
确认的 Host 导出。选择复制使用有界 FIFO 和一个异步 Windows Unicode 剪贴板 writer，仅在绘制
flush 成功后派发。退出时 join 其独立隐藏窗口和线程。`clipboard-win` 5.4.1 关闭默认功能、仅启用
`std`，负责安全的剪贴板内存分配与转移；其唯一新增依赖是 `error-code` 3.4.0（均为 BSL-1.0）。

pane 程序剪贴板／BEL 接收、显式 OSC 52 投递和可重载的剪贴板策略仍需接入；下面相应要求描述
这些待完成的集成。仍需收敛指针回归、验证原生 cohort，并完成真实 Windows Terminal 验收；模型
检查和中继测试不能单独证明用户效果。

## 指针归属与选择

- 当 pane 当前帧没有报告鼠标跟踪时，其上的无修饰主键手势归 Client 选择所有；跟踪 pane 上的
  同一手势归 pane 程序所有。带 `Shift` 的手势始终属于宿主终端原生选择，绝不启动 Client 工作。
- 按下捕获精确的已绘制 pane 实例、attachment、投影 generation、内容矩形，以及实时或保留历史
  视口。改变焦点的按下只在同一 pane 就绪后释放，绝不追随替换后的 pane。
- 拖动端点是 pane 内容单元格。离开已绘制内容时，端点钳制在该 pane 内；chrome、边框和相邻
  pane 绝不进入选区。
- 单击开始单元格选择，双击开始词选择。只有 Host 接受该精确 `end` 且返回活动选区时，松开才
  结束手势并请求自动复制。
- Host 绝对端点通过当前实时 scrollback 原点或保留行窗口的 `start_row` 投影。输出 revision 保留
  Host pin，并让可见高亮随所在行移动。新的投影 generation、pane 替换、无效历史 anchor、屏幕
  切换或端点被驱逐都会取消本地手势工作；驱逐还会清除高亮并显示本地化通知。

## 手势发布

Client 只允许一个选择命令在途。待处理 `update` 可以被最新端点替换，但 `end` 留在最后一个保留
的 `update` 之后；`cancel` 删除过时的排队移动。每个命令保留其 `start` 捕获的 generation。仅有
revision 的输出不会重标记或取消工作。stale-generation 结果清除 flight 和队列、释放本地捕获、
保留上一个 Host 选区、不执行复制，也不发出功能失败。其他有类型失败保持可见，且绝不自动重放。

## 导出与剪贴板投递

- 只有已接受且活动的 `end` 才开始纯文本选择导出。每次只允许一个导出，并由精确 attachment 与
  export sequence 围栏保护。
- Client 在 start 响应前最多接受一个 chunk，之后要求连续的 chunk sequence、声明的总长度、
  在该精确长度处出现 final chunk、按序 acknowledgement，以及匹配的 completed 结果。既有 Host
  上限保持为每个 chunk 1 MiB、每次导出 16 MiB。
- 只有完整缓冲区通过严格 UTF-8 解码后，才允许进入剪贴板。已取消、外来、畸形、残缺、超限或
  UTF-8 无效的导出不写入任何内容。生命周期取消会删除仍在队列中的完整复制。外部 dispatch
  一旦开始，成功或结果未知都会终结该次尝试，不重放。
- Windows 默认使用 Win32 Unicode 剪贴板。显式 OSC 52 模式改为选择外层终端。一次复制只使用
  一个 sink；OSC 52 没有肯定回执，因此写出后不再回退到 Win32。
- 剪贴板工作在渲染路径之外运行。确定失败和投递结果未知都会产生本地化状态；两者都不阻塞后续
  绘制。

## Pane 程序效果

Host 接受 pane 程序的 OSC 52 写入，拒绝读取和查询，校验 base64 与 UTF-8，排除控制字符，并将
文本限制为 16 KiB。原生 Client 默认允许这些写入，也可以通过剪贴板策略关闭。它绝不逐次弹窗，
并与直接选择复制使用同一个已配置的单一剪贴板 sink。

BEL 与获准的剪贴板写是两个独立的 Host 流。两者各自在精确 viewer attachment 上维护正数、严格
递增的 sequence，因此同一个 sequence 值可以在两个流中各合法出现一次。Worker 把两个流中已接纳
的记录按到达顺序放入同一个有界 FIFO。外来 attachment，或同一流内重复／倒退的 sequence，属于
协议失败。队列或聚合字节饱和时，不会通过驱逐较早记录来接纳新记录；而是只退役目标 viewer，并
释放该 viewer 的全部保留记录，相邻 viewer 不受影响。Poll 最多移除每条已交付记录一次。断连或
外部效果结果未知不会把它重新排队。

## 外部效果 writer

一个由 Client 拥有的效果 writer 串行化选择复制、获准的 pane 程序剪贴板写入和 BEL。只有当前
同步输出帧已经关闭且 flush 成功后，它才 dispatch；终端输出效果在途时不能开始新帧。BEL 发出
一个 `0x07` 字节。OSC 52 发出一个 write-only UTF-8 payload。Win32 投递通过平台适配器写入一个
以空字符终止的 Unicode 值。任何效果字节都不能出现在 DEC 2026 帧内。

在 Windows 上，剪贴板 writer 线程拥有一个专用、隐藏、不激活的 winsafe `WindowMain` 及其
消息泵，不借用通知窗口。该线程创建、使用并销毁其 `HWND`，自有代码不编写 unsafe。
单独的 `WindowMessageOnly` 在这里并不充分，因为 winsafe 既不为它公开安全消息泵，也不会在 drop
时自动销毁。每次 Win32 复制都先用 writer 的非空 owner 通过
[`OpenClipboard`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard)
打开剪贴板，再调用 `EmptyClipboard`；禁止用空 owner 打开后清空，因为这种状态会使随后的
[`SetClipboardData`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setclipboarddata)
失败。适配器将 `CF_UNICODETEXT` 编码成本机小端 UTF-16，并追加一个空 code unit，同时分配可移动
的全局内存。在 `SetClipboardData` 成功前，适配器保有该分配，并在每个错误分支释放它；成功后
所有权转移给 Windows，应用代码不再保留或释放这个 `HGLOBAL`。投递不经过 PowerShell 或可见
窗口 helper。

停机时先关闭效果准入并取消全部尚未 dispatch 的记录。如果 Win32 或终端 dispatch 已经开始，
Client 等待它得到成功、确定失败或结果未知的结算。随后 writer 线程销毁自己的隐藏 owner
窗口并退出，完成 join 后才能结束退役。这让不可取消的 Win32 调用始终处于 writer 生命周期内，
且拆除后不会重放。

## 状态模型

`NativeTerminalSelectionEffects.tla` 组合一次选择导出、一个有界效果队列和一个 writer。其语义
动作覆盖已接受和拒绝的松开、提前到达的导出 chunk、按序和无效 chunk、完成、UTF-8 准入、生命
周期 draining、精确和被拒的 pane 效果、帧打开／flush、dispatch、成功、未知结算，以及在途
dispatch 结算后的退役。

| 声明                                        | 模型属性                                                                         |
| ------------------------------------------- | -------------------------------------------------------------------------------- |
| 残缺或无效导出绝不到达 sink                 | `SelectionEffectRequiresCompleteExport`、`TerminalCopyNeverCommits`              |
| 选择复制最多提交一次                        | `SelectionCopyCommitsAtMostOnce`、`AttemptedEffectsAreUnique`                    |
| 一次复制只使用已配置的 sink                 | `EffectsUseOneSink`、`DeniedProgramWritesHaveNoEffect`                           |
| 效果保持 FIFO 顺序，dispatch 后绝不重新排队 | `QueuedEffectIdsIncrease`、`AttemptOrderIncreases`、`AttemptedEffectsNeverQueue` |
| 渲染与终端输出效果绝不重叠                  | `FrameAndTerminalWriterAreExclusive`                                             |
| draining 会删除未 dispatch 的效果           | `DrainingClientHasNoUndispatchedEffects`                                         |
| 退役会等待已 dispatch 的效果结算            | `RetiredClientHasNoEffects`、`CoverageRetirementWaitsForDispatchedEffect`        |

TLC 检查一次选择复制、最多两个 chunk、最多三个已准入效果、两个记录的队列、Win32 与 OSC 52
sink，以及 pane 程序写入策略的两个取值。字节、Unicode 解码、几何、传输 framing、OS 剪贴板
行为和真实终端渲染仍由确定性适配器与系统证据承担。模型不假设 OS 或外层终端会确认效果，因此
不声明活性；未知 dispatch 是不重放的终结结果。

## 验证映射

| 声明                         | 最低有效证据                               | 集成证据                                           | 边界                            |
| ---------------------------- | ------------------------------------------ | -------------------------------------------------- | ------------------------------- |
| 最终移动先于松开             | 共享手势 TLC 属性和确定性协调器轨迹        | 实时与历史的快速松开轨迹                           | 每次模型运行一个指针手势        |
| 只提交完整的 Host 文本导出   | 选择效果 TLC 属性和 chunk 状态测试         | 软／硬折行、CJK 宽度、组合文本、反向拖动和历史复制 | 纯文本，最大 16 MiB             |
| pane 效果只到达自己的 viewer | Worker 精确 attachment FIFO 与饱和测试     | Host–Worker–Server–Client 记录轨迹                 | 匹配的原生 cohort               |
| 鼠标归属保持互斥             | 指针归属和 generation 围栏测试             | `vim mouse=a`、未跟踪拖动和 `Shift` 原生选择       | 基于 cell 的输入                |
| 外部效果不拆开帧             | 效果 writer 状态测试和 backend 字节轨迹    | Windows Terminal OSC 52 与 BEL 观察                | 信任 Orchbee 之外的宿主终端行为 |
| Win32 复制是 Windows 默认值  | 非空 owner、UTF-16、所有权、争用及拆除测试 | 选择、Codex／Claude 与 Vim 剪贴板观察              | 仅 Windows；OSC 52 为显式模式   |

## 兼容性与恢复

维护中的确定性证据包括 `accepted_release_and_complete_host_export_reach_the_copy_queue_once`：
它运行真实 NativeUi／mailbox 命令与 Host 记录接收边界，不修改共享剪贴板；还有
`reverse_drag_uses_absolute_rows_and_highlights_a_wide_glyph_as_one_unit`。复制队列测试覆盖
dispatch 前取消、dispatch 后退役等待、adapter 消失的确定失败与未知结果不重放。原生隐藏 owner
测试创建并泵送真实 Windows 窗口，注入最终写入，验证 dispatch 和 join；它不证明 OS 剪贴板内容。
真实 Windows Terminal 的剪贴板内容及指针验收仍必需。

选择 viewer 动作和效果路由是由一个精确原生 Client、Server、Worker 与 Host cohort 交付的加法
契约。它们不产生持久选区或屏幕数据迁移。回退时整体替换该 cohort。旧二进制可以继续原有行为，
但不能在混合 cohort 中使用新动作。
