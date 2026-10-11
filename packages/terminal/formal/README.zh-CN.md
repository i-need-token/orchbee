# Terminal 正确性证据

Client 所有的异步流程使用受控实现测试。共享跨进程 attachment 协议由 Desktop 的 `projected-terminal-ipc-reliability` 模型单独检查；本地 UI 流程不能仅因等待异步工作就各自要求 TLA 模型。

## 声明与实现证据

路径相对于包根目录。这些是维护中的测试义务，不声明穷举全部状态。

| 声明                                        | 可执行证据                                                                                                             |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Renderer 退役、设备丢失和可见性隔离所有完成 | `src/rendering/__tests__/renderer.unit.test.ts`                                                                        |
| 只有最新且存活的显示配置才能发布全部字段    | `src/projected/__tests__/projected-terminal.unit.test.ts`                                                              |
| 迟到图像读取不能恢复过期预览                | `src/projected/__tests__/TerminalImagePreviewController.unit.test.ts`                                                  |
| 输入释放与修复保留顺序和所有权              | `src/input/__tests__/terminal-input-state.unit.test.ts`、`src/input/__tests__/OrderedInputDispatcher.unit.test.ts`     |
| 行读取、确认和请求退役保留当前视图          | `src/projected/__tests__/projected-terminal.unit.test.ts`、`src/projected/__tests__/projected-scrollback.unit.test.ts` |
| 搜索修订、watchdog 和导航拒绝过期完成       | `src/search/__tests__/projected-search-controller.unit.test.ts`                                                        |
| 提示符导航仅提交当前请求                    | `src/projected/__tests__/projected-prompt-navigation.unit.test.ts`                                                     |
| 选择手势和原生输入结算不重放                | `src/projected/__tests__/projected-terminal.unit.test.ts`、`src/input/__tests__/surface-mouse.unit.test.ts`            |

测试在生产边界控制完成顺序、取代、失败和销毁。实际载荷、几何、时钟和渲染断言保留于此。证据覆盖命名及生成轨迹，不保证所有外部调度或浏览器/GPU 行为。

## 验证

运行 `pnpm --filter @orchbee/terminal test`。共享 attachment 协议变化还需相应已注册 Desktop 模型和适配器测试。仍须执行仓库根门禁；此包不提供 `test:tla` 命令。
