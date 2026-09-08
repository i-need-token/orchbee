# Desktop 共享 Workbench 文档存储

## 范围和所有权

Desktop Main 拥有来自 `@orchbee/workbench/document` 的 `WorkbenchSharedDocument` 的特权持久效果。 Workbench 拥有 Project、Workspace、Tab、组、Panel、Split、终端呈现、生命周期意图、验证和Client View 协调语义。 Desktop 既不重新定义这些字段，也不向预加载或渲染器公开文件系统权限。

该 contract 涵盖一个 private shared-document file、exact-next compare-and-swap revision、serialized file
operation、一个 renderer-document-scoped capability port，以及 terminal lifecycle effect 之前的
write-ahead ordering。Store 自身不 merge concurrent edit、不持久化 Client View、不暴露 network service、
不推断缺失 binding、不采用未引用 binding，也不定义 recovery UI。Desktop session coordinator 单独拥有
canonical multi-client proposal queue 与 broadcast。

同一 coordinator 暴露进程内且仅 Main 可用的 canonical mutation queue。Slice 3.2A 在 SSH Environment
authorization 与 canonical Project-root inspection 后使用该 queue，把 remote Project、其 Environment
Directory、精确一个 inactive Workspace 与一个 empty Terminal Context 作为同一 next revision 提交。该
mutation 不创建 terminal presentation，也不释放 Host effect；retry 会升级缺少 Workspace 的 legacy remote
Project，否则收敛为 unchanged。

Slice 3.2B 在该已准入 empty context 上增加一次显式 Main-only mutation。它创建首个 remote terminal
Presentation，通过同一 queue 提交其 `provision` intent，随后才 reconcile 目标 Environment runtime。若
retry 发现精确 singleton Presentation，则无需再次 commit 就会 reconcile；不同或非空 shape 会被拒绝。
Runtime availability 在 document 外部观察，因此 disconnect 或 retryable/unknown execution 会保留 pending
durable intent，而不会发布虚假的 terminal outcome。

## 存储状态和修订策略

商店只返回一种状态：

| 状态          | 含义                                             |
| ------------- | ------------------------------------------------ |
| `missing`     | 新的共享文档路径不存在。                         |
| `available`   | 完整的文件解析为一个规范的版本 2 共享文档。      |
| `unavailable` | 路径不可读、非文件、无效 UTF-8/JSON 或无效文档。 |

当前文件名是 `workbench-shared-document-v2.json`。`load()` 首先只读取该路径。只要它存在，无论有效或
无效，它都是唯一 authority，Desktop 绝不 fallback 到 v1。若它缺失，Desktop 可以通过 Workbench 的历史
v1 parser 读取精确 `workbench-shared-document-v1.json` 一次，为每个 Directory 增加 local placement，
并原子写入已校验 v2 document。v1 file 保持逐字节不变。缺失 v1 仍为 `missing`；无效或不可读 v1 是
`unavailable` 且永不修复。不提供反向 migration。

`WorkbenchDocumentPlacementMigration` 在 v1/v2 的 missing、valid 与 invalid 全部组合上检查这种单向
authority 与 v1 不变安全性。其 rename 后 state 是内部 overapproximation，reload transition 只覆盖精确且
成功的 v2 reload。Reload failure 与 settlement progress 由确定性 file-store evidence 覆盖，不属于该
TLA+ model 的 claim。

`commit(expectedRevision, document)` 与存储实例上的每个操作一起序列化，并应用以下规则：

1. 丢失的文件仅接受 `expectedRevision = null` 和候选修订版 `0`。
2. 可用文件仅接受等于其当前修订版的期望。
3. 候选修订版必须恰好比预期大 1。
4. 过时的期望返回 `conflict` 以及实际修订版本并保留文件。
5. 跳过、重复或以其他方式非下一个候选者将返回 `rejected/revision-sequence`。
6. 无效或不可读的当前数据为 `unavailable`，并且永远不会被覆盖。
7. 确认发生在 rename 前的失败是 `unavailable/write-failed`。rename 后的失败是
   `unavailable/write-indeterminate`；调用者在重试前 restart 或再次 load。

