# 正确性试点

该目录是包正确性合约的第一个可执行片段。它的范围被故意缩小：将水平组标题移动到一个选项卡条内。它证明了有关迁移模型和实施的本地主张；它并不声称与上游浏览器的每个版本完全等效。

## 证据层

| 图层          | 神器                                                                                                                                  | 它建立了什么                                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| 行为模型      | `docs/horizontal-group-drag.md`                                                                                                       | 范围、源事实、状态、事件、不变量和可观察的检查点                                                           |
| TLA+          | `formal/tla/HorizontalGroupDrag.tla`                                                                                                  | 可达的六 Tab 图满足 identity、成员关系、连续性、整块移动、selection anchor、取消、拒绝和拖动生命周期不变量 |
| Lean          | `formal/lean/MoveContiguousBlock.lean`                                                                                                | 向左或向右移动连续块会保留元素排列、总长度和块的内部顺序                                                   |
| 细化/属性测试 | `src/tab-strip/dragging/__tests__/horizontal-group-drag-machine.unit.test.ts`、`src/tab-strip/model/__tests__/tab-model.unit.test.ts` | 固定种子拖动场景细化 Lean 范式；生成的模型转换负责分组、连续性、身份和成员顺序证据                         |
| 系统痕迹      | `system-tests/horizontal-group-drag.spec.ts`                                                                                          | 确定性浏览器进程指针跟踪检查展开和折叠的提交、取消、覆盖合成、选择、收集顺序和最终绘制顺序                 |

该表中的路径是相对于包根目录的。

## 状态映射

| 行为概念      | TypeScript                                           | TLA+                                                                   | Lean/系统投影                                                      |
| ------------- | ---------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------ |
| 权威 Tab 顺序 | `authoritative.tabs`                                 | 精确的六 identity `tabs` sequence 与 `previousAuthoritative`           | 列表输入／输出与水平 DOM collection 顺序                           |
| 组成员关系    | `authoritative.tabs[].group`                         | per-tab `groupOf`、已知组和连续性不变量                                | 生成式模型成员关系与 Lean 成员顺序保持                             |
| 折叠意图      | `authoritative.groups[].collapsed`                   | `collapsed` 与 `originalCollapsed`                                     | checkpoint 中的 `aria-expanded`                                    |
| 活动／选择    | `activeTabId`、`selectedTabIds`、`selectionAnchorId` | `active`、`selected`、`selectionAnchor`、`recordedActive` 与派生活动面 | 可见活动 Tab 或折叠组 header DOM、选中 identity 与测试页面 heading |
| 拖动阶段      | `drag.kind`                                          | `phase`                                                                | overlay/root 存在：armed、dragging、settling、idle                 |
| 整组视觉      | `visualKind` 与 `memberIds`                          | `visual` 与精确有序的 Research member block                            | ghost header 与被拖动 Tab identity                                 |
| 边界移动      | `move-to(preRemovalTargetIndex)`                     | 三个有界合法源块位置与不变的 outsider 顺序                             | Lean 左／右列表范式与最终 DOM／model 顺序                          |
| 取消          | `cancel` 加捕获的 `original`                         | `CancelDrag` 恢复包括 `selectionAnchor` 在内的六个权威字段             | 精确的拖动前语义与绘制 snapshot                                    |

TLA+ 模型包含一个 pinned Tab、两个各有两名成员的组、一个未分组 Tab，以及一个被拖动的源组。它探索每种初始 collapsed subset 与 active identity、三个源块位置，以及 arm 和 activation 之间一次源成员关系失效。具名的 stale、missing、repeated、out-of-phase 和 invalid-target 输入见证权威状态与 command output 保持。模型不对 pointer activation、release、cancellation 或 settling 假设公平性。Lean 与生成式属性测试把移动保持扩展到任意有效成员列表。

DOM focus 仍由 adapter 拥有。`system-tests/horizontal-group-drag.spec.ts` 检查折叠 header 获得焦点的 projection；Escape trace 检查 cancellation 后语义与绘制状态的恢复，但不把 DOM focus 分配给 TLA+ state。已删除的 `TabGrouping` 模型只会选择一个预先枚举的连续集合，并让 Tab 顺序保持不变，因此相较于更强的生成式 `TabModel` transition 与 invariant 套件，它没有增加任何需维护的行为。

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
