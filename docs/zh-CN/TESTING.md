# 测试

## 源码和测试路径

- `orchbee/filename-naming-convention`：源码路径使用 kebab-case；把单元测试放在源码文件同级的 `__tests__` 目录中，并命名为 `<source-name>.unit.test.ts` 或 `<source-name>.unit.test.tsx`。

## 必需的单元测试

- `orchbee/require-unit-test`：每个包含顶层函数、函数表达式、箭头函数或类的 `.ts` 或 `.tsx` 源码文件都必须有对应的单元测试文件。

同级文件规则是最低可发现性要求，并不代表已经具备行为覆盖。测试应检验可观察结果、失败路径和所拥有的不变量；占位测试或只重复实现细节的测试不符合项目契约。

检入的 shadcn 注册表快照及其生成的移动端 Hook 可免除逐文件同级测试。它们保留上游实现作为可审查的源码；应测试 Orchbee 的适配、公共包契约和所依赖的集成行为，而不是逐个组件复制上游组件测试套件。

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
