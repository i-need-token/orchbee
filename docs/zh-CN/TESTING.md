# 测试

## 源码和测试路径

- `orchbee/filename-naming-convention`：源码目录使用 kebab-case。`locales/` 的直接子目录使用规范 BCP 47 locale 标识符，包括 `en-US` 和 `zh-CN` 这样的规范大小写。恰好只有一个具名运行时导出且其本地定义为函数的自有源码模块，使用该符号精确的 camelCase 名称，例如 `startWebApplication.ts`；对应的类模块使用其精确的 PascalCase 符号，例如 `EventEmitter.ts`。仅类型导出不计数，私有实现符号也不改变模块的公共身份。没有唯一可调用或可构造运行时身份的模块——包括成组操作、契约、数据目录、常量和重新导出门面——使用按职责命名的 kebab-case。生成的源码保留由生成器拥有的 kebab-case 名称。自有函数组件使用其唯一运行时导出的精确 PascalCase 名称，例如 `TerminalPane.tsx`；专用 Hook 模块使用其唯一 Hook 导出的精确 camelCase 名称，例如 `useTerminalPane.ts`。非组件 TSX 入口文件仍使用 kebab-case。把单元测试放在源码文件同级的 `__tests__` 目录中，并在 `<source-name>.unit.test.ts` 或 `<source-name>.unit.test.tsx` 中精确保留源码 stem。

## 必需的单元测试

- `orchbee/require-unit-test`：每个包含顶层函数、函数表达式、箭头函数或类的 `.ts` 源码文件都必须有对应的单元测试文件。

同级文件规则是最低可发现性要求，并不代表已经具备行为覆盖。测试应检验可观察结果、失败路径和所拥有的不变量；占位测试或只重复实现细节的测试不符合项目契约。

`.tsx` 源码的测试是可选的。不要仅仅为了重复准备好的 props 或静态标记而添加组件测试；当测试能够保护有意义的回归、交互、无障碍契约或集成边界时再添加。

检入的 shadcn 注册表快照遵循相同的 TSX 可选测试策略，其生成的移动端 Hook 可免除逐文件同级测试。它们保留上游实现作为可审查的源码；应测试 Orchbee 的适配、公共包契约和所依赖的集成行为，而不是逐个组件复制上游组件测试套件。

## 测试执行

- `vitest/expect-expect`：每个测试都必须包含断言。
- `vitest/no-disabled-tests`：不要提交被跳过的测试。
- `vitest/no-focused-tests`：不要提交聚焦测试。
- `vitest/no-identical-title`：同一测试套件中的测试标题必须唯一。

## 类型错误测试

测试可以使用 `@ts-expect-error`，但必须附带至少十个字符的说明；`@ts-ignore` 和 `@ts-nocheck` 仍然禁用。

测试中允许为平台 Mock 和有意无效的边界输入使用类型断言；`any`、`oxlint-disable` 和 `eslint-disable` 仍然禁用。

测试局部辅助函数可以依赖推断的返回类型。

```ts
// ❌
// @ts-ignore
acceptsOnlyStrings(1);

// ✅
// @ts-expect-error -- 验证数字会被拒绝
acceptsOnlyStrings(1);
```

## 回归覆盖

每个缺陷修复都要添加一项在修复前失败、修复后通过的测试。

## 本地化契约

产品消息变更要保持源描述符和每个发布的目录完整，验证 ICU 语法和变量，并在 `en-US` 和 `zh-CN` 下渲染具有代表性的行为。Locale 协商测试应覆盖受支持的变体、无效输入、不受支持的语言和繁体中文回退。权威的消息和视觉审查要求位于 [`docs/frontend/localization.md`](frontend/localization.md#验证)。

## 包契约

库包在 `build` 后运行 `test:exports`，以验证打包后的公共表面、两种 JavaScript 模块格式和严格的外部 TypeScript 使用方。跨包行为应放在面向使用方的契约测试中，而不是放在导入其他包私有源码的测试中。
