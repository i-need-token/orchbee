# Kitty 回滚缓冲区查看器对齐

## 参考范围

本契约采用 2026-08-27 观察到的 Kitty commit
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 中常用的 `show_scrollback` 工作流。Kitty 在
`kitty/options/definition.py` 中映射 `kitty_mod+h`；`kitty/window.py` 把保留的屏幕与历史准备为
带软换行标记和可见顶部输入行的 ANSI 文本；`kitty/boss.py` 则在 overlay 窗口中启动配置的 pager。

Orchbee 采用一个聚焦终端 Action、`Ctrl+Shift+H` 默认绑定、当前全部主屏幕保留文本的冻结视图、
以终端可见顶部为初始位置、字面搜索、原生文本选择、复制、取消与焦点恢复。它扩展现有应用拥有的
不可变终端文本查看器，使该工作流可用于 Desktop、Web 和后续移动端客户端。对齐声明不包含任意
pager 或编辑器进程、pager 配置与环境、ANSI 样式、外部管道、第二份 pager 历史存储、Terminal Host
保留范围之外的历史、光标位置占位符，以及模态查看器打开时同时向终端输入。

## 变更授权与所有权

这是由产品所有者通过按顺序实现终端对齐功能的请求所接受的建模级兼容性变更。Terminal Host 继续
权威拥有 VT 状态与保留行。`@orchbee/terminal` 拥有精确快照准备。Workbench 拥有语义 Action
标识符。Web 拥有私有查看器、快捷键投影、本地化展示与设备本地快捷键迁移。应用拥有的剪贴板
capability 仍是唯一剪贴板写入者。

已接受结果是一个完整、不可变的保留快照纯文本视图。非目标包括上述被排除的 Kitty pager 能力、
改变保留策略、修改启动文件、滚动或选择实时终端，以及增加另一套模态交互实现。

## 领域模型

`ProjectedTerminal.readScrollback()` 是唯一公开的准备操作。其输入是一个 `AbortSignal`、已挂载终端
的实时主屏幕 metadata、当前展示的可见顶部，以及一个精确的 Terminal Host
generation/revision/总行数快照。缺少 metadata、已处置和备用屏幕属于 unavailable，而不是 empty。

该操作通过现有通用缓冲区 lane 从旧到新读取 text-detail 行，同时最多只有一个页面在途。每个接纳
的页面都必须匹配捕获的 generation、revision、总行数、请求起点、text detail、连续行标识与剩余
字节额度。页面大小受协议行数与 cell 数限制。编码来源总额度为 16 MiB；额度耗尽（包括单行溢出）
返回 `source-limit`，且绝不发布前缀。

确定性投影器按绝对行升序产生结果：

1. 每个保留行都贡献 Host 投影且裁掉尾部空白的纯文本，包括空行。
2. Ghostty 软换行行与后继行直接连接；其他每个边界变成换行符。
3. `initialTopOffset` 是准备开始时位于可见顶部的行之前的 UTF-16 offset。它会限制在保留快照内，
   且绝不会通过搜索文本推断。
4. 零行快照产生 `empty/no-scrollback`。非零快照即使投影文本为空也产生 `scrollback`，因为保留的
   空白行仍是可检查的快照。

`ProjectedScrollbackReadResult` 恰好是 `scrollback`、`empty`、`cancelled` 或 `unavailable` 之一。
scrollback 结果携带 generation、revision、首尾行、纯文本和 `initialTopOffset`。过时或淘汰的页面
映射为 `unavailable/stale`；畸形页面、传输失败、处置或不支持状态映射为类型化 unavailable reason。
最后一页之后，`ProjectedTerminal` 会在返回可发布结果前重新检查屏幕准入与精确快照。取消和所有失败
都会保持 Terminal Host、实时视口、所选内容、附件身份与之前打开的查看器文档不变。

## 查看器与 Action 模型

