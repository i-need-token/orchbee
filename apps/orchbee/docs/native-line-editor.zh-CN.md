# 原生单行编辑器

## 范围与归属

终端会话的重命名与 Workspace 初始化草稿共享同一个纯单行编辑器核心，位于
[`terminal_session/state/line_editor.rs`](../src/terminal_session/state/line_editor.rs)。
它覆盖键盘规范分配给草稿字段的编辑词汇：插入（键入输入与原子粘贴共用同一条路径）、
Backspace、Delete、Left、Right、Home/Ctrl+A、End/Ctrl+E、Ctrl+W 与 Ctrl+U。选择区、撤销历史、
Ctrl+K 以及 Up/Down 重映射不属于此核心；消费者对未绑定按键保持忽略而非转发。

编辑器是纯状态——没有 I/O、时钟、会话变更、终端读取或 ANSI 解析器。字符准入留在 typed-input
边界与各字段策略；核心从不过滤、规范化或改写所交付的文本。预填按字节精确保留协议合法的
文本，包括 chrome 投影无法显示的标量，使草稿原值与取消路径保持忠实。渲染仍是纯投影；
已存文本的 chrome 安全显示是消费者侧的另一个关注点。

## 状态与编辑契约

`LineEditor` 持有 `text`（精确 UTF-8）、`cursor`（始终是扩展 grapheme 边界的字节索引，
可为 `text.len()`）、`scroll`（可见窗口起始的字节索引，始终是 grapheme 边界且不越过
cursor），以及一个 `LengthLimit`。限制对 workbench 标签是 `Scalars`（512 个标量），对文件
系统路径是 `Bytes`（4096 字节），并按结果的精确文本校验——绝不按 grapheme 簇计数。超限的
插入或粘贴返回 `LengthLimitReached`，且 text、cursor、scroll 全部不变。

grapheme 移动与删除使用已锁定于 1.13.3 的 `unicode-segmentation`；不引入手写 UAX #29 或
第二张宽度表。每次变更对草稿重新分段——受长度限制天然有界——而不是假设
`cursor += inserted.len()` 在簇合并后仍成立。插入意图落在插入段末尾，删除意图落在段首；
当编辑把意图并入新簇内部（ZWJ 黏合邻居、两个区域指示符合成一面旗标），光标向前贴靠到
该簇末尾。`Ctrl+W` 从光标前最后一个 Unicode 词边界起删，保留此前的空白串；`foo.bar` 这类
标点串按规范仍是一个词，不承诺与 readline 行为一致。

## 视口契约

显示宽度沿用项目已有的 `Line::width` 度量，组合符占零格、宽字符占两格。
`pin_scroll(field_width)` 沿整簇边界推进 `scroll`，直到光标列——`text[scroll..cursor]` 的
宽度而非整段前缀——窄于字段宽度。`viewport(field_width)` 对零宽字段返回 `None` 而非伪造
光标位置，否则产出可绘制窗口与光标列；宽于剩余空间的簇从不被拆分。该投影仅证明窗口与
光标可绘制——不证明整份草稿都在一屏内，基于它的 draw token 同样只承载这一诚实范围。

## 正确性证据

确定性单元测试覆盖五标量家庭 emoji、ZWJ 与区域指示符合并、组合符插入与删除、Unicode 词
删除对空白串/标点/emoji/中英混排、标量与字节限制的精确边界、原子拒绝、字节精确预填、
文末光标、零宽字段、整簇滚动、不可拆簇，以及反复混合操作保持全部不变式。这些见证只确立
纯模型——接线三个草稿、modal 输入解码与字段渲染是独立的消费者整合，仅凭此核心不声明
编辑器界面已可用。
