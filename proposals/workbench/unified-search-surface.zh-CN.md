# 统一搜索界面

> 本文是 [英文 canonical 提案](unified-search-surface.md) 的简体中文评审译本。若两者存在差异，以英文版本为准。

| 字段     | 内容                                                                                                                             |
| -------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 已接受；迁移阶段 1–4 已实现                                                                                                      |
| Owner    | `@orchbee/workbench` 维护者与应用组合层 owner                                                                                    |
| 当前权威 | 只负责未来的 Workbench 产品设计；跨 Workspace 的 Action 契约由 [`docs/frontend/actions.md`](../../docs/frontend/actions.md) 规定 |
| 退出条件 | 实现已接受的行为，把当前公共事实和维护事实迁入 Workbench 所拥有的指南、模型和测试，然后删除本提案及其译本                        |

## 决策

构建一个由 Workbench 拥有、以 Provider 为基础且具有两个全局入口预设的搜索界面：

| 入口                               | 初始筛选 | 初始查询 | 目的                                          |
| ---------------------------------- | -------- | -------- | --------------------------------------------- |
| 可见搜索控件或 `Mod+K`             | All      | 空       | 发现最近实体、常用 Action 和设置              |
| 可见 Actions 入口或 `Mod+Shift+P`  | Actions  | 空       | 不必知道菜单或 Binding，就能发现并运行 Action |
| 经过评审的 Contextual Open Request | 已声明   | 已声明   | 打开一个预设，而不是创建第二套搜索实现        |

通过该界面选择的所有产品操作都调用
[`docs/frontend/actions.md`](../../docs/frontend/actions.md)
定义的 Action Runtime。搜索 Query、Filter、Highlight、Scroll、Nested Picker 和 Provider Loading State 保留为已挂载界面的局部状态。

## 问题

用户需要一个可预期的入口来发现和调用 Orchbee 的能力，而不必事先学会每个菜单位置或快捷键。空查询内容必须通过有用的分组和说明帮助用户理解产品；文本查询内容必须在彼此独立的 Provider 之间以确定性方式排列结果。

当前 Workbench Search 已注册为 `workbench.search.open`；侧边栏与快捷键 Binding 调用同一个
Action，其 Handler 委托给可选的 Host Capability。实现搜索界面的模型与呈现时，已接受设计将用该界面替换这个 Host Capability。

## 范围

| 包含                                                                                  | 首次实现不包含                                                   |
| ------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| All、Agents、Actions 和 Settings 筛选项                                               | 文件搜索                                                         |
| 精心编排的空查询分组和确定性的文本查询排序                                            | 用户编写的宏或类 Shell 命令语言                                  |
| 本地和异步 Search Provider                                                            | AI 生成或不可解释的排序                                          |
| 为需要输入的 Action 提供 Nested Argument Picker                                       | 任意 Action Chaining 或 Result Piping                            |
| Disabled Explanation、Confirmation Handoff、Execution Presentation 和 Result Feedback | 持久化 Query History 或持久化 Action MRU                         |
| 键盘、指针、焦点、屏幕阅读器、本地化和减少动态效果行为                                | 用户可编辑快捷键                                                 |
| 绝不自动提交的 Agent Prefill Fallback                                                 | 自动把本地 Action 暴露给远程或自动化                             |
| 未来通过 Owned Provider 接纳 Workspaces、Automations 和 Skills                        | 在 Stable Searchable Read Model 和真实使用出现前就承诺这些筛选项 |

## 所有权

| 职责                                                         | Owner                                                                               |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Search Surface 产品行为、状态模型和呈现                      | `@orchbee/workbench`                                                                |
| 已挂载 Action Registry 和 Action Execution                   | [`docs/frontend/actions.md`](../../docs/frontend/actions.md) 规定的 Product Runtime |
| Agent Result Data 和 Navigation Action                       | Agent Capability Owner                                                              |
| Stable Settings Destination 和 Navigation Action             | Settings Capability Owner                                                           |
| Provider Composition、Platform Category 和 Host Adapter      | 应用组合层                                                                          |
| Generic Dialog、Command List 和 Listbox Primitive            | 仅当不包含 Orchbee Entity、Message 和 Platform Effect 时，由 `@orchbee/ui` 拥有     |
| Product Descriptor 和两个随产品交付的 Translation            | `@orchbee/i18n`                                                                     |
| Storage、Process、Transport、Routing 和 Remote Job Lifecycle | 它们现有的 Application 或 Runtime Owner                                             |