Web 为每个投影终端挂载拥有一个私有终端文本查看器。其文档是带标签的不可变联合，来源为
`command-output` 或 `scrollback`；来源专用适配器会在打开前转换终端读取结果。一个查看器 controller
拥有可逆的查询、活动匹配、打开状态与焦点请求状态。打开任一来源都会替换之前的文档并重置搜索。
关闭会恢复终端焦点，而不改变 Host 状态。

字面搜索继续不区分大小写、互不重叠、按 UTF-16 来源 offset 排序，查询最多 1,024 个字符且最多
渲染 1,000 个匹配项，并独立于实时缓冲区搜索。回滚缓冲区文档在 `initialTopOffset` 渲染一个无副作用
锚点，并在初始展示时把该锚点滚动到顶部。Enter 与 Shift+Enter 循环匹配，`Mod+F` 聚焦查询，
Escape 关闭，并且模态焦点循环保持封闭。复制会调用来源专用语义 Action，并通过应用剪贴板
capability 准确写入不可变纯文本。

Workbench 除现有命令输出 Actions 外，还拥有 `terminal.history.show` 与 `terminal.history.copy`。
Web 在已挂载终端 scope 中注册两者。任一显示 Action 启动时，其查看器本地的最新准备协调器都会取消
较旧的通用缓冲区读取；每个 Action 的 replace-latest 策略则分别处理同一 Action 的重复调用。handler
会在关闭实时查找和替换查看器文档前检查取消。仅当打开的是回滚缓冲区文本且剪贴板 writer 可用时，
回滚缓冲区复制才启用。

## 绑定与持久化

Apple 和 Standard 平台都把显示回滚缓冲区默认绑定为 `Ctrl+Shift+H`；当 Kitty modifier 使用默认的
Control+Shift 时，这与 Kitty 的默认 `kitty_mod+h` 一致。该绑定只适用于聚焦终端，并投影到终端
上下文菜单、Action 搜索、快捷键 Settings 与本地化参考文本。查看器拥有焦点时会抑制终端快捷键。

增加该绑定会把 Web 的严格设备本地快捷键记录从版本 10 扩展到版本 11。版本 11 reader 会迁移
精确的 v10 目录与默认值，拒绝与新默认值的冲突，只写入规范 v11 记录，并原样保留 v10 作为回滚
快照。

## 正确性与验证

本变更不引入新的并发原语。现有 `TerminalRowFlightCoordination` 模型覆盖精确的单在途通用缓冲区读取与
过时响应隔离，Action runtime 的 replace-latest 模型覆盖同一 Action 在发布前的取消。查看器本地协调器
复用提示符导航已有的 AbortController 交接模式，并具有确定性的跨来源替换与处置测试。新的投影器与
查看器 controller 不含时间、进程、网络、存储或 DOM 依赖；DOM 焦点与锚点放置由 adapter 证据覆盖。
不选择新的 TLA+ 模型或 Lean 定理，因为本变更细化这些已有并发模型，并且只声明有界文本投影性质。

| 声明                                                              | 证据                                                                         | 限制                                  |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------- |
| 有序行覆盖、软换行连接、精确锚点 offset、页面校验与来源上限       | `src/projected/__tests__/projected-scrollback.unit.test.ts`                  | 合成 text 行与确定性 reader           |
| 主屏幕准入、可见顶部捕获、取消与最终过时拒绝                      | `src/projected/__tests__/projected-terminal.unit.test.ts` 加 row-flight 模型 | 模拟 Host 行窗口与现有通用缓冲区 lane |
| 带标签查看器替换、搜索、锚点、焦点、复制、Action、绑定与 v10 迁移 | Web controller、component、Action、快捷键、Settings、i18n 与 storage 测试    | 进程内 DOM、effects 与设备本地存储    |
| 快捷键与可见控件到达同一生产查看器                                | `terminal-parity/FS-09-scrollback-viewer`                                    | 使用有界合成保留历史快照的 Chromium   |
