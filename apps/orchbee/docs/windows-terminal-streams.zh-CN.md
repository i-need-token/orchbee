# Windows 终端流

## 范围与归属

`terminal session observe <terminal>` 从一个既有 Windows 终端绑定输出 NDJSON。
它不激活 Site、不启动终端、不读 stdin、不取得控制权，也不改变 Host 几何。
`terminal session control <terminal>` 增加独占输入、尺寸和历史命令。
交互 attach 仍是后续依赖能力。两个命令均选择既有终端 ID／选择器。

## 观察模型

| 状态            | 动作及下一状态                                              | 权威与不变量                                                |
| --------------- | ----------------------------------------------------------- | ----------------------------------------------------------- |
| Parsed          | 效果前验证全部参数                                          | CLI 拥有语法；默认视口 120×40                               |
| Admitting       | 发现既有 Site、读取 Snapshot、附着精确绑定                  | 单一五秒预算；不回退目标                                    |
| Observing       | 轮询保留 Viewer、验证帧、输出并刷新记录，再确认             | TerminalProjection 拥有序号／基线／epoch 检查；输出前不 ACK |
| Resynchronizing | 输出 reset、请求快照、恢复观察                              | 不将缺失帧区间伪装成连续流                                  |
| Ending          | 仅尝试一次 detach、释放连接，在输出仍可写时发出一个终止记录 | 两秒清理预留；不停止底层终端                                |
| Finished        | 停止并 join 转发进程、释放句柄及中断归属                    | 不遗留任务、线程、管道或临时文件                            |

默认不限制总时长。`--timeout <ms>` 指定正数总预算，上限 3600000 ms。
单次交换和输出刷新各有五秒上限。中断退出 130；超时 7、不可用 4、传输／协议丢失 5、
本地／输出失败 1。目标缺失、歧义和拒绝保持既有 CLI 拒绝类别。
传输 EOF 不证明终端成功退出。输出失败可能留下最后一行的部分字节；部分输出之后
不再追加第二条记录，也不声称流成功完成。

`--cols` 和 `--rows` 选择观察者视口，不调整终端尺寸。完整 ANSI 帧从验证后的样式单元格
合成，裁剪／补齐到该视口，另行报告源尺寸。这是屏幕投影，不是原始 PTY 字节或图形。
每条 JSON 记录声明 `orchbee.terminal.stream` 版本 2 及 kind。帧字节使用 base64，
sequence／generation／revision 身份使用十进制字符串。两种 format 选择均输出 NDJSON。

Windows stdout 通过本次调用拥有的隐藏系统 PowerShell 进程转发，运行固定脚本并禁用
profile。至多一条有界记录在途，子进程写入并刷新后才确认。父进程使用可取消、有截止
时间的非阻塞管道轮询，不将载荷插入命令。子进程 stdin 使用同步管道句柄；仅把 overlapped IPC 句柄
切换为阻塞模式，无法满足标准子进程读取器的要求。输出停滞或任意退出时，所有者停止并 join
子进程；这无法撤回已经交付下游的字节。

## 控制模型

control 附着精确 Viewer、取得独占控制权（或使用 `--takeover` 转移活动授权），
按 `--cols`／`--rows`（默认 120×40）调整尺寸，再输出包含 `mode: control` 和
`controllerId` 的 `opened`。接管遵循 Server 的
[控制权转移契约](../../orchbee-server/docs/terminal-controller-takeover.zh-CN.md)。
准入或 resize 失败仍释放已拥有的 Viewer；不切换目标，也不重试输入。

stdin 为 `orchbee.terminal.control.input` 定义的 UTF-8 NDJSON。单独的调用级隐藏
PowerShell 字节读取器使用固定脚本、父进程非阻塞管道和有界缓冲。所有出口均终止并等待
读取器和输出转发器。空行忽略；支持首行 BOM、CRLF 和无末尾换行的最后一行。
单行上限 2 MiB；超出帧界限会终止调用。其他非法记录输出类型化拒绝后继续读下一条。
未知／重复字段在命令产生效果前拒绝。

