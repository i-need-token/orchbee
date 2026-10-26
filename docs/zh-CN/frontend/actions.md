# 前端动作

## 语义边界

动作是具名、可由用户调用的产品意图，其含义不依赖某一个控件或输入路径。Button、Menu、Context Menu、键盘快捷键、Command 界面和经过批准的自动化都调用同一个动作，而不是实现彼此平行的状态修改路径。

GUI 与 TUI 的操作含义、目标范围和结果遵循 [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md)。
本文档拥有前端动作运行时；原生调用通过既有所有者映射到相同领域契约。

| 行为                                                 | 表示方式                                                                                         |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 具有语义结果的离散产品意图                           | 动作                                                                                             |
| 超过一个调用路径需要的操作                           | 动作                                                                                             |
| 需要稳定可用性或可观测性的操作                       | 动作                                                                                             |
| 局部 Input、Hover、Disclosure 或 List Highlight 状态 | Component 或 Feature 交互状态                                                                    |
| Pointer 移动、Drag Preview 或连续 Resize 输入        | Gesture Event 和可逆 Feature Projection                                                          |
| 由另一条调用路径使用的手势提交结果                   | 动作                                                                                             |
| Process、Network、Storage 或 Resource 更新           | 由对应 Resource Lifecycle 拥有的 Event；动作可以发起该 Event                                     |
| 领域转换返回的 Effect Command                        | [`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md) 下的 Package 局部 Domain Command |

不要把每一个 Callback 都包装成动作。Generic UI Primitive 继续暴露类型化 Callback；其 Product Semantic Consumer 决定某个 Callback 是否调用动作。

当对象行暴露省略号动作菜单时，省略号控件与该行的 `contextmenu` 手势使用同一份有序菜单
模型。两种投影保持相同的菜单项、顺序、分组、图标、可用性、破坏性处理和调用目标。右键或
长按不会静默激活、选择或展开对象，除非该状态变化本身就是已接受的动作。仅在所属触发区域
内且产品 Context Menu 可用时，才抑制平台原生 Context Menu。展示与交互遵循
[`menus.md`](menus.md)。

## 所有权

| 职责                                                       | Owner                            |
| ---------------------------------------------------------- | -------------------------------- |
| 稳定动作身份、Descriptor 和 Input Contract                 | 拥有语义结果的 Feature           |
| Availability、Confirmation、Handler 和 Result Mapping      | 拥有语义结果的 Feature           |
| Authoritative State 和 Domain Invariant                    | 所属 Domain Capability           |
| Registry、Scope Resolution 和 Execution Lifecycle          | 已挂载的 Product Runtime         |
| Platform、Process、Storage、Transport 和 Navigation Effect | Application 或现有 Runtime Owner |
| Product Message 和 Translation                             | `@orchbee/i18n`                  |
| 局部 Visual 和 Interaction State                           | 最窄的已挂载 Component Owner     |

Runtime 负责调度和应用横切策略；它绝不能变成 Feature Logic 的全局实现，也不能成为 Authoritative Product State Store。Handler 调用其 Feature 所拥有的 Public Operation。跨 Feature 行为属于显式 Composition Workflow。

每个已挂载的 Application Composition 构造一个 Runtime，并把它或一个更窄的 Invocation Capability 注入 Product Package。Package 绝不导入 Process-wide Action Singleton，也不通过 Host Detection 选择 Platform Adapter。

## 身份与注册

- 动作 ID 使用 `{owner}.{resource}.{verb}` 格式和稳定的点分 Segment。
- 绝不从本地化文案、Component Name、Menu Location 或 Shortcut 推导 ID。
- 一个 ID 标识一个语义结果；Entity、Target 和 Option Value 是类型化 Input。
- 重命名 ID 时，同时迁移所有 Binding、Menu Contribution、Command Result、Stored Reference、Diagnostic、Test 和 Automation Contract。
- Compatibility Alias 必须显式、临时且不可枚举，并随其最后一个 Consumer 一起删除。
- 注册返回一个可以精确释放的 Disposable，并随其 Owner 移除 Registration。

Registration 把动作绑定到 Application、Product、Feature Surface 或 Temporary Interaction Scope。多个已挂载实例可以在不同 Instance Scope 中绑定同一个动作 ID；同一 Scope 内的重复 ID 必须被拒绝。从最窄的 Active Scope 向外解析。Registration Order 绝不是优先级。

Registry 必须可枚举，并在 Definition、Binding 或 Availability Dependency 发生变化时通知 Consumer。枚举只暴露对请求 Context 可见的动作，不授予 Automation 或 Remote Authority。

### Discovery 与 Prepared Invocation

Discovery 返回 Invocation-shaped Value，而不只是 Presentation Metadata：

- `ready` 包含一个由 Runtime 拥有的 `PreparedActionInvocation` Token，它绑定 Action ID、Validator 生成的 Canonical Input，以及解析该 Action 的精确 Registration Identity。
- `requires-input` 包含一个 `UnboundActionInvocation`，其中保存 Action ID、具名 Input Contract 和精确 Registration Identity。Invocation Surface 把收集的 Input 传给 `prepareInvocation(...)`；只有 Runtime 可以验证该 Input 并创建 Prepared Token。

Narrow Discovery 从 Active Scope 向外解析，采用与 Execution 相同的 Shadowing Semantic。Global Command Surface 可以从其 Application Scope 显式请求 Subtree Discovery，以组合由 Sibling Feature 拥有的 Registration。Subtree Result 始终绑定到其 Registration Scope；Consumer 绝不能通过任意 Current Sibling 重新解析它。如果多个 Descendant Instance 提供同一个 Action ID，则 Result Identity 包含 Scope，Surface 要么区分这些 Instance，要么要求 Owner 使用类型化 Target Input 暴露一个更宽的 Action。

Required Input 尚不可用的 Action，必须通过一个 Pure Synchronous Discovery-availability Projection 显式选择加入 Pre-input Enumeration。没有该 Projection 时，Input 解析失败会省略该 Action；Runtime 绝不假设 Unresolved Action 可见。Hidden 和 Automation-denied Registration 仍然不会出现。选择 `ready` Result 时执行其 Stored Prepared Invocation，使 Action ID、Input、Scope、Source 和 Target 不会在 Presentation 与 Invocation 之间漂移。Prepared 和 Unbound Token 是 Runtime Capability，而不是 Serializable Request。复制其 Public Field 不能创建有效 Token。Bound Registration 被释放或替换后，Token 立即失效；Preparation 不返回 Result，Prepared Execution 返回类型化 Stale-invocation Block，绝不能解析到 Ancestor 或 Replacement Registration。

## Definition 契约

动作 Definition 提供或引用：

| 字段                     | 契约                                                                         |
| ------------------------ | ---------------------------------------------------------------------------- |
| `id`                     | 稳定语义身份                                                                 |
| `title`                  | Product Message Descriptor                                                   |
| `description`            | 可选 Product Message Descriptor，用于补充影响决策的 Context                  |
| `category`               | 稳定语义分组 Metadata，绝不作为不相关 Owner 之间的绝对位置                   |
| `icon`                   | 可选、经过批准的 Product Icon Name                                           |
| `keywords`               | 可选的本地化 Descriptor 或稳定且不可见的 Alias                               |
| `input`                  | Named Type，以及返回 Owner-owned Canonical Snapshot 的 Boundary Validator    |
| `scope`                  | Registration 和 Resolution Scope                                             |
| `availability`           | 对一份 Read-only Context Snapshot 的 Pure Synchronous Projection             |
| `discovery availability` | 用于未解析 Dynamic Input 的可选 Pure Pre-input Visibility Projection         |
| `confirmation`           | 针对已解析 Input 和 Target、由 Feature 拥有的可选 Confirmation Model         |
| `handler`                | 由 Feature 拥有的 Synchronous 或 Asynchronous Execution                      |
| `bindings`               | 在 Feature 职责明确的 `*-shortcuts.ts` Module 中只拥有一次的 Default Binding |
| `execution policy`       | 适用时的 Surface Lifetime、Concurrency、Progress 和 Cancellation Decision    |

使用一份由 Owner 定义的 Action Catalog，以及一份同时负责 Registration 和 Invocation 的 Catalog-bound Adapter，让 Trusted Feature Code 保留每个 Action ID、Definition Input、Handler Result、Invocation Input 和 Execution Result 之间的编译期关系。先通过 Untyped Runtime 注册、之后只包装 Invocation，不满足该契约。Dynamic Command、Automation、IPC、Network 或 Persisted Input 在所属 Validator 于第一个可信边界解析之前始终保持为 `unknown`。Validator 必须复制 Execution 所保留的每一个由 Caller 拥有的 Mutable Collection 或 Nested Value；Canonical Snapshot 绝不能引用由 Invocation Surface 拥有的 Mutable Input。

Action Descriptor 携带 Message Descriptor，而不是已格式化字符串。Menu、Button Tooltip、Command Result、Shortcut Reference、Progress Presentation 和 Confirmation 按照
[`localization.md`](localization.md)
通过 Effective Locale 格式化同一组 Owned Descriptor。

## Context 与目标

Invocation Context 只包含贯穿 Execution 的事实，不能包含不受限制的 Service 或 Feature Private State：

- Invocation Source；
- 一份 Read-only Scope 和 Focus Snapshot；
- 一个 `AbortSignal`；
- 适用时的 Semantic Target Handle；以及
- 所属 Workflow 明确允许的 Narrow Capability。

使用 Stable Resource ID 等 Semantic Target Handle；绝不把 DOM Node 保留为 Action Target。Owner 声明 Invocation 使用调用时捕获的 Target，还是在 Execution 前立即解析 Current Target。

在 Argument Collection、Asynchronous Preparation 和 Confirmation 之后重新解析 Target。缺失或陈旧 Target 产生类型化 Blocked Result，绝不能回退到无关的 Active Item。Handler 继续保护 Domain Precondition，因为状态可能在 Runtime 上一次检查后发生变化。

## 可用性与确认

| 可用性     | 含义                                         | 展示                                                      |
| ---------- | -------------------------------------------- | --------------------------------------------------------- |
| `enabled`  | 动作可以针对当前 Input、Scope 和 Target 运行 | 可见且可调用                                              |
| `disabled` | 动作相关，但当前暂时不能运行                 | 可见、不可调用，并附带本地化原因                          |
| `hidden`   | 动作不相关、未授权、不受支持或不应暴露       | 从 Menu、Command Surface、Reference 和 Enumeration 中省略 |

- Availability 基于一份 Context Snapshot 进行同步纯计算。
- 在 Presentation、Shortcut Resolution 和即将调用 Handler 前重新计算。
- 按照
  [`keyboard-shortcuts.md`](keyboard-shortcuts.md#上下文与可用性)
  保持 Shortcut `when` Applicability 与 Action Availability 相互独立。
- Checked State 表示 Semantic Toggle Value，不表示某个界面中的 Visual Selection。
- Asynchronous Precondition 进入显式 Preparation Workflow；不能让 Shortcut Routing 等待 Network 或 Process Work。

Feature 决定已解析 Input 和 Target 是否需要 Confirmation，并提供本地化 Title、Explanation、Confirmation Label 和 Risk。Runtime 以一致方式展示该 Model，在接受后重新验证，并且在接受前不调用 Handler。Confirmation 绝不能替代 Authorization、Target Validation 或 Domain Guard。

## 结果

每次 Invocation 都恰好以一种 Result Variant 完成一次：

| 结果          | 含义                                                                      |
| ------------- | ------------------------------------------------------------------------- |
| `completed`   | 语义操作已经提交，可以包含类型化值                                        |
| `unchanged`   | 请求有效，并且有意不产生变化                                              |
| `blocked`     | 已知 Guard、陈旧 Target、不可用 Capability 或 Permission 阻止了 Execution |
| `cancelled`   | Caller 或所属 Lifecycle 在 Commit 前取消                                  |
| `failed`      | 发生了预期的类型化 Failure 或经过映射的 Unexpected Failure                |
| `unsupported` | 没有 Active Registration 可以处理该 Invocation                            |

Expected Failure 携带 Stable Code 和 Structured Value。Product Surface 把它们映射为本地化 Message。Raw Exception 可以进入 Owned Diagnostic，但绝不能成为 Product Copy。Runtime 把 Handler Exception 映射为 `failed` 并 resolve Completion；Caller 不会收到未分类的 Rejected Promise。

记录每个修改状态的 Asynchronous Action 的 Commit Point。Commit 前的 Cancellation 不产生 Authoritative Change，除非 Owner 明确建模了 Reversible Optimistic State。Commit 后的 Cancellation 不能把 Completed Operation 重新标记为 Cancelled。

## Execution 契约

Runtime 立即返回 Execution Handle。Handler 可以返回 Immediate Result 或 Promise；Caller 始终观察同一套 Execution Model。

```ts
interface ActionHandler<Input, Output> {
	execute(
		context: ActionExecutionContext,
		input: Input,
	): ActionResult<Output> | Promise<ActionResult<Output>>;
}

interface ActionExecution<Output> {
	readonly executionId: ActionExecutionId;
	readonly completion: Promise<ActionResult<Output>>;

	getSnapshot(): ActionExecutionSnapshot<Output>;
	subscribe(listener: () => void): ActionDisposable;
	cancel(): void;
}
```

Immediate Completed Synchronous Handler 可以在 `ActionRuntime.execute(...)` 返回前产生 Terminal Snapshot。Consumer 先读取 `getSnapshot()` 再订阅，并使用 `completion` 获得 Exactly-once Terminal Result；只靠 Event Delivery 绝不能成为事实来源。

### 状态

| 状态                    | 含义                                                                        |
| ----------------------- | --------------------------------------------------------------------------- |
| `resolving`             | 解析 Registration，并验证 Input、Target 和 Availability                     |
| `awaiting-confirmation` | 等待由 Feature 拥有的 Confirmation                                          |
| `running`               | Handler Work 正在进行                                                       |
| `cancelling`            | 已请求 Cancellation，但 Handler 尚未到达 Terminal State                     |
| Terminal Result         | `completed`、`unchanged`、`blocked`、`cancelled`、`failed` 或 `unsupported` |

- State Transition 单调变化，一个 Execution 只到达一个 Terminal Result。
- 一个 Accepted Invocation 最多到达一个 Handler。
- `cancel()` 是 Cooperative Request，不是 Work 已经停止的证明。
- 已越过 Commit Point 的 Handler 可以在收到 Cancellation Request 后完成。
- Runtime 绝不隐式 Retry、Timeout 或把 Failure 转换为 Success。
- Owner Workflow 定义 Retry、Recovery、Timeout、Rollback 和 Idempotency。
- 禁止 Unmanaged Fire-and-forget Work；每个 Execution 都有 Lifecycle Owner 和 Observable Terminal Outcome，或者返回由外部拥有的 Stable Job Identity。

## 异步生命周期

按照 Ownership 与 Presentation 对 Asynchronous Work 分类：

| 模式         | 契约                                                                                               |
| ------------ | -------------------------------------------------------------------------------------------------- |
| Foreground   | Invocation Product Surface 拥有 Pending Presentation，并保持可用直到获得所需结果                   |
| Background   | Invocation Surface 可以关闭；Application-owned Activity 或 Notification Surface 继续观察 Execution |
| External Job | Process 或 Service 拥有 Durable Work；Action 返回 Stable Job ID，Application 对其进行观察          |

关闭或卸载 Invocation Control 时只取消其 Presentation Subscription；除非动作明确把 Execution Ownership 指派给该 Surface，否则不取消已接受的 Business Work。释放 Action Runtime 时，请求取消 Runtime-owned Execution。External Job 遵循其 Process 或 Service Lifecycle，绝不能仅仅因为一个 In-memory Promise 仍处于 Pending 就把它表示为 Durable。

Progress 是 Structured State，而不是任意 Handler 发出的 Product String：

```ts
interface ActionProgress {
	readonly phase: string;
	readonly completed?: number;
	readonly total?: number;
	readonly message?: MessageDescriptor;
}
```

- Indeterminate Progress 省略 `completed` 和 `total`。
- 在一个 Phase 内保持 Total 稳定，否则发布新 Phase。
- 在 Presentation 处格式化 Message Descriptor；绝不把 Raw Exception 或 User-sensitive Input 放入 Progress。
- 限制并合并 High-frequency Progress，避免 Rendering 和 Telemetry 反过来成为 Workload。
- Button、Menu Follow-up、Command Surface 和 Activity View 观察同一个 `executionId`；它们不拥有彼此竞争的 Pending State。

Late Completion 绝不从后续用户工作中抢夺 Focus 或 Navigation。只有 Execution 仍拥有相关 Interaction Token 时才恢复 Focus 或导航；否则通过 Application 的 Nonintrusive Result Channel 报告。

## 并发

每个 Asynchronous Action 都要定义如何处理具有相同 Semantic Concurrency Key 的另一次 Invocation：

| 策略               | 行为                                                 |
| ------------------ | ---------------------------------------------------- |
| `parallel`         | 启动另一个独立 Execution                             |
| `reject-duplicate` | Matching Execution 处于 Active 时返回 Blocked Result |
| `replace-latest`   | 请求取消 Previous Execution 并启动 Latest            |
| `serialize`        | 以确定性顺序对 Matching Execution 排队               |

Owner 使用最少且相关的 Action ID、Target 和 Validated Input 推导 Key。不要使用 Global Default、Object Identity、本地化文本、Registration Order 或偶然的 Component Instance 作为 Key。

定义 Cancellation 延迟、Replaced Execution 越过 Commit、Queued Target 变陈旧或 Completion 乱序到达时的行为。Caller Deadline 不会取消 Shared Work，除非 Caller 明确请求 Cancellation 并拥有该 Authority。

## 调用界面与自动化

- Product Control 准备类型化 Invocation 并调用 Runtime；绝不实现第二条状态修改路径。
- Presentational Component 不检查 Registry、不解析 Target，也不拥有 Domain Transition。
- Command 和 Search Surface 投影 Discovery Result；选择 Result 时执行同一个 Prepared Invocation，Unresolved Result 则进入其声明的 Input Collection Flow。
- 不同 Surface 可以用不同方式展示同一个 Execution，但它们观察同一个 Execution Handle 和 Terminal Result。
- Hidden Action 不出现在 User 和 Automation Enumeration 中。
- Local Enumeration 不会让动作自动获得 Remote Invocability。

Automation 必须显式选择加入，并具有 Stable Serializable Input Schema、Authorization Policy、Noninteractive Confirmation Policy、Compatibility Lifecycle 和 Safe Output Contract。Unsafe、Interactive-only 或 Context-ambiguous Action 即使已经本地注册，也保持不可用。

## 隐私与可观测性

Execution Observation 可以包含 Action ID、Invocation Source、Result Kind、Stable Failure Code、Duration Bucket 和不含用户数据的 Target Type。默认不记录 Input、Result Label、Target Name、Raw Error、Terminal Content、Transcript、Secret 或 Confirmation Copy。

为每个 Execution 提供 Correlation ID，用于 Diagnostic 和 External Job Handoff。除非 Feature 拥有经过评审的 Safe Projection，否则 Diagnostic 必须移除 Input。Progress 和 Retry 不自动产生 Telemetry Event。

## 验证

| 边界                    | 必需证据                                                                                              |
| ----------------------- | ----------------------------------------------------------------------------------------------------- |
| Definition 与 Registry  | Identity、Duplicate Rejection、Scoped Resolution、Enumeration、Disposal 和 Change Notification        |
| Availability            | Enabled、Disabled Reason、Hidden、Checked、Stale Target、Recheck 和 Shortcut Fallthrough              |
| Execution               | Immediate 与 Promised Handler、Exactly-once Result、Exception Mapping、Confirmation 和 Unsupported    |
| Cancellation            | Prepare 前、Confirmation 中、Commit 前、Commit 后、Delayed Cancellation 和 Owner Disposal             |
| Concurrency             | 每个 Declared Policy、Semantic Key、Stale Queued Target、Replacement 和 Out-of-order Completion       |
| Progress                | Initial Snapshot、Determinate 与 Indeterminate Phase、Coalescing、Late Subscription 和 Terminal State |
| Invocation Path         | Visible Control、适用 Menu、Shortcut、Command Result 和 Approved Automation 到达一个 Operation        |
| Focus 与 Accessibility  | Pending、Cancellation、Failure、Surface Close、Late Completion 和禁止 Late Focus Theft                |
| Localization 与 Privacy | 两个 Shipped Locale、Formatted Descriptor、Structured Failure 和 Redacted Observation                 |

Action 流程使用受控完成顺序、取消、过期结果、失败及清理测试。仅当独立排序参与者带来的具体所有权/提交/进展风险无法由实现轨迹充分覆盖时，才按 [`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md) 采用共享 TLA 协议。异步工作或取消分支本身不要求功能模型。

Browser Verification 通过每条可见路径调用一个代表性动作，并比较 Owned Semantic Result、Pending Behavior、Focus 和 Error Presentation，而不只是比较渲染像素。

<!-- documentation-pairing: english-sha256=986f10b0b2d4b00e026f8ab9a60a1acf004cfe975c38ee0677974327011ff7e7 zh-cn-sha256=8f80d84497a4689ef339ef4ac8c885222e00c46c48178510e8f40cd175ea4992 -->
