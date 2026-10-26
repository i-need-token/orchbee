# TUI 输入系统与交互词汇扩展

| 字段     | 值                                                                                                                                                                                                                                        |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 提请产品决策人验收的草案。本文尚无任何已接受内容；每个新键位、mode、手势规则与 wire 请求都标注为 provisional，只有经过与父 contract 相同的验收路径才生效                                                                                  |
| 负责人   | 产品命令体验，以及 Rust `orchbee` Client TUI 输入路径、Server 语义命令边界与 terminal protocol 的维护者                                                                                                                                   |
| 父设计   | [`native-command-surface.md`](native-command-surface.md) 拥有已接受的 TUI Client contract 与 `Ctrl+B` initial map；[`native-runtime-topology.md`](../platform/native-runtime-topology.md) 拥有共享 Workbench document 与 Client View 边界 |
| 决策权威 | 产品决策人决定二级输入结构、各 provisional 键位表、每项 wire 准入请求以及 mouse-capture 策略。Astra 的调研结论已把 floating panes、插件系统、可配置键位文件、multiplexer session 语义与客户端 tiling 排除在范围外                         |
| 退出条件 | 本词汇的一个已接受切片落入所属 app 文档、输入实现与测试；届时本提案退役或缩减为尚未接受的剩余部分                                                                                                                                         |

英文规范原文：
[tui-input-vocabulary.md](tui-input-vocabulary.md)。
如有冲突，以英文为准。

Windows 键盘调整已遵循接受的[原生键盘契约](../../apps/orchbee/docs/native-keyboard-resize.zh-CN.md)。下文更完整的 PANEL 词表仍是提案；其设计清单不替代当前 app 权威。

## 目的

当前 TUI 输入系统是在原始 byte stream 之上的单层 `Ctrl+B` 前缀。它今天可用，但已准入的
Workbench surface 持续增长：`panel.split`、`panel.close` 与 `panel.move` 已是 accepted 的
Server mutation 却没有快捷键，scrollback、navigator 深化与 session/attachment 管理紧随其后。
本提案盘点现有输入面，论证单层扁平前缀无法承载下一代动作，比较四种二级结构，并给出一份
扩展词汇与 mouse→semantic intent 映射草案供验收。

## 当前输入盘点

原生 Client 把继承的 TTY 读作原始 byte stream。`InputDispatcher` 逐字节依次经过：独占式
modal editor decoder、close-confirmation capture、`ClientView` 前缀状态机；未被任何一方认领
的字节按原顺序转发给 terminal。`ClientView` 是纯决策函数：`decide` 把一个字节映射为至多一
个 `ClientAction`，`commit` 应用该 transition。`ClientAction::backend_action` 把需要 session
port 的动作与纯本地动作分开。