| 输入类型           | 字段与效果                                                                                                                             |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal.input`   | `text` 与 base64 `bytes` 恰选一个，解码后最多 256 KiB；恢复实时 demand，按顺序提交 4096 字节块，不重放                                 |
| `terminal.resize`  | `cols`、`rows`，可选且成对的 `cell_width_px`／`cell_height_px`；Host 确认有效几何后才更新本地视口                                      |
| `terminal.scroll`  | `direction: up/down`、正 u16 `lines`，可选 `source: wheel/page_key`、零基 `column`／`row`（分别默认零）及 crossterm `modifiers` 位标记 |
| `terminal.release` | 结束本次流，detach 精确 Viewer 并释放授权；底层终端继续运行                                                                            |

几何沿用单轴 4096、262144 单元格和 65535 像素乘积上限；像素尺寸须同时为零或同时为正。
文本限制按 UTF-8 字节计。输入 schema 定义形状，解析器额外检查字节／乘积界限。
修饰键标记为 Shift=1、Control=2、Alt=4、Super=8、Hyper=16、Meta=32；只接受已定义位。

串行循环在输入记录之间轮询 Viewer 并推进一次历史步骤。输入回执包含记录序号和已确认
字节前缀；后续块失败不会重放前缀，也不声称剩余字节成功交付。只有对应操作已确认才
发布回执。中断或输出丢失可能使已经产生的输入效果无法回执。

历史复用原生 TUI 的保留视口状态机：设置历史 demand、暂停实时投影、验证精确 Host 行和
事实、刷新活动画面，再发送 RowsApplied。键入先恢复 following-bottom／active demand。
Page 键仅在未启用鼠标跟踪的普通屏幕、且应用光标关闭或 bracketed paste 开启时使用历史，
否则发送 PageUp／PageDown。当前帧尚未匹配已确认尺寸时，scroll 返回 `frame-resizing` 拒绝。
鼠标跟踪每次发送一条带指定修饰键的滚轮报告，与 `lines`
无关。备用屏幕回退发送一条光标箭头；现有帧契约未暴露 DEC 1007，因此关闭 alternate-scroll
的行为仍是待对齐义务。像素鼠标在坐标能力可用前明确拒绝。历史输出为安全文本 ANSI，
不重建源样式或图形。

所有输出记录均使用版本 2。`control` 输出命令回执，`history` 输出保留行画面；普通
`frame`／`reset` 保持原有投影含义。历史活动时，帧确认发生于输出活动历史画面或 pending
reset 之后，不代表隐藏的实时帧已经显示。EOF 和 release 仅在确认 detach 后成功；无法
确认清理时为 outcome-unknown。Ctrl+C 和总超时沿用观察命令退出类别及有界清理预留。

## 交互式 attach

`terminal attach <terminal>` 和 `agent attach <target>` 是 Windows 终端界面。
支持 `--takeover`、`--timeout <ms>` 和 `--mouse on|off`（默认 on）；Agent 选择器还支持
`--session`、`--name` 或 `--panel`。先验证语法，再准入继承的交互 stdin/stdout。
二者均不启动 Site、不改变 Workbench 焦点，也不回退到其他终端。Agent 解析在原始可取消
准入预算内选出唯一活动绑定。即使指定全局 format，输出仍是交互画面。

| 阶段             | 所有者动作与释放规则                                                                      |
| ---------------- | ----------------------------------------------------------------------------------------- |
| TTY admitted     | 读取继承控制台几何，不改变模式                                                            |
| Binding selected | 单次 Snapshot 和精确 Viewer；Agent 解析只读                                               |
| Granted          | 取得或显式接管控制权，确认初始 resize                                                     |
| Mode owned       | 既有 TerminalModeGuard 拥有 raw mode、备用屏幕、光标、paste、focus 及可选 pointer capture |
| Attached         | 共享 control Session 和历史核心；先刷新显示画面，再确认 frame/row                         |
| Ending           | 仅 detach 一次精确 Viewer；错误及展开时也须在诊断前恢复控制台模式                         |

`Ctrl+B` 后按 `q` 脱离，但不停止终端。连续两次 `Ctrl+B` 发送一个字面前缀。
其他后续事件保留暂存前缀及原顺序。粘贴是单次语义操作，绝不参与脱离匹配。
非法、超长或超时粘贴被拒绝，不提交片段；排空到结束标记前始终保有粘贴所有权。
`--mouse off` 下的鼠标报告不会消耗暂存前缀。
raw mode 下的 `Ctrl+C` 交付应用；外部控制台中断取消本次调用。普通 PageUp/PageDown 和
滚轮共享 control 的历史／应用分流；其他鼠标事件使用已验证的窗格跟踪契约。
不支持的输入产生有界本地化提示。尺寸变化使用实际控制台几何；暂时的零几何保留最后
有效尺寸，不发送 resize。旧尺寸帧不能授权新坐标
下的鼠标／历史操作。

进程内显示接口共享 NDJSON 的帧／历史渲染，不先编码再解析该协议。控制台写入沿用既有
继承 Unicode TUI writer；不声称对操作系统控制台无响应提供硬时限。IPC 和清理仍有有界
预算。显式脱离只有在释放已确认、模式已恢复后才成功；传输丢失不伪造终端正常退出。
DEC 1007、像素／图形和扩展键盘模式的残余能力仍须实现并取得 Windows 证据。

## 验证与限制

确定性核心和 Windows 脚本化传输检查覆盖精确绑定、多帧、reset、部分输出、中断、到期
和清理。管道测试须证明下游阻塞不会遗留转发进程。同作者测试不代表独立评审、TLA 执行、
远端／命名 Site 路由、其他平台或图形流验收已完成。control 证据须覆盖
有序输入、部分接受、非法记录后继续、resize、release／EOF、历史反馈顺序和在途控制交换中断。

清理结果在尚未尝试 Viewer attach 时为 `not-attached`，仅收到确认后为 `detached`；
释放连接但无法确认 detach 时为 `session-released`。Windows 打包控制台实测覆盖了包含
脱离前缀的中文 bracketed paste、原始 Ctrl+C 交付、关闭鼠标时的准入、忙碌控制者拒绝、
接管隔离、历史翻页、正常脱离、超时恢复、重新获取控制权以及不同控制台尺寸准入，
也覆盖了独立 TUI 的共享输入。宿主窗口动态缩放、鼠标视觉交互和真实认证 Agent 的
attach 仍是独立验收义务。流超时使用专门诊断；拒绝保留控制者原因，不暗示此前已交付
的输入被撤销。脚本化证据不能替代最终对 Windows 打包运行时的验收。
