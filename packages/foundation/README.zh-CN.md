# @orchbee/foundation

Orchbee 的私有包，提供跨工作区边界共享、领域无关且依赖轻量的编程原语。

## 准入

API 只有在具备具体的工作区使用方，并且脱离产品、功能、平台、传输、持久化或呈现语义后仍有意义时，才属于此处。仅有复用需求、文件结构或实现相似性并不足以准入。特定于某个所有者的辅助逻辑留在其语义所有者中。

根入口是经过审查的允许列表，不是自动 barrel。框架和环境集成还使用显式子路径，让使用方可以直接指出该依赖边界。

## React

`@orchbee/foundation/react` 拥有语义依赖 React 渲染生命周期的领域无关原语。React 是 peer dependency。`useMemoizedFn` 只从显式 React 子路径导出；根入口没有兼容别名。

```ts
import { useMemoizedFn } from '@orchbee/foundation/react';

const handleCommit = useMemoizedFn(() => {
	// Reads the latest render values without changing function identity.
});
```

`useMemoizedFn` 在每次渲染间保持同一个函数标识，同时调用最新提交的实现。暂停或被丢弃的渲染无法改变上一次提交所暴露的可调用行为。该 Hook 保留原始参数、返回类型和调用方提供的 `this`。

仓库级准入和包规则见 [`docs/PACKAGES.md`](../../docs/PACKAGES.md#foundation-boundary)。
