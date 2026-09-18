# 原生单行编辑器

## 范围与归属

终端会话的重命名与 Workspace 初始化草稿共享同一个纯单行编辑器核心，位于
[`terminal_session/state/line_editor.rs`](../src/terminal_session/state/line_editor.rs)。
它覆盖键盘规范分配给草稿字段的编辑词汇：插入（键入输入与原子粘贴共用同一条路径）、
Backspace、Delete、Left、Right、Home/Ctrl+A、End/Ctrl+E、Ctrl+W 与 Ctrl+U。选择区、撤销历史、
Ctrl+K 以及 Up/Down 重映射不属于此核心；消费者对未绑定按键保持忽略而非转发。

编辑器是纯状态——没有 I/O、时钟、会话变更、终端读取或 ANSI 解析器。字符准入留在 typed-input
边界与各字段策略；核心从不过滤、规范化或改写所交付的文本。预填按字节精确保留协议合法的
文本，包括 chrome 投影无法显示的标量，使草稿原值与取消路径保持忠实。渲染是由
[`terminal_session/state/field_window.rs`](../src/terminal_session/state/field_window.rs)
持有的纯消费者投影；核心不拥有显示布局。

## 状态与编辑契约

`LineEditor` 持有 `text`（精确 UTF-8）、`cursor`（始终是扩展 grapheme 边界的字节索引，
可为 `text.len()`），以及一个 `LengthLimit`。限制对 workbench 标签是 `Scalars`（512 个标量），
对文件系统路径是 `Bytes`（4096 字节），并按结果的精确文本校验——绝不按 grapheme 簇计数。
超限的插入或粘贴返回 `LengthLimitReached`，text 与 cursor 均不变；所属草稿把该拒绝转为
可见的输入故障。

grapheme 移动与删除使用已锁定于 1.13.3 的 `unicode-segmentation`；不引入手写 UAX #29 或
第二张宽度表。每次变更对草稿重新分段——受长度限制天然有界——而不是假设
`cursor += inserted.len()` 在簇合并后仍成立。插入意图落在插入段末尾，删除意图落在段首；
当编辑把意图并入新簇内部（ZWJ 黏合邻居、两个区域指示符合成一面旗标），光标向前贴靠到
该簇末尾。`Ctrl+W` 从光标前最后一个 Unicode 词边界起删，保留此前的空白串；`foo.bar` 这类
标点串按规范仍是一个词，不承诺与 readline 行为一致。

## 字段窗口契约

草稿字段从不直接对原始文本做 sanitize 或窗口切片。`field_window(editor, field_width)`
把完整原始文本一次性送入 `terminal_text` 的 ANSI 感知投影——完整转义序列整段剥离、禁用
标量直接丢弃——并保留同一次解析产生的 raw→visible 字节边界映射。随后窗口在*可见*文本上
按完整 grapheme 簇选取，使落入被剥离序列内部的 raw 光标贴靠到显示边界，且任何序列中段的
raw 切片都不会再次进入解析器。返回的窗口是可绘制文本加严格落在字段宽度内的光标列；零宽
字段返回 `None` 而非伪造位置，宽于剩余空间的簇从不被拆分。

两个渲染器都在 modal 的内边距区内绘制该窗口，并把终端光标放在字段行加光标列处；类型化的
`ModalPresentation` 回执证明完整文本放得下，无法呈现的字段会取消草稿而非吞掉按键。

## 正确性证据

确定性单元测试覆盖五标量家庭 emoji、ZWJ 与区域指示符合并、组合符插入与删除、Unicode 词
删除对空白串/标点/emoji/中英混排、标量与字节限制的精确边界、原子拒绝、字节精确预填与
文末光标。投影见证覆盖整段序列剥离且 raw 光标落入被删区间、窄窗口下的光标移动、普通中文、
emoji、组合符与零宽字段。草稿级测试覆盖文末预填编辑、导航、删除、词退格、原子粘贴、选择
器对文本编辑的惰性，以及故障恢复；渲染器与会话旅程见证所绘窗口、所放光标与原文精确提交。
