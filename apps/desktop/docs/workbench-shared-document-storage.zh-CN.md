# Desktop 共享 Workbench 文档存储

## 范围和所有权

Desktop Main 拥有来自 `@orchbee/workbench/document` 的 `WorkbenchSharedDocument` 的特权持久效果。 Workbench 拥有 Project、Workspace、Tab、组、Panel、Split、终端呈现、生命周期意图、验证和Client View 协调语义。 Desktop 既不重新定义这些字段，也不向预加载或渲染器公开文件系统权限。

该 contract 涵盖一个 private shared-document file、exact-next compare-and-swap revision、serialized file
operation、一个 renderer-document-scoped capability port，以及 terminal lifecycle effect 之前的
write-ahead ordering。Store 自身不 merge concurrent edit、不持久化 Client View、不暴露 network service、
不推断缺失 binding、不采用未引用 binding，也不定义 recovery UI。Desktop session coordinator 单独拥有
canonical multi-client proposal queue 与 broadcast。

同一协调器提供进程内、仅 Main 可用的规范文档变更队列。托管的本地
[Worktree 创建](project-worktree-creation.zh-CN.md)和[接纳](project-worktree-adoption.zh-CN.md)通过该队列，
与渲染器提案一起提交 Directory、Workspace 和 Terminal Context 变更。当前 Desktop 终端效果要求本地
placement。有效文档可以保留旧的 Environment placement，但它不提供 SSH 准入、远程 provision 或远程投影能力。

## 存储状态和修订策略

该存储（store）只返回一种状态：

| 状态          | 含义                                             |
| ------------- | ------------------------------------------------ |
| `missing`     | 新的共享文档路径不存在。                         |
| `available`   | 完整的文件解析为一个规范的版本 1 共享文档。      |
| `unavailable` | 路径不可读、非文件、无效 UTF-8/JSON 或无效文档。 |

当前文件名是 `workbench-shared-document-v1.json`。`load()` 只读取该路径并严格校验当前 version-1 图。缺失返回 `missing`；无效或不可读返回 `unavailable` 且不改写。旧文件、历史 DTO 与回退迁移不再准入。

当前图要求完整资源命名空间、Directory placement 与显式 Tab `customTitle`。缺失这些字段的旧 v1 不能被误认为当前记录。Project 分组不定义 Directory 或远程终端权限。

`commit(expectedRevision, document)` 与存储实例上的每个操作一起序列化，并应用以下规则：

1. 丢失的文件仅接受 `expectedRevision = null` 和候选修订版 `0`。
2. 可用文件仅接受等于其当前修订版的期望。
3. 候选修订版必须恰好比预期大 1。
4. 过时的期望返回 `conflict` 以及实际修订版本并保留文件。
5. 跳过、重复或以其他方式非下一个候选者将返回 `rejected/revision-sequence`。
6. 无效或不可读的当前数据为 `unavailable`，并且永远不会被覆盖。
7. 确认发生在 rename 前的失败是 `unavailable/write-failed`。rename 后的失败是
   `unavailable/write-indeterminate`；调用者在重试前 restart 或再次 load。

修订版是规范的十进制文本，并通过 `BigInt` 推进；他们没有 JavaScript number 上限。仅提交更改的共享指纹。 Client-本地导航、公开、焦点、活动子项和 Split-权重转换不会更改该指纹，因此不会推进修订。

## 文件效果

该路径是绝对路径，并且是 Desktop Main 私有的。提交验证完整的候选者，创建仅限所有者的父目录，写入新的独占随机临时文件，同步并关闭它，在目标上重命名它，并在支持的情况下同步父目录。结果是以换行符结尾的 UTF-8 JSON，具有仅限所有者的权限。预重命名失败会删除临时文件。rename 后的父目录同步失败无法证明新文件是否 durable，因此属于 indeterminate，而不是安全的否定结果。

共享文档存储与终端绑定分类账是分开的。模式、所有者、故障恢复和更新时间不同；没有事务假装以原子方式写入两者。

## 能力边界

