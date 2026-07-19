# TypeScript

## 编译器契约

| 选项                         | 要求                                                           |
| ---------------------------- | -------------------------------------------------------------- |
| `strict`                     | 将所有隐式不确定性视为类型错误                                 |
| `noUncheckedIndexedAccess`   | 显式处理缺失的索引值                                           |
| `exactOptionalPropertyTypes` | 区分属性缺失和显式的 `undefined`                               |
| `erasableSyntaxOnly`         | 使用 JavaScript 运行时语法；用 `as const` 对象表示类似枚举的值 |
| `verbatimModuleSyntax`       | 明确区分运行时导入和仅类型导入                                 |
| `module: preserve`           | 为工作区构建工具保留源模块语法                                 |
| `moduleResolution: bundler`  | 与 Rslib、Rspack、Vite 和 Vitest 的模块解析方式保持一致        |

## 类型安全绕过手段

- `typescript/no-explicit-any`：使用具体类型，或者使用 `unknown` 后再验证或收窄。
- `typescript/no-empty-object-type`：不要使用 `{}` 或空接口作为宽泛的占位类型。
- `typescript/no-unsafe-function-type`：使用显式函数签名，而不是 `Function`。
- `typescript/no-wrapper-object-types`：使用原始的 `string`、`number`、`boolean`、`symbol` 和 `bigint` 类型。
- `typescript/consistent-type-assertions`：不要在生产代码中使用类型断言。
- `typescript/no-non-null-assertion`：证明值存在，而不是追加 `!`。
- `typescript/ban-ts-comment`：不要在生产代码中抑制 TypeScript 诊断。

## 公共边界

- `typescript/explicit-module-boundary-types`：为导出函数和公共方法声明返回类型。
- 在边界处使用 Zod 解析不可信值，在内部传递推断出的已验证类型。
- 对预期失败返回可辨识联合；抛出错误只用于不变量被破坏和依赖不可用的情况。

```ts
// ❌
const config = JSON.parse(rawConfig) as Config;

// ✅
const config = ConfigSchema.parse(JSON.parse(rawConfig));
```

## 模块语法

- `typescript/no-require-imports`：使用 ESM 导入，而不是 `require()`。
- `typescript/no-namespace`：使用 ES 模块和对象组合，而不是 TypeScript namespace。
- `typescript/prefer-as-const`：当类型拓宽会丢失有用信息时，使用 `as const` 保留字面量值。
- 相对 TypeScript 导入省略运行时扩展名；写 `./validation`，不要写 `./validation.js` 或 `./validation.ts`。
- 库构建必须为生成的 bundleless 声明添加运行时扩展名。使用 Rslib 时，在生成声明的输出上把 `redirect.dts.extension` 设为 `true`。
- TypeScript 输出直接在 Node.js 中运行的工作区必须覆盖基础配置，将 `module` 和 `moduleResolution` 设为 `nodenext`；该工作区在相对源码导入中使用显式 `.js` 扩展名。

## 冗余类型

- `typescript/no-inferrable-types`：省略重复字面量初始化器推断类型的注解。
- `typescript/no-unnecessary-type-assertion`：移除不会改变推断类型的断言。

## 验证

`pnpm typecheck` 是必需的，因为当前 oxlint 命令不运行类型感知规则。
