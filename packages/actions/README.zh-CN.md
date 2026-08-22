# @orchbee/actions

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

`formal/tla/ActionExecution.tla` 是该运行时契约的有界正确性模型。一个变体检查单次执行的精确注册代次、已准备令牌失效、调用快照、确认、进度、提交、取消、结果映射和运行时处置。另有四个变体：分别用两次执行检查 `parallel`、`reject-duplicate` 和 `replace-latest`，并用三次执行检查 `serialize`，以证明被取消的中间队列项不会阻塞后继项。将注册和处理程序生命周期维度与并发维度拆开，是显式的状态空间边界，并不声称生产事件会被串行化。TLC 检查安全不变量、就绪队列排空满足弱公平性时的串行队列进展，以及所有变体要求的非零 Action 覆盖率。它不能证明任意执行次数、处理程序终止、已记录浅层快照之下由调用方持有的可变值，也不能证明产品处理程序本身正确。

仓库级语义和生命周期契约见 [`docs/frontend/actions.md`](../../docs/frontend/actions.md)。

```sh
pnpm --filter @orchbee/actions verify:correctness
pnpm --filter @orchbee/actions test:tla
pnpm --filter @orchbee/actions test:exports
```
