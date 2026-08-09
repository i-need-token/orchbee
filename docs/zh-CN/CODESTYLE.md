# 代码风格

## 格式化

- `.oxfmtrc.json` 对 Oxfmt 管理的文件具有权威性；Zig 源码和构建文件使用工作区精确固定的 `zig fmt` 实现。
- 修改受格式化工具管理的文件后，从仓库根目录运行 `pnpm format`；它会调用 Oxfmt 和每个已声明的原生工作区格式化器。
- `useTabs`：在文件格式允许使用 Tab 缩进的地方一律使用 Tab。
- `singleQuote` 和 `jsxSingleQuote`：JavaScript、TypeScript 和 JSX 字符串使用单引号，除非使用双引号能让转义更清晰。
- `semi`：语句以分号结束。
- `trailingComma`：在 Oxfmt 支持的地方一律使用尾随逗号。
- 不要用缩进空格手工对齐代码；让 Oxfmt 决定语法或对齐所需的空格。

## 模块

- `import/no-default-export`：除配置文件和 JavaScript lint 插件入口点外，使用命名导出。
- `import/no-cycle`：保持模块依赖无环。
- `import/no-self-import`：不要从模块自身导入。
- `import/no-duplicates`：合并来自同一模块的导入。
- `typescript/consistent-type-imports`：使用 `import type` 或内联 `type` 说明符标记仅类型依赖。

## 绑定和控制流

- `no-var`：使用 `const` 或 `let` 声明绑定。
- `prefer-const`：绑定从未重新赋值时使用 `const`。
- `no-shadow`：为嵌套绑定使用不同的名称。
- `eqeqeq`：使用严格相等运算符。
- `unicorn/no-nested-ternary`：用具名分支或控制流替代嵌套三元表达式。
- `no-console`：通过有明确归属的日志或宿主边界输出运行时诊断。

## 内联抑制

- `orchbee/no-inline-suppressions`：从生产代码和测试代码中移除 `oxlint-disable` 和 `eslint-disable` 注释；修复代码，或者经人工批准后修改项目规则。

## 动态代码

- `no-eval`、`no-implied-eval`、`no-new-func`：不要执行由字符串组装的代码。

## 注释

注释用于解释约束和不明显的决策；名称和结构用于解释行为。

```ts
// ❌ 增加重试次数。
retryCount += 1;

// ✅ Bridge 允许三次瞬时故障，之后才向宿主报告错误。
retryCount += 1;
```

## React

React 专属规则的权威来源是 [`docs/frontend/react.md`](frontend/react.md)。

## Zig

Zig 专属的命名、资源、失败、边界、并发和测试规则的权威来源是 [`docs/ZIG.md`](ZIG.md)。
