# 原生单行编辑器

## 范围与归属

Workbench Client 的重命名、Workspace 初始化与设置草稿共享同一个纯单行编辑器核心，位于
[`terminal_text/line_editor.rs`](../src/terminal_text/line_editor.rs)。
它覆盖键盘规范分配给草稿字段的编辑词汇：插入（键入输入与原子粘贴共用同一条路径）、
Backspace、Delete、Left、Right、Home/Ctrl+A、End/Ctrl+E、Ctrl+W 与 Ctrl+U。选择区、撤销历史、
Ctrl+K 以及 Up/Down 重映射不属于此核心；消费者对未绑定按键保持忽略而非转发。

编辑器是纯状态——没有 I/O、时钟、Client 变更、终端读取或 ANSI 解析器。字符准入留在 typed-input
边界与各字段策略；核心从不过滤、规范化或改写所交付的文本。预填按字节精确保留协议合法的
文本，包括 chrome 投影无法显示的标量，使草稿原值与取消路径保持忠实。渲染仍是消费者投影：
`terminal_text` 拥有共享的净化文本与光标映射，重命名/Workspace 字段与设置各自拥有显示窗口。
编辑器核心不拥有显示布局。

## 状态与编辑契约

`LineEditor` 持有 `text`（精确 UTF-8）、`cursor`（始终是扩展 grapheme 边界的字节索引，
可为 `text.len()`），以及一个 `LengthLimit`。限制对 workbench 标签是 `Scalars`（512 个标量），
对文件系统路径是 `Bytes`（4096 字节）。设置使用[各字段的字节上限](../src/native_settings/rows.rs)，
最大为 32 KiB。每项限制都按结果的精确文本校验——绝不按 grapheme 簇计数。
超限的插入或粘贴返回 `LengthLimitReached`，text 与 cursor 均不变；所属草稿把该拒绝转为
可见的输入故障。

grapheme 移动与删除使用已锁定于 1.13.3 的 `unicode-segmentation`；不引入手写 UAX #29 或
第二张宽度表。每次变更对草稿重新分段——受长度限制天然有界——而不是假设
`cursor += inserted.len()` 在簇合并后仍成立。插入意图落在插入段末尾，删除意图落在段首；
当编辑把意图并入新簇内部（ZWJ 黏合邻居、两个区域指示符合成一面旗标），光标向前贴靠到
该簇末尾。`Ctrl+W` 从光标前最后一个 Unicode 词边界起删，保留此前的空白串；`foo.bar` 这类
标点串按规范仍是一个词，不承诺与 readline 行为一致。

## 字段窗口契约

每个草稿字段把完整原始文本一次性送入 `terminal_text` 的 ANSI 感知投影——完整转义序列整段
剥离、禁用标量直接丢弃——并保留同一次解析产生的 raw→visible 字节映射。
`TerminalTextProjection::cursor_boundary` 将映射后的光标贴靠到当前位置或之前的可见字素边界：
剥离控制字符可能合并相邻的簇。不单独解析原始文本的前缀或序列中段切片。

对重命名与 Workspace 初始化，
[`field_window(editor, field_width)`](../src/workbench_client/state/field_window.rs) 在*可见*文本上
按完整字素簇选择窗口。返回的窗口是可绘制文本加严格落在字段宽度内的光标列；零宽
字段返回 `None` 而非伪造位置，宽于剩余空间的簇从不被拆分。

重命名与 Workspace 渲染器都在 modal 的内边距区内绘制该窗口，并把终端光标放在字段行加
光标列处。类型化的 `ModalPresentation` 回执证明完整 surface、字段窗口与精确光标投影均已
绘制。经过评估的几何阻塞会拒绝从未成功绘制的新草稿。草稿一旦成功绘制，相同阻塞会挂起
该 incarnation，并保留其 token、原始文本、grapheme 光标与目标；同一 token 再次完整绘制
并恢复之前，普通编辑和提交均不生效，本地 `Esc` 仍可取消。缺失或被覆盖的回执只撤销当前
绘制证明，既不判定几何，也不丢弃编辑器状态。设置保留 Paragraph 滚动策略并拥有自己的
呈现生命周期，但文本与光标边界仍来自同一个完整文本投影；绘制从不改写原始草稿。

## 正确性证据

确定性单元测试覆盖五标量家庭 emoji、ZWJ 与区域指示符合并、组合符插入与删除、Unicode 词
删除对空白串/标点/emoji/中英混排、标量与字节限制的精确边界、原子拒绝、字节精确预填与
文末光标。投影见证覆盖整段序列剥离且 raw 光标落入被删区间、窄窗口下的光标移动、普通中文、
emoji、组合符与零宽字段。草稿级测试覆盖文末预填编辑、导航、删除、词退格、原子粘贴、选择
器对文本编辑的惰性，以及故障恢复；渲染器与会话旅程见证所绘窗口、所放光标与原文精确提交。
[设置渲染器见证](../src/tui_render/settings/tests.rs) 检查移除控制字符/ANSI 后组合符或 emoji
变体选择符合并成簇、正常宽字形及文末光标。[设置输入轨迹](../src/terminal_input/dispatcher/tests/settings.rs)
让字面粘贴与 Ctrl+W 经过解码器，验证删词且不保存、不向窗格转发。

<!-- documentation-pairing: english-sha256=b246fce1d09dbdba3abb03cda5687736c4e21bb6e6e2bdc243ab9f09f4e690ee zh-cn-sha256=5f2e7918910976423a7986056d83e32c0216cbe440832415723f6e779822bbf1 -->