不要为 Provider、Search Hook、Store 或 Type 创建 Package。每个 Searchable Read Model 保留在其 Semantic Owner 中，并通过 Narrow Public Contract 组合 Provider。

## 筛选项

| 筛选项   | 内容                                                                        |
| -------- | --------------------------------------------------------------------------- |
| All      | 对 Agents、Actions 和 Settings 进行配额平衡后的合并                         |
| Agents   | 由 Agent Owner 提供的最近和匹配 Agent                                       |
| Actions  | 当前 Scope 内可见的 Registered Action                                       |
| Settings | 使用 Bound Setting ID 调用 Settings Navigation Action 的 Stable Destination |

Files 被明确排除。只有 Workspaces、Automations 和 Skills 暴露 Stable Searchable Read Model 后，它们才可以作为 Owned Group 出现在 All 中。只有 Group 的内容量和重复使用情况足以证明 Navigation Cost 合理时，才把它提升为 Persistent Filter。

在同一次打开期间切换 Filter 时保留 Query。Global Entry 每次都从其声明的 Filter 和空 Query 开始。关闭时丢弃 Query、Highlight、Scroll、Provider Error 和 Nested Picker State。

## 空查询内容

| 筛选项   | 初始分组                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------ |
| All      | Recent Agents、Recommended Actions、Mode、Terminal、View 和精选 Settings Destination             |
| Agents   | Recent Agents，之后是 Agent Owner 的 Empty 或 Loading State                                      |
| Actions  | 当前 Application Session 的 Recent Actions，之后是精心编排的 Feature Category                    |
| Settings | 如果 Owned State 已经提供，则先显示 Recently Opened Destination，之后是 Stable Settings Category |

- Group Order 由产品精心编排，并且具有确定性。
- Feature Owner 提供 Category 和 Recommendation Metadata；它们不能为彼此无关的 Owner 指定 Absolute Position。
- Provider 和 Group Quota 防止一个 Source 挤掉 All 中所有其他有用结果。
- 当 Disabled Reason 可以说明如何让 Action 变为可用时，Disabled Action 仍然可见。
- Hidden Action 绝不保留 Result Row 或 Empty Group。
- 首次实现只在内存中保留 Action Recency。持久化需要单独的 Versioned Storage Contract 和隐私评审。

## Provider

每个 Provider 返回 Stable Result Identity、Prepared Presentation 和一个 Scope-bound Action Invocation：

```ts
type SearchResultInvocation =
	| {
			readonly kind: 'ready';
			readonly prepared: PreparedActionInvocation;
	  }
	| {
			readonly kind: 'requires-input';
			readonly unbound: UnboundActionInvocation;
	  };

interface SearchResult {
	readonly id: SearchResultId;
	readonly providerId: SearchProviderId;
	readonly group: SearchResultGroup;
	readonly label: string;
	readonly description?: string;
	readonly icon?: ProductIconName;
	readonly invocation: SearchResultInvocation;
	readonly availability: ActionAvailability;
	readonly score: SearchScore;
}

interface SearchProvider {
	readonly id: SearchProviderId;
	getDefaultSections(context: SearchContext): SearchSection[] | Promise<SearchSection[]>;
	search(
		query: SearchQuery,
		context: SearchContext,
		signal: AbortSignal,
	): SearchResult[] | Promise<SearchResult[]>;
}
```

Provider 返回 Value；绝不修改 Surface Store。Surface 为每个 Request 标记 Open Session ID 和 Query Revision，并且只在两者仍然匹配时接受 Completion。

Action Provider 从 Application Action Scope 请求 Subtree Discovery，使 Workbench、Tab Strip 和 Panel Registration 可以参与，而不创建第二个 Registry。它在每个 Prepared 或 Unbound Invocation 中保留 Runtime 提供的 Scope。它绝不能根据裸 Action ID 重新构造 Invocation，也不能通过之后碰巧处于 Active 状态的 Sibling 重新解析 Result。

两种 Invocation Variant 都是 Runtime-owned Token。Argument Picker 使用 `prepareInvocation(...)` 绑定 Input；Stale 或复制的 Token 不能回退到其他 Registration。

### 异步协调

