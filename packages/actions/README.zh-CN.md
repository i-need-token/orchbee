# @orchbee/actions

私有包入口只导出仓库内有消费者的符号，Knip 的 `includeEntryExports` 检查该边界。内部契约、生成资产与独立一致性证据按其所有权保留；它们的存在不自动扩大公共入口。新增客户端应按已准入用途添加所需导出。

Orchbee 产品语义 Action 的框架无关运行时。

该包负责作用域内注册与解析、发现、可用性复查、可观察的同步/异步执行、确认、协作式取消、进度和显式并发策略。功能包保留其权威状态，并注册调用既有领域操作的处理程序。应用为每个已挂载组合创建一个 `ActionRuntime`，并注入该运行时或更窄的调用能力。

## 使用

```ts
import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionSignature,
} from '@orchbee/actions';

interface ApplicationActionCatalog {
	readonly 'workbench.sidebar.toggle': ActionSignature<void>;
}

const runtime = new ActionRuntime();
const applicationScope = runtime.createScope('application');
const actions = createTypedActionRuntime<ApplicationActionCatalog>(runtime);

const registration = actions.register(
	applicationScope,
	defineAction({
		id: 'workbench.sidebar.toggle',
		title: { id: 'workbench.sidebar.toggle' },
		category: { id: 'workbench', title: { id: 'action.category.workbench' } },
		input: noActionInput,
		execution: {
			mode: 'foreground',
			concurrency: 'parallel',
			concurrencyKey: () => 'workbench-sidebar',
		},
		getAvailability: () => ({ kind: 'enabled' }),
		handler: {
			execute: () => ({ kind: 'completed' }),
		},
	}),
);

const execution = actions.execute('workbench.sidebar.toggle', undefined, {
	scope: applicationScope,
	source: 'button',
});
await execution.completion;

registration.dispose();
runtime.dispose();
```

Action 定义拥有稳定 ID、描述符、动态输入验证、可用性、确认、执行策略和结果映射。运行时不是产品状态 Store，也不会仅因某个 Action 可被枚举就授予自动化权限。

`discover(...)` 默认向外解析活动范围。全局命令表面可以从其应用程序范围请求后代注册，并接收范围绑定的 `ready` 或 `requires-input` 结果。仅当其定义提供 `getDiscoveryAvailability` 时，未解决的操作才是可枚举的；将其运行时拥有的令牌和收集的输入传递给 `prepareInvocation(...)`，然后使用 `executePrepared(...)` 执行生成的不透明 `PreparedActionInvocation`。令牌保留验证者的规范输入和准确的注册身份；处置或替换会阻止执行，而不是退回到另一个注册。验证器必须返回所有者拥有的规范值，并复制任何调用者拥有的可变集合或保留以供以后执行的嵌套值。

仅当同一个 `createTypedActionRuntime(...)` 适配器注册定义并调用它们时，所有者定义的 Action 目录才有效。它的 `register(...)` 方法根据目录检查每个定义的 ID、输入和处理程序输出，而 `execute(...)` 检查调用输入和执行输出。

`execute(...)` 在解析之前捕获一个浅只读调用快照，并始终立即返回 `ActionExecution`。可用性、确认、并发和处理程序阶段观察相同的快照。同步处理程序可能已经有终端快照；异步处理程序公开相同的句柄、`completion`、进度、协作取消信号和显式提交点。完成解析为结构化结果，并且永远不会因未分类的处理程序、确认、验证、可用性或并发键异常而拒绝。每个定义为其语义并发键声明 `parallel`、`reject-duplicate`、`replace-latest` 或 `serialize` 行为。

`replace-latest` 先将替换动作登记为当前执行，再请求取消旧执行。若中止回调或执行观察者同步发起更新的调用，该调用会继续拥有当前执行身份；中间调用在处理程序启动前即被取代。

注册表与执行状态发布会捕获固定的观察者集合。每个回调在一次发布中最多被访问一次，
即使它移除并重新加入自身；轮到执行之前已被移除的回调会被跳过。观察者异常仍与注册、
执行和完成隔离。[运行时订阅轨迹](src/runtime/__tests__/action-runtime.unit.test.ts)
通过公开 API 覆盖两个发布所有者，包括取消与替换重入。

`notifyChanged()` 是可用性刷新提示。释放后它不再执行操作，包括已由挂载适配器排队的提示；
它不能恢复注册，也不能向已释放的观察者发布。

仓库级语义和生命周期契约见 [`docs/frontend/actions.md`](../../docs/frontend/actions.md)。

```sh
pnpm --filter @orchbee/actions verify:correctness
pnpm --filter @orchbee/actions test:exports
```

<!-- documentation-pairing: english-sha256=1a6dae45ac2a61b1fa1d48bae87204152cd760a47f4484a81f2dc15e2168b8af zh-cn-sha256=08f9976956387036fa750e95ef9695d2d3d51e45dbcd80b6ff7b0a2c3671132a -->