修订版是规范的十进制文本，并通过 `BigInt` 推进；他们没有 JavaScript 号码上限。仅提交更改的共享指纹。 Client-本地导航、公开、焦点、活动子项和 Split-权重转换不会更改该指纹，因此不会推进修订。

## 文件效果

该路径是绝对路径，并且是 Desktop Main 私有的。提交验证完整的候选者，创建仅限所有者的父目录，写入新的独占随机临时文件，同步并关闭它，在目标上重命名它，并在支持的情况下同步父目录。结果是以换行符结尾的 UTF-8 JSON，具有仅限所有者的权限。预重命名失败会删除临时文件。rename 后的父目录同步失败无法证明新文件是否 durable，因此属于 indeterminate，而不是安全的否定结果。

共享文档存储与终端绑定分类账是分开的。模式、所有者、故障恢复和更新时间不同；没有事务假装以原子方式写入两者。

## 能力边界

实时渲染器文档通过沙盒预加载接收一个列入白名单的 `workbench-document-control` `MessagePort`。命令仅包含有界请求标识以及加载或提交意图、​​预期修订以及完整的经过验证的共享文档。结果仅包含相关的键入结果。文件路径、节点错误、绑定记录、Host 身份、通用 IPC 和文件系统操作永远不会跨越边界。

Main 拒绝格式错误的命令、传输的端口、错误的请求形状和处理后的消息。 Preload 只接受 Electron 文档的一个经过验证的引导程序，并通过 `window.postMessage` 传输恰好一个端口；它不公开 `ipcRenderer` 对象或通用发送。渲染器响应通过请求 ID 进行关联，重复的 IDs 不可用，并且忽略处理后的延迟事件。

Client proposal 可以保留或删除精确的 Main-admitted remote placement，也只能把精确已准入 remote
Presentation 从 `provision` 收敛到 `retain`。若 Client 引入、relabel、reparent、duplicate、convert、
retarget 或以其他方式 mutate remote Directory、Workspace 或 terminal structure，或复用其受保护 identity，
coordinator 会拒绝并 retire 该 Client。该 proposal-origin check 独立于结构 document parsing。

## 渲染器和预写工作流程

渲染器在构建其托管的 Workbench 运行时之前加载共享文档。可用文档使用确定性的默认 Client View 完成水合，并且不发出任何进程命令。存储缺失时会创建正常的初始运行时，并在发布初始 provision 命令前提交修订版 `0`。存储不可用时，渲染器代会以失败关闭：不渲染、不创建替代共享结构，也不调度终端效果。

渲染器协调器仅观察共享文档投影更改并投影完整文档。它会在一项提交待处理时合并更改。 Client View 转换使用单独的观察者，无法调度写入；指纹比较仍然是协调器的防御措施。配置、恢复、关闭和应用程序停止命令在包含其相应意图的共享文档屏障后面等待。只有成功的精确下一个提交才会释放准备好的命令。冲突、拒绝或不可用的存储会导致渲染器生成永久失败；没有不同的文档被覆盖。

协调器启动后，可用的 `retain` Presentation 会启动仅恢复协调。Main 只接受恢复那些已经由其账本保留，或在同一 Main 生命周期内建立的绑定。恢复绝不会分配新终端。渲染器处置会抑制待处理响应与新调度。仍在 session queue 中等待的 document proposal 会在其 renderer generation 被 dispose 后、抵达 storage 前被 suppress；Main 已经接受的 proposal 或 terminal effect 仍可完成。Main 会按 durable terminal operation identity 保留 settled result，因此新的 renderer generation 可以应用该 result，而不会再次执行相同 effect。尚未 settled 的 durable intent 仍可继续 reconcile。