- 第一帧就渲染本地 Action 和 Settings Result。
- 渐进展示 Asynchronous Result，但不改变 Highlighted Result ID。
- 每个 Revision 使用一个 `AbortController`；Query、Filter、Context 或 Session 变化时取消它。
- 即使 Provider 没有遵守 Cancellation，也忽略 Late Completion。
- 把 Provider Failure 隔离在其 Group 中，其他 Provider 保持可用。
- 限制 Provider Result 数量；Completion Order 绝不改变 Final Ordering。
- 关闭 Surface 时取消 Search Provider Work，但不取消已经接受的 Product Action，除非该 Action 明确把 Execution Ownership 指派给 Surface。

Provider Search 不是 Action。它是由 Open Surface 拥有的 Read Model Work。选择 Result 后，按照 Action Execution 和 Asynchronous Lifecycle 标准调用 Bound Action。

## 匹配与排序

按照以下用户可理解的层级排序：

1. 本地化 Title 完全匹配；
2. 本地化 Title Prefix；
3. 本地化 Title Token Prefix；
4. 显式编写的本地化 Keyword 或 Alias；
5. Active Locale 不同时的 Canonical English Title 或 Alias；以及
6. Title 或 Keyword Fuzzy Match。

在同一个层级内，依次应用 Feature Recommendation、Session Recency、Provider Priority，最后使用 Stable Result ID。Exact Match 始终排在 Recency 之前。首次实现不使用不可解释的 Personalization，也不把 Query 发送给 AI Service。

- 规范化 Unicode、大小写和重复空白，但不修改 Displayed Text。
- 搜索 Localized Title、Approved Translated Keyword、Canonical English Fallback 和 Explicit Stable Alias。
- 中文搜索使用中文 Title 和 Authored Alias。在评审 Quality、Package Size 和 Terminology Behavior 前，不生成拼音索引。
- 绝不索引 Hidden Action、Raw Error、Terminal Content、Agent Transcript、Secret 或无关 User Data。
- Provider Quota 应用于 All；Explicit Filter 可以显示该 Provider 完整但有界的 Result Set。

## 嵌套 Action 输入

选择一个仍缺少必需 Input 的 Action 时，在同一个 Surface 中打开 Nested Picker。

- Picker 拥有自己的 Prompt、Query、Result、Validation、Loading、Empty、Error 和 Cancellation State。
- Back 恢复 Parent Query、Filter、Scroll Position 和 Highlighted Result。
- Final Selection 把收集的 Input 和 Original Unbound Token 传给 `prepareInvocation(...)`，然后执行得到的 Prepared Token。
- Picker 可以使用一个由 Action Owner 准入的 Provider；不能不受限制地访问所有 Search Provider。
- 首次实现没有 Generic Pipeline 或 Action Composition Language。

## Agent Fallback

当 All 或 Agents Query 没有 Direct Match 时，Agent Owner 可以提供最后一个
`Ask Agent about "{query}"` Result。

- 选择后打开 Agent Composer，并预填 Query。
- 未经另一次 Explicit Submit，它绝不发送 Message、启动 Process 或产生成本。
- Query 为空、Context 不受支持或 Surface 敏感时省略。
- `@orchbee/i18n` 拥有两个 Locale 中的完整 Label 和 Accessible Name。

## 交互模型

| 状态               | 必需数据                                                                                   |
| ------------------ | ------------------------------------------------------------------------------------------ |
| `closed`           | Semantic Focus Return Target                                                               |
| `browsing`         | Session、Filter、Query Revision、Query、Section、Provider State、Scroll 和 Highlighted ID  |
| `picking-argument` | Parent Snapshot、Action ID、Picker Step、Query Revision、Section、Scroll 和 Highlighted ID |
| `confirming`       | 观察由 Feature 拥有的 Confirmation 的 Bound Action Execution                               |
| `observing-action` | Execution ID，以及 Policy 让 Surface 保持打开的 Action 的 Surface Presentation             |

| 事件                                  | 结果                                                                         |
| ------------------------------------- | ---------------------------------------------------------------------------- |
| Open                                  | 捕获 Semantic Focus，创建 Session，应用 Preset，并聚焦 Query Input           |
| Query Changed                         | 增加 Revision，取消旧 Provider Work，保留有效 Highlight，并请求当前 Provider |
| Provider Completed                    | 只接受 Current Session 和 Revision，然后确定性合并                           |
| Move Selection                        | 通过 Stable ID 选择一个 Visible Result；Disabled Result 仍然可以检查         |
| Invoke Enabled Result                 | 进入 Argument Picker 或调用其 Action                                         |
| Invoke Disabled Result                | 保留 State 并暴露 Localized Reason                                           |
| Registry 或 Context Changed           | 重新投影 Action；只有同一个 Result 仍然可见时才保留 Highlight                |
| Nested State 中的 Escape              | 取消该 Step 并恢复其 Parent Snapshot                                         |
| Browsing 中的 Escape 或 Outside Click | 关闭且不调用，并恢复 Focus                                                   |
| Surface Unmounted                     | 取消 Provider Work、取消订阅 Action Execution 并释放 Subscription            |

