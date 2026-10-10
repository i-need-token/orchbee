# 横向分组拖拽正确性

此目录拥有单个 Tab Strip 内移动横向分组头的可执行证据。本地拖拽和分组流程使用实现测试，不另设 TLA 模型，也不声明全部交错的穷举保证。

## 证据与映射

| 证据          | 相对包根目录的产物                                                                                                                    | 声明与限制                                                                                |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| 契约          | `docs/horizontal-group-drag.md`                                                                                                       | 来源事实、状态、事件、不变量及可观察检查点                                                |
| Lean          | `formal/lean/MoveContiguousBlock.lean`                                                                                                | 连续块向左/右移动保留排列、长度及块内顺序                                                 |
| 属性/细化测试 | `src/tab-strip/dragging/__tests__/horizontal-group-drag-machine.unit.test.ts`, `src/tab-strip/model/__tests__/tab-model.unit.test.ts` | 固定种子拖拽轨迹细化 Lean 范式；生成转换覆盖身份、分组、连续性、选择、过期/被拒动作及取消 |
| 系统轨迹      | `system-tests/horizontal-group-drag.spec.ts`                                                                                          | 指针提交/取消、展开/折叠分组、焦点、覆盖层、语义与绘制顺序                                |

权威 Tab 顺序和分组成员映射到列表输入/输出与 DOM 集合顺序。折叠意图映射到 `aria-expanded`；活动/选择/锚点身份映射到语义检查点与可见活动表面。拖拽阶段和可视成员身份映射到覆盖层/根节点及拖动项。边界移动比较 Lean 列表范式；取消比较准确捕获的语义与绘制快照。DOM 焦点仍归适配器所有。

## 重现

```sh
pnpm --filter @orchbee/tab-strip verify:correctness
pnpm --filter @orchbee/tab-strip test:property
pnpm --filter @orchbee/tab-strip test:lean
pnpm --filter @orchbee/tab-strip test:system
```

Lean 启动器校验 elan 4.2.1 的公开 SHA-256，安装在被忽略的包内 `.tools` 目录，并运行 Lean 4.30.0。浏览器测试在 macOS 使用已安装 Google Chrome；`TAB_STRIP_BROWSER_EXECUTABLE` 可覆盖可执行文件。属性种子为 20260721。系统测试固定视口、语言、配色、设备缩放、worker 数及初始状态，等待语义/动画完成。

## 来源限制

迁移源码固定在上游提交 `e4f73f6`；BSD 归属保留在 `NOTICE.reference`。尚未基于单独固定浏览器构建捕获可移植参考语料。证据提供内部一致性及回归保护，不证明与每个浏览器版本等价。