活动渲染器通过原子 Desktop bootstrap 接收初始文档状态，再经其 `client-session` `MessagePort`
提交完整文档提案。每项提案携带有界请求标识、预期修订和严格校验的共享文档；Main 拥有存储加载与提交。
关联结果和有序的规范发布使用同一会话能力。存储路径、Node 错误、绑定记录、Host 身份、通用 IPC
及文件系统操作不属于文档控制能力。

沙盒 preload 在通过 `window.postMessage` 转发前，校验一次 bootstrap 和
[`DESKTOP_RENDERER_PORT_ROLES`](../../../packages/desktop-client-protocol/src/desktop-renderer-bootstrap-contract.ts)
声明的完整能力包；它不暴露 `ipcRenderer` 对象或通用发送接口。Main 拒绝畸形命令和传入的端口。
内容完全相同的重复请求会去重；用同一标识提交不同内容会使该会话退役。处置会抑制迟到响应和后续提案准入。

旧远程 placement 保护允许精确保留或删除、已准入的 `provision` 到 `retain` 收敛、精确整 Tab 关闭意图，
以及 Tab 显示元数据变更。它拒绝伪造 Directory、Workspace、Presentation、launch-CWD、placement 或拓扑
权限。该提案来源检查独立于结构解析，不会授权远程效果。

## 渲染器和预写工作流程

Main 在发布渲染器 bootstrap 前加载共享文档。可用文档会水合托管的 Workbench 运行时，不派发进程命令。
存储缺失时提供一个公共初始化文档；Main 只有在其修订 `0` 提交后才能释放初始 provision 效果。
存储不可用时，不能发布替代共享结构或派发新的终端效果。

渲染器协调器只观察共享文档投影变化，并投影完整文档。一项提案待处理时，它会合并后续变化。
Client View 转换使用独立观察者，不能调度写入；指纹比较仍是协调器的防线。Main 先发布已提交文档，
再协调其中的 provision、close 或应用 stop 意图。等价竞态不重复写入；存在分歧的陈旧提案会返回规范文档，
渲染器应用它而不重放败出的本地变更。被拒绝、不可用或结果不明确的提案会关闭该渲染器协调器的准入，
不能因此覆盖存在分歧的文档。

协调器启动后，可用的 `retain` Presentation 会启动仅恢复协调。Main 只接受恢复那些已经由其账本保留，或在同一 Main 生命周期内建立的绑定。恢复绝不会分配新终端。渲染器处置会抑制待处理响应与新调度。仍在 session queue 中等待的 document proposal 会在其 renderer generation 被 dispose 后、抵达 storage 前被 suppress；Main 已经接受的 proposal 或 terminal effect 仍可完成。Main 会按 durable terminal operation identity 保留 settled result，因此新的 renderer generation 可以应用该 result，而不会再次执行相同 effect。尚未 settled 的 durable intent 仍可继续 reconcile。

Main 拥有的变更使用同一修订队列。规范文档初始化后，被拒绝或没有变化的变更不再分配修订，也不写入。成功的持久提交先于
通知、归档协调、客户端发布和终端协调；某个提交后观察者失败，不能撤销提交或阻止其他观察者。
不确定的存储结果会冻结规范控制，直至恢复。显式协调可以重试未变化的当前文档，不伪造另一修订。
规范包含关系必须把每个 Presentation 唯一解析到本地运行时；缺失、歧义和 Environment 目标均在终端效果前失败。

## Client View

Desktop 不会把 `WorkbenchClientView` 序列化到此文件或浏览器键值存储。每个渲染器拥有自己的内存视图。
Web 组合可以按[导航路由](../../web/docs/navigation-routing.zh-CN.md)的约定，在当前文档中恢复一个有界、
会话本地的焦点提示。活动 Workspace/Tab、展开状态、焦点、最大化、活动子项和 Split 权重仍在共享文档之外，
不能成为另一页面的导航权威。

## 不变量和证据