### 不变量

- Highlighted Result 必须能在 Current Session、Revision、Filter 和 Visible Result Set 中解析。
- Stale Provider Completion 绝不进入 Observable Projection。
- Provider Completion Order 不改变 Final Grouped Order。
- Disabled 或 Hidden Action 绝不执行。
- 一个 Accepted Result Invocation 最多到达一个 Action Execution。
- Invocation 前取消或关闭时不提交 Action。
- Focus 解析到 Query、Owned Nested Control 或 Valid Semantic Return Target。

## Action Execution 呈现

Action Execution 遵循
[`docs/frontend/actions.md`](../../docs/frontend/actions.md#execution-contract)。
Surface 只决定如何呈现该 Execution：

| 策略              | Surface 行为                                                                        |
| ----------------- | ----------------------------------------------------------------------------------- |
| Default           | Execution 开始时关闭；Result 通过所属 Action 或 Application Result Channel 展示     |
| `remain-open`     | 观察同一个 Execution ID，阻止 Duplicate Invocation，并展示 Owned Progress 或 Result |
| Background Action | 关闭；Application-owned Activity 或 Notification Surface 继续观察                   |
| External Job      | 按照声明关闭或导航；External Owner 通过 Stable Job ID 保持权威                      |

关闭 Surface 绝不能隐式把 Running Action 变成 Cancelled。Late Completion 不重新打开 Surface、不恢复 Stale Focus，也不从后续用户工作导航离开。

## 界面

桌面呈现是一个位于 Viewport 上方区域中央的 Modal Overlay，使用经过批准的 Workbench 和 Design System Geometry。Result Group 滚动时，Input、Filter 和 Footer 保持可见。

```text
┌──────────────────────────────────────────────────────────────┐
│ 搜索 Agent、Action 和设置…                                  │
├──────────────────────────────────────────────────────────────┤
│ 全部   Agent   Action   设置                                 │
├──────────────────────────────────────────────────────────────┤
│ 最近使用的 Agent                                             │
│  图标  Agent 标题                  Workspace          时间   │
│                                                              │
│ Terminal                                                     │
│  图标  向右分割 Panel                            快捷键      │
│  图标  关闭 Terminal       禁用原因                          │
│                                                              │
│ 设置                                                         │
│  图标  外观                                      设置 ›      │
├──────────────────────────────────────────────────────────────┤
│ ↑↓ 选择       Enter 打开或运行       Esc 返回或关闭          │
└──────────────────────────────────────────────────────────────┘
```

| 区域          | 契约                                                                                             |
| ------------- | ------------------------------------------------------------------------------------------------ |
| Input         | 初始 Focus，Browsing 期间持续存在，不能只在 Placeholder 中放置必需说明                           |
| Filter        | 具有 Visible 和 Programmatic Selection 的 Tablist                                                |
| Group Heading | 安静的 Semantic Category，与 Result Row 之间有足够间隔                                           |
| Result Row    | Product Icon、Primary Label、可选 Description 或 Context，以及尾部 Resolved Shortcut 或 Metadata |
| Footer        | 只展示当前 State 中有效的 Registered Interaction；它是指导，绝不是唯一说明来源                   |
| Scroll Region | 只包含 Result Group；Input、Filter 和 Footer 保持可见                                            |

- 使用
  [`docs/frontend/visual-foundations.md`](../../docs/frontend/visual-foundations.md)
  中的 Typography、Hierarchy、Spacing、Radius、Control、Icon、Border、Surface、Focus 和 Shadow Role。
- 保持一列 Icon 和一列尾部 Metadata。
- 优先使用单行 Title；必需的 Disabled Reason、Error 和 Instruction 绝不截断。
- Keyboard Highlight 优先于 Hover；Focus 独立保持可见。
- 避免移动 Selectable Row 的动画；在可行时保留 Highlighted Visual Position。
- Narrow Viewport 保留相同 Semantic Region。后续 Mobile Sheet 是 Presentation Adapter，不是另一套 Search Model。

## 空状态、加载与失败状态

| 条件                               | 呈现                                                                 |
| ---------------------------------- | -------------------------------------------------------------------- |
| Provider Pending 且有 Local Result | 立即展示 Local Group，只在 Pending Group 中展示 Loading              |
| Provider Pending 且无 Local Result | 展示 Localized Loading State，而不是 Blank Panel                     |
| No Match                           | 命名被搜索的 Filter，并在可用时提供经过评审的 Agent Prefill Fallback |
| 一个 Provider Failed               | 在该 Group 中展示 Localized Retry，并保持其他 Group 可用             |
| 所有 Provider Failed               | 展示一个 Focused Error State，并提供 Retry 与 Close Action           |
| Action Disabled                    | 保持可发现性并暴露 Localized Reason                                  |
| Remain-open Action Running         | 阻止 Duplicate Invocation，只展示由该 Action 拥有的 Progress         |
| Highlight 周围 Result 变化         | 保留同一个 Result ID；否则以确定性方式选择最近的 Enabled Result      |

## 焦点与无障碍

- 使用具有一个 Accessible Name 的 Modal Dialog。
- 把 Query 实现为控制 Listbox 的 Combobox；Provider Heading 是 Associated Group，Result 是 Option。
- 把 Filter 实现为 Named Tablist。不要混合 Tab、Option 和 Button 语义。
- DOM Focus 留在 Query 中，并使用完整的 Active Descendant Pattern 移动 Visual Selection。
- Up 和 Down 移动 Result；Enter 调用 Enabled Result；Escape 返回一个 Nested Level 或关闭 Browsing；Composition Enter 绝不调用。
- Tab 遵循已记录的 Focus Order，不是未记录的 Filter Cycle Command。
- 播报 Result Count 变化、Provider Failure、Disabled Reason、Nested Step 和 Observed Action Result，但不重复整个 List。
- Outside Click Dismissal 绝不 Click Through。
- 恢复仍然有效的 Semantic Focus Target 或所属 Surface 的 Fallback。
- Reduced Motion 移除非必要的 Entry、Exit 和 Row Reposition Animation。
- 在 Conflict Analysis 证明不会覆盖 Navigation 或 Browser Behavior 前，暂不决定 Filter Switching Shortcut；Footer 绝不展示未注册 Chord。

## 本地化与隐私

- 每个 Title、Description、Category、Keyword、Placeholder、Group、Empty State、Error、Disabled Reason、Confirmation、Accessible Name、Progress Label 和 Footer Instruction 都通过 `@orchbee/i18n` 提供 `en-US` 与 `zh-CN`。
- Stable ID、User Value、Path、Command、Terminal Output 和 External Agent Content 不翻译。
- Platform Formatter 拥有 Shortcut Glyph 和 Label。
- 首次实现只在已挂载内存中保留 Query Text。
- 不向 Telemetry 发送 Query Text、Result Label、Agent Name、Terminal Content、Bound User Input 或 Confirmation Copy。
- Search Result 绝不在 Highlight、Hover、Preview 或 Prefetch 时执行。

## 性能

- 在经过测量、具有代表性的受支持桌面环境中，使用 Focused Input 和 Local Action、Settings Result 在 `100 ms` 内打开。
- 输入时无需等待 Asynchronous Provider，就更新 Input 和 Local Result。
- Matching、Ranking、Grouping 和 State Transition 保持 Pure、Bounded 且不依赖 React。
- Static Action Index 每个 Locale 或 Registry Revision 只构建一次，而不是每个 Result Row Render 都构建。
- 只有测量证明 Bounded Result 无法达到 Interaction Target 后才使用 Virtualization。
- 性能证据记录 Dataset、Environment 和 Percentile。

## 验证

| 单元或边界              | 必需证据                                                                                             |
| ----------------------- | ---------------------------------------------------------------------------------------------------- |
| Search Reducer          | 每个 Transition、Stale Completion、Selection Stability、Nested Restoration、Close 和 Invariant       |
| Ranking                 | Locale Term、Ranking Tier、Deterministic Tie、Quota 和 Provider Order Independence                   |
| Provider Coordinator    | Cancellation、Late Completion、Partial Failure、Retry、Registry Change 和 Bounded Result             |
| Action Integration      | Visible Result 到达与 Button、Menu 和 Shortcut Path 相同的 Operation                                 |
| Focus 与 Accessibility  | Open、Query、Filter、Nested Picker、Close、Late Provider 与 Action Completion、Zoom 和 Screen Reader |
| Localization 与 Privacy | 两个 Locale、Wrapping、Disabled Reason、Long Metadata、Redaction 和禁止 Automatic Agent Submission   |

Async Provider Ordering 和 Cancellation 要求在生产集成前建立 TLA+ 模型。建模 Session、Query Revision、Provider Request 与 Completion、Cancellation、Visible Revision、Selection、Close 和 Reopen。

| Claim                                                     | 证据                                                            |
| --------------------------------------------------------- | --------------------------------------------------------------- |
| Stale Completion 绝不进入 Newer Query 或 Reopened Session | TLA+ Safety Property 和 Deterministic Reordered Completion Test |
| Selection 可以在 Current Visible Result Set 中解析        | Reducer Invariant 和 Stateful Property Test                     |
| Final Ranking 与 Provider Completion Order 无关           | Pure Ranking Property Test                                      |
| Close 或 Nested Cancellation 不提交 Action Invocation     | Reducer Test 和 Deterministic Browser Trace                     |
| Late Provider 或 Action Completion 不抢夺 Focus           | 具有 Semantic Focus Checkpoint 的 Deterministic Browser Trace   |

TLA+ Progress 可以假设未取消的 Current Provider 最终完成。它不声明 Network Availability、Provider Correctness 或 Action Domain Correctness。

Storybook 覆盖两个 Entry Mode、四个 Filter、Empty 和 Typed Query、Local 与 Async State、Partial 和 Total Failure、Enabled 和 Disabled Action、Nested Input、Confirmation Handoff、Remain-open Execution、两个 Locale、Narrow Layout、Zoom、High Contrast、Dark Theme 和 Reduced Motion。

## 迁移

阶段 1–4 已实现。阶段 5–8 仍是由本提案约束的未来工作。

| 阶段 | 变更                                                                                                            | 完成证据                                                                         |
| ---- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 1    | 实现并测试 Frontend Action Standard 要求的 Action Runtime                                                       | Runtime、Execution、Async Lifecycle、Concurrency、Localization 和 Scope Evidence |
| 2    | 注册 Current Workbench Action，并通过一条 Domain Commit Path 适配 Existing Callback                             | Stable ID、Descriptor、Availability 和 Compatibility Test                        |
| 3    | 让 Visible Control 和 Menu 通过 Runtime 调用                                                                    | Existing Interaction Test 加 Same-operation Contract                             |
| 4    | 使用 Enumerable Shortcut Resolver 替换彼此独立的 Workbench Keydown Branch                                       | Shortcut Matrix、Conflict Test 和 Old Listener Removal                           |
| 5    | 实现 Search State Model、Surface、Local Actions 与 Settings Provider、Accessibility 和 Story                    | Deterministic Core、TLA+、Component、Storybook 和 Browser Evidence               |
| 6    | 通过 Agent-owned Public Contract 添加 Agents Provider 和 No-auto-submit Fallback                                | Cancellation、Ranking、Privacy、Empty State 和 Fallback Test                     |
| 7    | 在应用组合边界适配 Active Terminal Instance Action                                                              | Instance Disposal、Active Scope、Localized Presentation 和 Terminal Test         |
| 8    | 删除 Duplicate Callback 和 Metadata；更新 Workbench Current Guide、API、Project Contract 和 Test；删除 Proposal | 不存在 Competing Path、Package Check 和根 `pnpm verify`                          |

Package 局部 `TerminalActionRegistry` 可以保留 Per-instance Terminal Semantic。Composition Adapter 映射 Compatible Definition 和 Outcome，而不让 `@orchbee/terminal` 依赖 Workbench Presentation 或 Localization。

## 后果

- 新用户获得 Categorized Discovery、Description、Disabled Reason 和 Shortcut Hint。
- Control、Menu、Shortcut 和 Result 共享一条 Action Execution Path。
- Async Search 只有一套 Cancellation 与 Stale Result Model。
- Feature Owner 必须暴露 Stable Read Model、Action Invocation 和 Localized Descriptor。
- Provider Order、Focus、Accessibility 和 Async Completion 需要持续维护 Deterministic 与 Formal Evidence。
- Product-specific Behavior 在实现前保留在这里；已接受的 Cross-workspace Action Rule 只保留在
  [`docs/frontend/actions.md`](../../docs/frontend/actions.md)。