仅 Main 可用的 remote-Workspace admission 会进入同一 serialized canonical queue，但没有 renderer
write-ahead command。其 Project、Directory、Workspace 与 empty context 要么共用一次成功 file commit，
要么全部保持缺失；rename 后的 indeterminate result 会通过 load 并 retry 相同
`(EnvironmentId, canonicalPath)` key 来 reconcile。

显式 first-remote-terminal operation 也进入该 queue，但在 admission 前不准备任何 Host work。它在 queue
admission 时重新检查 cancellation 与 Environment connectivity，应用精确 Workbench mutation，并 commit
next canonical revision。只有成功 commit 才能向 Environment-specific runtime 释放 reconciliation。
`reconcileCanonicalDocument()` 允许精确 retry 在 canonical Presentation 未变化时 reconcile，而不伪造第二次
shared change。Reconciler 从 canonical Project/Workspace containment 派生 execution target；缺失或 ambiguous
ownership 会 fail closed，而不会 fallback 到 local runtime。

## Client View

Desktop 不会将 `WorkbenchClientView` 写入此文件或浏览器键值存储。渲染器拥有一个内存视图，并在新页面开始后派生默认值。这可以防止共享文档携带活动 Workspace/Tab、披露、焦点、最大化、活动子项或 Split 权重，并避免意外与未来的 Browser 客户端共享页面本地状态。

## 不变量和证据

| 声明                                                 | 证据                                                                             | 限制                                                                |
| ---------------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| 仅下一个 CAS 更改存储的修订版本                      | `DesktopWorkbenchDocumentStore.tla` 和确定性核心测试                             | TLC 通过 `2` 检查两个提交者和修订缺失                               |
| 已 dispose 的 queued work 在 storage 前被 suppress   | `DesktopWorkbenchDocumentStore.tla` 与 session queue regression                  | 两个 renderer generation、每个一个 request；已 accepted work 可完成 |
| 无效/当前不可用状态永远不会被覆盖                    | 文件存储损坏和注入故障测试                                                       | 选定的文件系统故障                                                  |
| v1 migration 单向且 v2-present 权威                  | `WorkbenchDocumentPlacementMigration.tla` 与 migration test                      | 精确 v1；TLA+ 仅覆盖成功的精确 rename 后 reload                     |
| Client View-only transition 不写 shared state        | Workbench projection 加 coordinator fingerprint test                             | 每个 Client View 保持 client-local                                  |
| 渲染器不接收通用 IPC 或文件系统权限                  | 合约、代理、预加载和渲染器客户端测试                                             | Electron 结构化克隆值得信赖                                         |
| Remote Workspace admission 原子且仅由 Main 拥有      | Remote-admission TLA+、persistence、concurrency 与 forgery test                  | Slice 3.2A 无 terminal presentation 或 Host effect                  |
| 首个 remote terminal 先 commit 再执行 target effect  | Remote-provision TLA+、queue/commit trace、target-routing、retry 与 forgery test | 一个精确 singleton Presentation；projection/input 仍保留            |
| durable intent 先于 effect；settled retry 不重复执行 | `DesktopWorkbenchWriteAhead.tla` 与 reconciler trace                             | 一个抽象 Presentation 与一个可复用 operation identity               |
| 恢复无法创建替换终端                                 | 仅恢复适配器和渲染器恢复跟踪                                                     | 绑定账本和Host对账仍然可信                                          |
| 清理失败保留可重播意图                               | 预写模型和管理删除集成跟踪                                                       | 产品回收 UI 仍超出范围                                              |

更名后的 CAS model 保留现有 transition semantics；本 slice 改变 persisted payload contract，但不改变其
异步 queue 或 revision algorithm。更广泛的 multi-client arbitration、broadcast ordering、reconnect
behavior 与 quiescing progress 继续由独立的 Desktop session coordinator 拥有并建模。
