# Kitty 命令输出查看器对齐

## 参考范围

本契约采用 2026-08-27 观察到的 Kitty commit
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 中常用的 `show_last_command_output` 工作流。
Kitty 在 `kitty/options/definition.py` 中映射 `kitty_mod+g`；`kitty/window.py` 提取上一条 Shell
命令输出；`kitty/boss.py` 则把 ANSI 文本发送到 overlay 窗口中的可配置外部 pager。

Orchbee 采用单 Action 工作流、Shell 语义边界、可搜索只读视图、可复制性、取消以及
`Ctrl+Shift+G` 默认绑定。它有意使用应用拥有的查看器，使同一交互可以在 Desktop、Web 和后续
移动端客户端中工作。对齐声明不包含 Kitty 的外部 pager 进程、pager 配置与环境、ANSI 样式、
独立 pager 历史、完整 scrollback 浏览、首条/最近访问/最近非空输出变体或任意程序启动。

## 领域契约

`ProjectedTerminal.readLastCommandOutput()` 是唯一公开的准备操作。它只对已挂载且保留投影历史的
主屏幕附件可用。一次调用会捕获准确的 Terminal Host generation、revision 与总行数，再通过现有
通用缓冲区 lane 从新到旧读取 render-detail 行。每个接纳的页面都必须匹配该快照、请求区间、
detail 与字节额度。编码来源总额度为 16 MiB；当较低的语义边界超出额度时，操作绝不发布局部结果。

确定性提取器忽略未写入 cell，并使用 Host 编写的 OSC 133 提示符行与 cell 语义内容类别：

1. 最新提示符之后已写入的输出，是当前上一条命令的输出区域。
2. 否则，最新提示符是排他的上边界，前一个提示符是上一条已完成命令的下边界。
3. 当较旧提示符已被淘汰，但较新的提示符证明存在 Shell Integration 时，返回直到缓冲区起点的
   保留输出，并设置 `truncatedStart: true`。
4. 没有提示符证据的缓冲区产生 `no-shell-integration`；没有已写入输出的有界区域产生
   `no-output`。
5. 只有归类为输出的已写入 cell 才贡献文本。由 Ghostty 软换行连接的物理行保持为一个逻辑行；
   硬行边界变成换行符；提示符与输入 cell 绝不进入结果。

结果恰好是 `output`、`empty`、`cancelled` 或 `unavailable` 之一。`output` 携带纯 UTF-8 文本
快照、保留区间的首尾行以及截断事实。Empty reason 是稳定语义值。过时或淘汰的快照、备用屏幕、
畸形页面、处置、单行溢出、传输失败或来源耗尽都会产生类型化 unavailable reason，且绝不复用
先前结果。

## 查看器与 Action 模型

Web 为每个投影终端挂载拥有一个私有 terminal-text viewer。其不可变 document 会标记为命令输出或
回滚历史，共享 controller 只拥有可逆的查询与导航状态。打开新结果会替换旧快照、重置查询并聚焦
查询字段。Escape 或可见的关闭控件会关闭查看器并恢复终端焦点。关闭绝不会取消、清除、选择或滚动
Terminal Host 状态。

字面搜索不区分大小写，按 UTF-16 来源 offset 排序、互不重叠，并把渲染匹配限制为 1,000 个。
Enter 与 Shift+Enter 循环向前和向后导航。当查看器拥有焦点时，`Mod+F` 会聚焦并选中查看器查询。
搜索绝不会改变终端独立的保留缓冲区搜索 session。内容保持为原生可选择文本；复制输出使用应用拥有
的剪贴板 capability，并在越过 Action commit point 前报告其效果。

Workbench 拥有 `terminal.output.show-last` 与 `terminal.output.copy`。Web 在已挂载终端 scope 中注册
两者。显示上一条命令输出使用 replace-latest 并发：替换会取消较旧的精确行读取，handler 在发布
已准备结果前再次检查取消。复制输出只在打开的非空结果与可用 clipboard writer 同时存在时启用。
共享查看器仍是私有 business component，因为规范终端 Find Surface 搜索实时 Host 缓冲区，不能
拥有不可变的已准备文本快照。

## 绑定与持久化

Apple 和标准平台都把显示上一条命令输出默认绑定为 `Ctrl+Shift+G`；当 Kitty modifier 使用默认的
Control+Shift 时，这与 Kitty 的默认 `kitty_mod+g` 一致。该绑定适用于聚焦的终端，并投影到终端
上下文菜单、Action 搜索、快捷键 Settings 与本地化参考文本。

增加该绑定会把 Web 的严格设备本地快捷键记录从版本 9 扩展到版本 10。版本 10 reader 会迁移
精确的 v9 目录与默认值，拒绝与新默认值的冲突，只写入规范 v10 记录，并原样保留 v9 作为回滚
快照。

## 验证

| 声明                                                       | 证据                                                                         | 限制                                      |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------- |
| 边界选择、语义过滤、换行、边界与来源限制                   | `src/projected/__tests__/projected-command-output.unit.test.ts`              | 合成 render 行与确定性 reader             |
| 精确快照读取、取消、过时拒绝与主屏幕准入                   | `src/projected/__tests__/projected-terminal.unit.test.ts` 加 row-flight 模型 | 模拟 Host 行窗口与现有有界通用缓冲区 lane |
| 查看器搜索、焦点、无障碍、复制效果、Action、绑定与 v9 迁移 | Web controller、component、Action、快捷键、Settings、i18n 与 storage 测试    | 进程内 DOM、效果与设备本地存储            |
| 快捷键与可见控件到达同一生产界面                           | `terminal-parity/FS-08-command-output-viewer`                                | 使用合成 OSC 133 render 行的 Chromium     |
