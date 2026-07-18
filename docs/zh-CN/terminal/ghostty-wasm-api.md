# Ghostty WASM API

`packages/terminal` 使用上游 `libghostty-vt`，并应用仓库拥有的 DECRQSS 补丁 `packages/terminal/patches/ghostty-vt-decrqss.patch`。不要应用历史 `ghostty-web` API 补丁。

使用 [Ghostty 终端面板符合性](./ghostty-surface-test-provenance.md)判断上游测试是否属于 Orchbee。

## 构建

从仓库根目录运行 `pnpm build:ghostty-wasm`。该命令需要 Git、访问 GitHub 的网络连接和 Zig 0.15.2。如果该版本不在 `PATH` 上，设置 `ZIG=/path/to/zig`；Homebrew 使用 `brew install zig@0.15` 安装它，该命令也会检测 Homebrew 默认的 Apple Silicon keg 路径。

该命令把 Ghostty commit `91f66da24527fa02d92b5fd0b41cd020f553a64c` 克隆到操作系统临时目录，验证并应用 DECRQSS 补丁，使用 `ReleaseSmall` 为 `wasm32-freestanding` 构建 `libghostty-vt`，验证 WebAssembly 模块，然后原子替换 `packages/terminal/ghostty-vt.wasm`。构建失败时，已跟踪产物保持不变，临时检出会被删除。

WASM 模块必须导出 `memory` 和 `__indirect_function_table`。

## ABI

升级 Ghostty 时从 `ghostty_type_json()` 读取生成的 ABI 元数据；绑定当前使用的 wasm32 布局如下：

| 类型                               | 大小 | 重要偏移                               |
| ---------------------------------- | ---: | -------------------------------------- |
| `GhosttyTerminalOptions`           |    8 | `cols=0`、`rows=2`、`max_scrollback=4` |
| `GhosttyPoint`                     |   24 | `tag=0`，坐标 `x=8`、`y=12`            |
| `GhosttyGridRef`                   |   12 | `size=0`、`node=4`、`x=8`、`y=10`      |
| `GhosttyPointCoordinate`           |    8 | `x=0`、`y=4`                           |
| `GhosttyMousePosition`             |    8 | `x=0`、`y=4`                           |
| `GhosttyMouseEncoderSize`          |   36 | `size=0`，尺寸和 padding `4..32`       |
| `GhosttySelectionGestureBehaviors` |   12 | 点击行为 `0`、`4`、`8`                 |
| `GhosttySelectionGestureGeometry`  |   16 | 几何字段 `0`、`4`、`8`、`12`           |
| `GhosttyStyle`                     |   72 | flag `56..64`                          |
| `GhosttyRenderStateColors`         |  784 | 调色板始于 `14`                        |

`GhosttyTerminalOptions.max_scrollback` 是原始终端页面的字节预算。Ghostty 将其舍入到终端页面边界，并始终为活动 viewport 保留足够页面；`0` 禁用历史记录。`@orchbee/terminal` 默认使用 10 MB。

WASM `i64` 参数（例如 `GhosttyCell` 和 `GhosttyRow`）在 JavaScript 中是 `bigint` 值。

## 集成

| 能力                            | 上游 API                                                                       |
| ------------------------------- | ------------------------------------------------------------------------------ |
| 生命周期和 VT 输入              | `ghostty_terminal_new/free/reset/resize/vt_write`                              |
| 终端响应                        | `GHOSTTY_TERMINAL_OPT_WRITE_PTY` callback                                      |
| Bell 和标题事件                 | `GHOSTTY_TERMINAL_OPT_BELL/TITLE_CHANGED` callback                             |
| 帧快照                          | `ghostty_render_state_update` 加行/单元格 iterator                             |
| 单元格文本和解析后的颜色        | `ghostty_render_state_row_cells_get`                                           |
| 单元格宽度和 hyperlink 是否存在 | 对原始单元格调用 `ghostty_cell_get`                                            |
| Scrollback 和 OSC 8 URI         | `ghostty_terminal_grid_ref` 加 `ghostty_grid_ref_*`                            |
| 跨 reflow 的 viewport anchor    | `ghostty_terminal_grid_ref_track` 加 `ghostty_tracked_grid_ref_point`          |
| 模式                            | `ghostty_terminal_mode_get`                                                    |
| 键盘编码                        | `ghostty_key_encoder_setopt_from_terminal` 加 `ghostty_key_encoder_encode`     |
| 鼠标编码                        | `ghostty_mouse_encoder_setopt_from_terminal` 加 `ghostty_mouse_encoder_encode` |
| 焦点和粘贴编码                  | `ghostty_focus_encode`、`ghostty_paste_is_safe`、`ghostty_paste_encode`        |
| 选择手势                        | `ghostty_selection_gesture_*`，包括 `DRAGGED` 状态                             |

每个请求帧调用一次 `ghostty_render_state_update`，捕获一次 iterator 数据，并从该快照服务 renderer/buffer 使用方。

Grid reference 用于 scrollback、链接和其他非帧查找；不要在活动渲染循环中使用它们。

### DCS 响应补丁

上游 `libghostty-vt` 当前会在仅终端 stream handler 中忽略 `dcs_hook`、`dcs_put` 和 `dcs_unhook` 操作，尽管 Ghostty 原生 termio stream handler 会为 DECRQSS 查询发出 DECRPSS 响应。仓库补丁复用 Ghostty 的 DCS 状态机，并通过现有 `GHOSTTY_TERMINAL_OPT_WRITE_PTY` callback 路由 DECRPSS；它不添加另一套 VT parser，也不公开历史 `ghostty-web` ABI。

当固定的上游修订版在 `src/terminal/stream_terminal.zig` 中处理 DECRQSS 时，移除补丁及其构建步骤。在此之前，同时更新固定修订版、补丁上下文、ABI 元数据和已跟踪 WASM 产物。

## JavaScript callback

浏览器和 Node 运行时无法可移植地把普通 JavaScript 函数插入 `WebAssembly.Table`；`WasmBridge` 创建带类型的 WASM trampoline、扩展导出的 table，并在终端释放时清除每个槽位。

Callback 在 `ghostty_terminal_vt_write` 期间同步运行；返回前复制借用的字节，绝不能在同一终端上重入 `vt_write`。

## 升级检查

1. 更新 `scripts/build-ghostty-wasm.mjs` 中固定的修订版和补丁上下文。
2. 运行 `pnpm build:ghostty-wasm` 并提交重新构建的 `ghostty-vt.wasm`。
3. 将 `ghostty_type_json()` 与 `types.ts` 中的常量比较。
4. 对照上游 header 验证枚举值。
5. 运行类型检查、单元测试、Node 集成测试和 WebGPU 浏览器 demo。
