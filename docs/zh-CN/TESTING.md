# 测试

## 源码和测试路径

`orchbee/filename-naming-convention` 强制执行：

| 产物                    | 命名契约                                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------- |
| 源码目录                | 使用 kebab-case。                                                                                                   |
| `locales/` 的直接子目录 | 使用规范 BCP 47 标识符和大小写，例如 `en-US` 或 `zh-CN`。                                                           |
| 唯一运行时身份          | 只计算具名运行时导出；仅类型导出和私有实现符号不改变公共身份。                                                      |
| 自有函数模块            | 唯一具名运行时导出是本地定义的函数时，使用精确的 camelCase 符号，例如 `formatDisplayLabel.ts`。                     |
| 自有类模块              | 唯一具名运行时导出是本地定义的类时，使用精确的 PascalCase 符号，例如 `EventEmitter.ts`。                            |
| 通用模块                | 成组操作、契约、数据目录、常量、重新导出门面及其他没有唯一可调用或可构造身份的模块，使用按职责命名的 kebab-case。   |
| 生成的源码              | 保留由生成器拥有的 kebab-case 名称。                                                                                |
| 自有函数组件            | 使用唯一运行时导出的精确 PascalCase 名称，例如 `FilterPanel.tsx`。                                                  |
| 自有专用 Hook           | 使用唯一 Hook 导出的精确 camelCase 名称，例如 `useFilterPanel.ts`。                                                 |
| 非组件 TSX 入口         | 使用 kebab-case。                                                                                                   |
| 单元测试                | 使用同级 `__tests__` 目录，并在 `<source-name>.unit.test.ts` 或 `<source-name>.unit.test.tsx` 中精确保留源码 stem。 |

## 必需的单元测试

- `orchbee/require-unit-test`：每个包含顶层函数、函数表达式、箭头函数或类的 `.ts` 源码文件都必须有对应的单元测试文件。

| 范围               | 契约                                                                                                                 |
| ------------------ | -------------------------------------------------------------------------------------------------------------------- |
| 必需的同级测试     | 只提供可发现性，不证明覆盖率。测试可观察结果、失败路径和所属不变量；占位测试和只断言实现细节的测试不合格。           |
| `.tsx` 源码        | 测试可选；只为有意义的回归、交互、无障碍契约或集成边界添加测试，不要重复准备好的 props 或静态标记。                  |
| 检入的 shadcn 快照 | 保留上游实现作为可审查源码并遵循可选 TSX 策略。测试 Orchbee 适配、公共包契约和依赖的集成行为，不要复制上游测试套件。 |
| 生成的移动端 Hook  | 免除逐文件同级测试。                                                                                                 |

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

## 原生应用测试

- 把 Rust unit-test body 放在相邻 test-module 文件或 crate 的 `tests/` 树中，只从 production
  source 暴露窄化 conditional module 声明，并通过根 test 门禁运行每个 Cargo member。Rust
  专属布局和 failure-boundary 规则以 [`docs/RUST.md`](RUST.md#测试) 为权威。
- 把所有 Zig 测试和测试专用 helper 放在所属源码树的 `__tests__` 目录下，并通过应用的非交互式 `test` 脚本公开每个测试根节点。Zig 专属布局、命名和白盒访问的权威来源是 [`docs/ZIG.md`](ZIG.md#测试)。
- 通过 `test:cross` 编译每个维护的操作系统系列；绝不能仅凭交叉编译就声明运行时、ABI、PTY、IPC、打包或安装器行为。
- 在声明某项平台行为受支持前，在其原生操作系统上运行行为和集成测试。
- 固定必需测试使用的每个原生 dependency resolution 和 compiler input；机器本地的同级
  checkout 和可变 revision 不是测试 fixture。

## 包契约

库包在 `build` 后运行 `test:exports`，以验证打包后的公共表面、两种 JavaScript 模块格式和严格的外部 TypeScript 使用方。跨包行为应放在面向使用方的契约测试中，而不是放在导入其他包私有源码的测试中。