已实现的前缀表绑定 33 个字节值。[`apps/orchbee` 指南](../../apps/orchbee/README.zh-CN.md#原生-tab-交互)
中面向产品记录的动作是 17 个；完整实现表还包含 accepted initial contract 里的 sidebar toggle、
shortcut help、literal-prefix 转发与前缀取消：

| 键                    | 行为                                                          | 语义               |
| --------------------- | ------------------------------------------------------------- | ------------------ |
| `c`                   | 在当前 context 中 `tab.create`                                | Workbench mutation |
| `n` / `p`             | 选择下一个 / 上一个 Tab，切换 display attachment              | Client-local       |
| `1` … `9`             | 选择指定序位的 Tab，切换 display attachment                   | Client-local       |
| `[` / `]`             | 选择上一个 / 下一个 standalone 或 Workspace context           | Client-local       |
| `a`                   | 通过分阶段 setup wizard 执行 `workspace.create`               | Workbench mutation |
| `e`                   | 对 Empty Workspace 执行 `workspace.primary-directory.accept`  | Workbench mutation |
| `r` / `R`             | 通过 rename editor 执行 `workspace.rename` / `project.rename` | Workbench mutation |
| `<` / `>`             | `tab.move` 前移 / 后移                                        | Workbench mutation |
| `,`                   | 通过 rename editor 执行 `tab.rename`                          | Workbench mutation |
| `x`                   | 经绘制确认的 `tab.close`                                      | Workbench mutation |
| `w`                   | 切换 navigator overlay；refresh 执行一次 Server snapshot read | Client-local       |
| `h` / `j` / `k` / `l` | 在 split tree 上移动本地 focus                                | Client-local       |
| `q` / `d`             | 关闭本 Client Session；retained 执行继续                      | Client-local       |
| `b`                   | 切换 business sidebar（按 viewport 决定 docked 或 overlay）   | Client-local       |
| `?`                   | 打开 shortcut-help overlay                                    | Client-local       |
| `Ctrl+B`              | 向 terminal 转发一个字面 `0x02` 字节                          | Client-local       |
| `Escape`              | 取消待决前缀                                                  | Client-local       |

前缀表之外还有四个输入面：

- modal 行编辑器在 rename draft（512 个 Unicode scalar）或 workspace-setup 阶段
  （project 选择、label、4096 字节 directory path）打开期间独占键盘。decoder 约束
  UTF-8 碎片、CSI/SS3 序列与 bracketed paste；Enter 提交，Escape 取消，Backspace
  删除一个 scalar，Ctrl+U 清空草稿。
- close confirmation 捕获完整的 input packet，使尾随字节不会泄漏进 shell；只有确切的
  confirmation key 起作用。
- navigator、shortcut-help 与 sidebar overlay 不捕获输入。它们打开期间普通字节继续流向
  terminal，只有前缀层仍解释按键。
- terminal mode（raw mode、alternate screen、bracketed paste `?2004`）由 session lifecycle
  通过带 restoration step 的私有 `crossterm::Command` 序列拥有，不经 crossterm event
  stream。

## 容量论证

扁平前缀层已占用 33 个位置，其可助记空间接近耗尽：

| 键位空间 | 已绑定                              | 空闲                             |
| -------- | ----------------------------------- | -------------------------------- |
| 小写字母 | 15：`a b c d e h j k l n p q r w x` | 11：`f g i m o s t u v y z`      |
| 大写字母 | 1：`R`                              | 25                               |
| 数字     | 9：`1`–`9`                          | 1：`0`                           |
| 标点     | 6：`[` `]` `<` `>` `,` `?`          | 约 26 个可印刷位置               |
| 控制字节 | 2：`Ctrl+B`、`Escape`               | 实际上没有可安全用于快捷键的字节 |

仅已准入的需求就超过剩余可记忆容量：

- `panel.split` 需要四个方向变体；`panel.close` 与 `panel.move` 需要键位。
- scrollback 进入/退出、一个 search editor 与 selection export 构成第二个域。
- navigator 深化在捕获输入后需要移动、激活与过滤。
- session 与 attachment 管理（status surface、control-grant 请求）需要键位。
- `panel.resize` 若在 wire 上准入，需要步进 grow/shrink 绑定。

十五个以上新绑定对上十一个空闲字母，只能落入没有助记价值的标点，而接近五十项的扁平表
超出可靠记忆。词汇需要第二级；问题是它取什么形态。

## 二级结构选型

无论由哪种结构分发，每个键都恰好解析为三类意图之一：作用于共享文档的
`workbench.command` mutation、attachment 级 terminal-protocol 效果（如 `MouseInput`、
`SetHistoryDemand`、`SelectionGesture`）、或纯 Client-local 的 view/session 转换。因此第二
级只是 Client-local 输入问题——wire 与 Server contract 永远看不到它。

- **(a) `Ctrl+B` 下的二级前缀。** 一级键武装一个具名 sub-prefix，第二个键
  触发动作。每个序列保持两次按键且不引入可见状态，但每个 sub-prefix 又是个要背的魔法字
  母，悬空的 armed prefix 需要自己的 timeout/dismiss 规则，而且不打开 help 就看不到分组。
- **(b) 显式 mode 族。** 前缀键进入带可见指示的具象名 mode；mode 内整个键盘
  重绑定到该域，直到 Escape。mode 按域复用同一批字母，廉价吸收大型低频域，让重复操作高
  效。代价是 modal 风险：忘记所处 mode 的用户会迷失，除非指示恒可见且 Escape 总能退出。
- **(c) 命令面板 / `:` 命令行。** 一个绑定打开行编辑器，按 CLI noun/verb 语法输入命令，
  例如 `panel split right`。可发现、动词不受限，且是 resize 数值这类带参动作唯一的自然
  归宿——但对空间化高频操作太慢。
- **(d) 混合。** 扁平前缀保留高频一次性动作；一小组保留的具名 mode 承载低频域；`:` 承载
  参数化与罕见命令，同时充当前两者的发现路径。

推荐：**(d) 混合。** 分层按动作频率划分，这正是现有前缀已经擅长的事；mode 给 panel 管理
与 scrollback 这类域一整块第二键盘而不消耗扁平命名空间；`:` 复用已接受的 CLI 语法，使
TUI 已承诺的命令语言保持一致。纯 (b) 会把 `c`、`n` 也关进 mode 墙后；纯 (c) 让 split 与
focus 操作受打字速度约束。(a) 最弱：它增加不可见状态和二阶助记符，却没有在真正承压的地
方增加容量。

mode 机制沿用现有独占输入模式：mode 是与 modal editor、confirmation capture 并列的又一
种输入拥有者，其进入与退出都是 Client-local view transition。任何拥有者持有键盘期间没有
字节透传到 terminal；常驻指示器标出当前 mode；Escape 永远返回普通层；每个键仍按每个输
入事件至多一个 typed intent 解析。

## 建议的词汇扩展

以下绑定全部为 **provisional**——供验收的草案，不是已承诺行为。约定：新大写绑定打开具名
mode；新小写与标点绑定保持一次性动作。`R` 先于该约定存在，保持原样。

`Ctrl+B` 之后的一级新增：

| 绑定 | 动作                                                   | 语义               | 状态        |
| ---- | ------------------------------------------------------ | ------------------ | ----------- |
| `-`  | `panel.split`，新 panel 放在下方                       | Workbench mutation | Provisional |
| `_`  | `panel.split`，新 panel 放在上方                       | Workbench mutation | Provisional |
| `\`  | `panel.split`，新 panel 放在左侧                       | Workbench mutation | Provisional |
| `\|` | `panel.split`，新 panel 放在右侧                       | Workbench mutation | Provisional |
| `X`  | 对聚焦 leaf 执行 `panel.close`，沿用同样的绘制确认风格 | Workbench mutation | Provisional |
| `z`  | 切换聚焦 panel 的 Client-local zoom                    | Client-local       | Provisional |
| `P`  | 进入 `PANEL` mode                                      | Client-local       | Provisional |
| `U`  | 进入 `SCROLLBACK` mode                                 | Client-local       | Provisional |
| `:`  | 打开命令行                                             | Client-local       | Provisional |
| `i`  | 打开 session/attachment status overlay                 | Client-local       | Provisional |
| `t`  | Client 处于 view-only 时请求 controller grant          | Client-local       | Provisional |

split 字形绑定读作它们画出的线：`-` 分成行、`|` 分成列；未加 shift 的字形把新 panel 放在
自然端（下、右），加 shift 的变体做镜像（上、左）。`X` 把 `x` 的目标从 Tab 收窄到聚焦的
Panel。`u` 系字母保持空闲，所以 `U` 可读作向上进入 history。

`PANEL` mode 草案映射（mode-local 命名空间；Escape 退出）：

| 键              | 动作                             | 语义               | 状态              |
| --------------- | -------------------------------- | ------------------ | ----------------- |
| `h`/`j`/`k`/`l` | 把聚焦 panel 向该侧 `panel.move` | Workbench mutation | Provisional       |
| `x`             | 对聚焦 leaf 执行 `panel.close`   | Workbench mutation | Provisional       |
| `z`             | 切换 Client-local zoom           | Client-local       | Provisional       |
| `+` / `-`       | 把聚焦 panel 步进放大 / 缩小     | Workbench mutation | Blocked: wire gap |
| `=`             | 把聚焦 split 重置为等分          | Workbench mutation | Blocked: wire gap |
| `?`             | 当前 mode 的 shortcut help       | Client-local       | Provisional       |
| `Escape`        | 退出该 mode                      | Client-local       | Provisional       |

`SCROLLBACK` mode 草案映射。进入时在 attachment 上声明 `ViewingHistory` demand；退出时
回到 `FollowingBottom`。全部查看键都是对既有 wire read 的 Client-local 驱动：

| 键                     | 动作                                        | 语义              | 状态        |
| ---------------------- | ------------------------------------------- | ----------------- | ----------- |
| `j`/`k` 或 `Down`/`Up` | 滚动一行                                    | Attachment-scoped | Provisional |
| `d`/`u` 或翻页键       | 滚动一个 viewport                           | Attachment-scoped | Provisional |
| `g` / `G`              | 跳到最旧 / 最新                             | Attachment-scoped | Provisional |
| `/`                    | 基于 `Search` row read 打开 search 行编辑器 | Client-local      | Provisional |
| `n` / `N`              | 下一个 / 上一个 search 命中                 | Client-local      | Provisional |
| `y`                    | 导出当前 selection                          | Attachment-scoped | Provisional |
| `q` / `Escape`         | 退出该 mode                                 | Client-local      | Provisional |

`:` 命令行复用 modal 行编辑器机制与 CLI noun/verb 语法：`panel split right`、
`panel resize right 10`、`tab rename`、`help panel`。Enter 提交至多一个 typed intent；
Escape 取消。它是所有带参动作的规定归宿，包括 wire 准入后的 resize。

navigator 深化是单独的决策点，因为该 overlay 目前从不捕获输入。若作为捕获面被准入，其
草案映射为：`j`/`k` 或方向键移动，`h`/`l` 切换 context 分组，`Enter` 激活，`/` 过滤，
`x` 对标记项请求 Close，`Escape` 关闭——全部 Client-local，激活解析为与一级选择键相同
的 selection intent。

帮助发现保持在顶层 `?`，在 mode 内变为上下文相关，并可经 `:` 以 `help` 到达。

## mouse→semantic intent 映射

对已锁定 `crossterm 0.29.0` 依赖核实的能力结论：

- `EnableMouseCapture`/`DisableMouseCapture`、`EnableFocusChange`、`MouseEvent` 与
  `event::read`/`poll` stream 都存在，但被 `events` 与 `bracketed-paste` cargo feature
  门控，而 `apps/orchbee` 通过 `default-features = false` 关掉了它们。启用会把 `mio`
  与 `signal-hook-mio` 拉进依赖树。
- Orchbee 既有模式已是在 terminal-mode lifecycle 下直接发送私有 DECSET 序列
  （bracketed paste 的 `?2004`）并带 restoration step。mouse capture 可用同一模式发送
  `?1002` button-event tracking 加 `?1006` SGR 编码，不加依赖、不迁移到 event stream。
- SGR mouse 报告随后以 `ESC [ < b ; x ; y M/m` 进入同一条原始 byte stream，输入层可在
  既有 CSI 处理旁解码。点击、拖拽与滚轮不需要 `?1003` any-motion tracking。

手势映射草案。命中区域来自 Client-local layout projection：

| 手势                   | 命中面         | 语义意图                                                          | 语义               | 状态              |
| ---------------------- | -------------- | ----------------------------------------------------------------- | ------------------ | ----------------- |
| 左键点击               | Tab strip 条目 | 选择该 Tab                                                        | Client-local       | Provisional       |
| 左键点击               | Panel viewport | 聚焦该 panel；仅当内部 app 上报 mouse 时转发为 `MouseInput`       | Client-local       | Provisional       |
| 左键拖拽               | split 边界     | 释放时产生一个 `panel.resize` intent                              | Workbench mutation | Blocked: wire gap |
| 左键点击 / 激活        | navigator 条目 | 解析为与选择键相同的 selection intent                             | Client-local       | Provisional       |
| 滚轮上 / 下            | Panel viewport | 进入或在 `SCROLLBACK` demand 内移动；在底部回到 `FollowingBottom` | Attachment-scoped  | Provisional       |
| 左键拖拽               | Panel 文本     | `SelectionGesture` start/update，然后 `SelectionExport`           | Attachment-scoped  | Provisional       |
| 修饰键拖拽（如 Shift） | 任意处         | 交给外层 terminal 自己的 bypass 做原生选择                        | Client-local       | Provisional       |

selection 与 mouse-reporting passthrough 之间是唯一真正的策略决策。当 terminal 内部的
app 开启自己的 mouse reporting 时，其 viewport 内的拖拽属于该 app；没有开启时，同一手势
是 Orchbee 自己的文本选择。Client 今天无法做此选择，因为内部 terminal 的 mouse mode 没
有投影给它。建议：把 terminal 的 mouse-reporting 标志投影为 Client 可见的 terminal fact
（见下述 wire gap）；在此之前保守的过渡规则是点击仅聚焦、拖拽执行 Orchbee 文本选择、不
转发任何 `MouseInput`。这使每个手势保持确定，并把 app mouse 支持留给后续准入切片。

## wire 缺口与决策请求

既有 wire 已覆盖 `MouseInput`、`FocusInput`、`SetHistoryDemand`、`ReadRows`（`Buffer`、
`Search`、`Viewport`）、`SelectionGesture`/`SelectionCommand`/`SelectionChanged` 与
`SelectionExport`。缺口：

| 缺口                                 | 用途                            | 请求的决策                                                             |
| ------------------------------------ | ------------------------------- | ---------------------------------------------------------------------- |
| `panel.resize` `WorkbenchAction`     | 边界拖拽与键盘步进 resize       | 准入一个 resize mutation——形态选项：direction 加 delta，或新的 slot 比 |
| 内部 terminal mouse-mode 投影        | selection 与 passthrough 的路由 | 把 terminal 的 mouse-reporting 标志暴露为 Client 可见的 terminal fact  |
| exported selection 的 clipboard 交接 | `y` 与拖拽选择的投递            | 确认以 OSC52 或平台 clipboard 作为 Client-local 投递路径               |

## 非目标

- floating panes、插件系统、可配置键位文件、multiplexer session 语义与客户端 tiling 已由既定
  调研结论排除。
- 本提案不改任何代码；实现按验收后的切片各自落地。
- 除明确列出的 wire 缺口请求外，本文不设计任何新的 Server 语义。
- 不采用 crossterm event stream，不加新依赖；raw-byte 输入路径与私有 mode 序列保留。
- 不设第二前缀字节或公开可配置 chord；`Ctrl+B` 仍是唯一 chord。

## 对已验收契约的影响

`native-command-surface.md` 中的 `Ctrl+B` initial map 是已验收的 _initial_ contract。本
提案做增量扩展：所有既有绑定保持行为，下列不变量被保留或一般化而非削弱：

- 一个 chord、一层待决：`Ctrl+B` 仍是唯一顶层入口；`Ctrl+B` `Ctrl+B` 仍发送字面
  `0x02`；Escape 仍取消待决前缀。
- 有序透传：未被任何输入面认领的字节按序、原样到达 terminal。
- 每个输入事件至多一个 typed intent，现扩展至 mode 键、mode-local 键、`:` 行提交与
  mouse 手势。
- 带可见指示的独占输入所有权：modal editor、confirmation capture、具名 mode、命令行与
  任何可捕获 overlay 属于同一类拥有者；Escape 永远返回普通层。
- 未知输入保持惰性：未绑定键只产生 notice，无副作用。
- mode、overlay 与 palette 都是 Client-local 输入机制；它们绝不出现在 wire 上，每个
  Server-semantic 结果仍按 at-most-once mutation 规则解析为恰好一个携带新 request
  identity 的 `workbench.command`。

本提案不改变 `workbench-client-runtime.md` 的 runtime 输入边界、确认绘制要求或任何 wire
schema。每个已接受切片在同一变更中更新所属 app 文档。

## 决策请求

- **D1 — 结构：** 接受混合方案 (d)，或改向其他二级形态。
- **D2 — 键位表：** 对上表每个 provisional 绑定逐项接受、调整或删除。
- **D3 — `panel.resize` wire：** 准入并选择其形态；否则边界拖拽与 resize 键保持不绑定。
- **D4 — mouse-mode 投影：** 暴露内部 terminal 的 mouse 标志，或保留过渡的仅聚焦规则。
- **D5 — mouse capture：** 在 session 开始启用 `?1002`+`?1006` 并接入 lifecycle restoration。
- **D6 — navigator 捕获：** 保持不捕获，或准入捕获版草案映射。
- **D7 — zoom：** 准入 Client-local zoom toggle，或放弃 `z`。
