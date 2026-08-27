# Kitty 键盘回滚对齐

## 参考范围

本契约采用 2026-08-27 观察到的 Kitty 提交
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 中主屏幕的逐行、逐页、顶部和底部键盘回滚入口。
Kitty 的 `kitty/options/definition.py` 绑定 `kitty_mod+up`、`kitty_mod+down`、
`kitty_mod+page_up`、`kitty_mod+page_down`、`kitty_mod+home` 和 `kitty_mod+end`；`kitty_mod`
默认为 `ctrl+shift`。其 `kitty/screen.c` 翻页操作按可见行数减一移动，从而保留一行上下文。

对齐声明覆盖这六个主要绑定、它们的主屏幕准入、方向与边界，以及翻页时的一行重叠。它不声明与
Kitty 额外的 `kitty_mod+j/k` 或 macOS Command 别名、每个 Action 的多个绑定、键盘驱动的平滑逐行动画、
像素时长调节、命令输出分页或备用屏幕应用行为一致。Orchbee 的每个 Action 仍只有一个可自定义的有效
绑定，因此别名可以替换默认值，但不能在本目录中与默认值并存。

## 领域契约

`ProjectedTerminal` 拥有本地保留历史视口。Terminal Host 继续权威拥有保留行、总行数边界、生成、
修订和行窗口内容。只有当 `canScrollHistory` 为真时，键盘 Action 才能滚动：终端仍活动、具有当前
元数据并正在呈现主屏幕。只读输入能力不会禁用本地历史读取。

操作消费当前可见行数和历史位置：

- 向上或向下逐行移动一个绝对行；
- 向上或向下逐页移动 `max(rows - 1, 0)` 行，保留一行上下文；
- 顶部请求绝对第零行；
- 底部返回实时的跟随底部投影。

每个获准的移动都会委托给现有的有界历史意图与 Host 行窗口工作流。转换或视口读取期间的重复输入
由该所有者合并；不会创建另一条滚动队列。操作在发布或排队意图时返回 `true`，视口无法移动时返回
`false`。它永远不会修改 Host 历史或发出 PTY 输入。

## 决策与不变量

| Action ID                           | Apple 与标准平台默认值 | 视口转换                 |
| ----------------------------------- | ---------------------- | ------------------------ |
| `terminal.history.scroll-line-up`   | `Ctrl+Shift+Up`        | 朝保留历史移动一行       |
| `terminal.history.scroll-line-down` | `Ctrl+Shift+Down`      | 朝实时底部移动一行       |
| `terminal.history.scroll-page-up`   | `Ctrl+Shift+PageUp`    | 向上移动一个有重叠的页面 |
| `terminal.history.scroll-page-down` | `Ctrl+Shift+PageDown`  | 向下移动一个有重叠的页面 |
| `terminal.history.scroll-top`       | `Ctrl+Shift+Home`      | 保留历史的绝对起点       |
| `terminal.history.scroll-bottom`    | `Ctrl+Shift+End`       | 实时跟随底部投影         |

工作流保持这些不变量：

- 六个固定默认值在 Apple 上使用显式 Control，而不是平台 `Mod` 抽象；
- 已聚焦的主屏幕终端即使到达边界也会消费精确有效组合键，此时 Action 以 `unchanged` 完成且视口保持稳定；
- 备用屏幕或已处置终端会禁用全部六个 Action，因此分发不会调用 `preventDefault()`，终端应用可以处理按键；
- 没有 PTY 输入权威时，本地历史导航仍然可用且不会发布输入；
- 每次移动都会撤销更旧的提示符导航提交权威，并保留现有的生成、修订、请求序列和行窗口验证；
- 逐行、逐页、顶部和底部 API 导航即时完成；这些键盘 Action 不会复用或扩展现有的仅滚轮平滑呈现；
- 顶部和底部在各自边界具有幂等性，每个数量在发布前都受现有历史所有者约束。

## Action 与持久化

Workbench 拥有这六个语义 Action。Web 在已聚焦终端作用域中注册它们，共用一个替换最新的历史滚动
并发键，并把同一张绑定表投影到分发、Action 搜索、标签和设置中。执行和消费按键前会检查同步可用性。

新增六个快捷键 ID 会把 Web 的严格持久键盘目录扩展到版本 7。准确有效的 v6 记录会依据其旧目录与
默认值迁移，等于当前默认值的覆盖会被移除，迁移冲突会被拒绝，而 v6 原样保留为回滚快照。无效当前
记录不会回退到更旧版本。

## 验证

| 声明                                       | 证据                                                      | 限制                                            |
| ------------------------------------------ | --------------------------------------------------------- | ----------------------------------------------- |
| 逐行、重叠翻页、顶部、底部、边界与屏幕准入 | `src/projected/__tests__/projected-terminal.unit.test.ts` | 确定性投影终端和模拟 Host 行窗口                |
| Action 可用性、类型化结果、共享并发与分发  | Web 投影终端 Action 和快捷键单元测试                      | 进程内 Action 运行时与合成键盘事件              |
| 目录标签、无冲突、自定义以及 v6 到 v7 恢复 | Web 目录、设置、控制器和存储单元测试                      | 合成的设备本地存储                              |
| 六个物理组合键移动一个生产保留历史视口     | `terminal-parity/HS-11-keyboard-scrollback`               | 主屏幕上的一个真实 Desktop 附件和 Terminal Host |