| 声明                                   | 证据                                                                | 限制                                         |
| -------------------------------------- | ------------------------------------------------------------------- | -------------------------------------------- |
| 仅精确下一修订的 CAS 能更改存储修订    | [修订决策测试][decision-tests]和[文件存储测试][store-tests]         | 缺失、过期、非下一修订、大修订值及并发写入者 |
| 已处置的排队工作在存储前被抑制         | [会话队列测试][queue-tests]                                         | 受控的准入前后断连；已准入工作仍可完成       |
| 无效或当前不可用的状态不会被覆盖       | [文件存储测试][store-tests]                                         | 选定的损坏和文件系统故障                     |
| 当前文件与当前形状是唯一权威           | [文件存储测试][store-tests] 与 [文档测试][document-tests]           | 严格解析、无效输入保留与选定写失败           |
| 仅 Client View 变化不写共享状态        | [文档运行时测试][runtime-tests]和[渲染器协调器测试][renderer-tests] | 每个 Client View 保持客户端本地              |
| 渲染器只接收声明的文档能力             | [会话 broker 测试][broker-tests]和[preload 测试][preload-tests]     | 信任 Electron 结构化克隆                     |
| Main 拥有的变更与渲染器提案共用队列    | [会话队列测试][queue-tests]和[Worktree 运行时测试][worktree-tests]  | 受控的进程内提交与故障轨迹                   |
| 旧 placement 不能授予远程效果权限      | [提案保护测试][proposal-tests]和[目标解析测试][target-tests]        | 当前 Desktop 效果仅支持本地                  |
| 持久意图先于效果，已结算重试不重复执行 | [会话队列测试][queue-tests]和[协调器测试][reconciler-tests]         | 当前操作标识；仍信任 Host/账本的幂等性       |
| 恢复不能创建替代终端                   | [生命周期适配器测试][adapter-tests]                                 | 仍信任绑定账本和 Host 协调                   |
| rename 前的清理写入失败保留存储意图    | [文件存储故障测试][store-tests]和[文档运行时测试][runtime-tests]    | 选定的写入失败与水合轨迹，不是工作流模型     |

存储与会话队列各自只有一个进程内串行化所有者。上面的实现测试直接检查修订决策、竞争提交、
准入前后断连和队列排空。此边界不声明 TLA+ 安全性或进展性的穷举保证；完成轨迹假设等待的操作
最终结算。文件测试覆盖选定的严格准入和写入故障，不证明一般文件系统持久性。多客户端仲裁、发布、
断开和 quiescing 的证据由[会话协调器](multi-client-session-coordinator.zh-CN.md)拥有。

[decision-tests]: ../src/main/workbench-document/__tests__/decideWorkbenchDocumentCommit.unit.test.ts
[store-tests]: ../src/main/workbench-document/__tests__/FileWorkbenchDocumentStore.unit.test.ts
[document-tests]: ../../../packages/workbench/src/workbench/document/__tests__/workbench-document.unit.test.ts
[runtime-tests]: ../../../packages/workbench/src/workbench/model/__tests__/workbench-document-runtime.unit.test.ts
[renderer-tests]: ../../web/src/workbench-client/workbench-document-control/__tests__/WorkbenchDocumentCoordinator.unit.test.ts
[queue-tests]: ../src/main/client-session/__tests__/DesktopClientSessionCoordinator.unit.test.ts
[broker-tests]: ../src/main/client-session/__tests__/DesktopClientSessionBroker.unit.test.ts
[preload-tests]: ../src/preload/__tests__/installDesktopRendererBootstrapBridge.unit.test.ts
[worktree-tests]: ../src/main/project-worktree/__tests__/DesktopProjectWorktreeRuntime.unit.test.ts
[proposal-tests]: ../src/main/client-session/__tests__/validateDesktopClientRemotePlacementProposal.unit.test.ts
[target-tests]: ../src/main/client-session/__tests__/desktop-terminal-effect-target.unit.test.ts
[reconciler-tests]: ../src/main/client-session/__tests__/DesktopTerminalIntentReconciler.unit.test.ts
[adapter-tests]: ../src/main/terminal-document-control/__tests__/DesktopTerminalLifecycleAdapter.unit.test.ts

<!-- documentation-pairing: english-sha256=95a55ca9ac783613c2f36ddadf875901a15a03737e2b1e64edba696b20a6e5ff zh-cn-sha256=466290946d3e089386330884ae3a861b56964f52c6f572bdb1ad532a2d4bfe0a -->
