# 正确性试点

该目录是包正确性合约的第一个可执行片段。它的范围被故意缩小：将水平组标题移动到一个选项卡条内。它证明了有关迁移模型和实施的本地主张；它并不声称与上游浏览器的每个版本完全等效。

## 证据层

| 图层          | 神器                                                                                                                                  | 它建立了什么                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 行为模型      | `docs/horizontal-group-drag.md`                                                                                                       | 范围、源事实、状态、事件、不变量和可观察的检查点                                           |
| TLA+          | `formal/tla/HorizontalGroupDrag.tla`                                                                                                  | 可达抽象图满足拖动生命周期、边界、活动面、折叠、选择、取消、反馈和整组运动不变量           |
| Lean          | `formal/lean/MoveContiguousBlock.lean`                                                                                                | 向左或向右移动连续块会保留元素排列、总长度和块的内部顺序                                   |
| 细化/属性测试 | `src/tab-strip/dragging/__tests__/horizontal-group-drag-machine.unit.test.ts`、`src/tab-strip/model/__tests__/tab-model.unit.test.ts` | 固定种子拖动场景细化 Lean 范式；生成的模型转换负责分组、连续性、身份和成员顺序证据         |
| 系统痕迹      | `system-tests/horizontal-group-drag.spec.ts`                                                                                          | 确定性浏览器进程指针跟踪检查展开和折叠的提交、取消、覆盖合成、选择、收集顺序和最终绘制顺序 |

该表中的路径是相对于包根目录的。

## 状态映射

| 行为概念          | TypeScript                                  | TLA+                                                                       | Lean/系统投影                                                          |
| ----------------- | ------------------------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| 权威的 Tab 键顺序 | `authoritative.tabs`                        | 采集粒度 `order`                                                           | 列出输入/输出和水平 DOM 集合顺序                                       |
| 崩溃的意图        | `authoritative.groups[].collapsed`          | `collapsed` 和 `originalCollapsed`                                         | 检查站 `aria-expanded`                                                 |
| 活动/选择         | `activeTabId`、`selectedTabIds`、派生活性面 | `active`、`selected`、`recordedActive`、`ActiveSurface`、`VisibleSurfaces` | 可见的活动选项卡或折叠的组标题 DOM、选定的选项卡标识和测试夹具页面标题 |
| 拖动阶段          | `drag.kind`                                 | `phase`                                                                    | 覆盖/根存在：武装、拖动、稳定、空闲                                    |
| 全组视觉          | `visualKind` 和 `memberIds`                 | `visual` 抽象                                                              | 幽灵标题和拖动的选项卡标识                                             |
| 新组 Tab 键顺序   | `snapshot.tabs[].id`                        | 外拖专用 TLA+ 声明                                                         | 生成的模型和浏览器测试比较分组前后的完整选项卡标识序列                 |
| 边界移动          | `move-to(preRemovalTargetIndex)`            | 相邻集合移动                                                               | Lean 左/右列表定理与最终 DOM/模型顺序                                  |
| 取消              | `cancel` 加捕获的 `original`                | `CancelDrag`                                                               | 精确的预拖动可观察快照                                                 |

拖动专用的 TLA+ 模型刻意将一个标签组表示为单个集合节点。Lean 与 TypeScript 细化测试证明更底层的声明：该节点的成员列表既不会拆分，也不会重新排序。浏览器跟踪检查最终绘制顺序是否与语义集合顺序一致。已删除的 `TabGrouping` 模型只会选择一个预先枚举的连续集合，并让标签顺序保持不变，因此相较于更强的生成式 `TabModel` 转换和不变式套件，它没有增加任何需维护的行为。

## 复现

运行完整的切片：

```sh
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip verify:correctness
```

或者运行一个证据层：

```sh
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:property
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:tla
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:lean
npx --yes pnpm@11.11.0 --filter @orchbee/tab-strip test:system
```

工具版本均已固定。仓库 TLA+ 运行器下载 `tla2tools 1.7.4` 并校验固定 SHA-256；可设置 `ORCHBEE_TLA2TOOLS_JAR` 使用已配置且匹配的副本。Lean 启动器下载 `elan 4.2.1`，校验其发布 SHA-256，将其安装到已忽略的包本地 `.tools` 目录，并执行 Lean `4.30.0`。浏览器测试套件使用 macOS 上安装的 Google Chrome；可设置 `TAB_STRIP_BROWSER_EXECUTABLE` 覆盖可执行文件。

属性套件使用种子 `20260721`。 TLC 使用固定指纹多项式和一名工人。系统套件修复了视口、区域设置、配色方案、设备比例、浏览器工作人员数量和初始固定状态；它等待语义和动画完成而不是任意睡眠。

## 出处限制

迁移实现固定到提交 `e4f73f6`（`feat: harden Chromium tab interaction fidelity`）时的只读上游源码仓库，BSD 归属声明保留在 `NOTICE.reference`。参考浏览器跟踪语料库尚未形成针对独立固定浏览器版本的可移植工件。因此，该试点为本次迁移建立内部一致性与回归保护，但不证明与所有浏览器版本完全一致。
