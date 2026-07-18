# 包

## 范围和分类

- 当可复用代码已有工作区使用方，或者有意将其作为可独立使用的公共库开发时，把它放入 `packages/*`。
- 在包边界拥有具体使用方之前，把应用专属代码留在该应用的 `src` 树中。
- 按照 [`docs/frontend/package-boundaries.md`](frontend/package-boundaries.md) 放置前端产品 UI、平台适配器和面向会话的库。
- 把 `packages/*` 下的每个运行时包都视为库包。库包使用 Rslib 并公开构建产物。生产使用方使用这些产物；根据工作区开发契约，仓库应用在开发期间可以编译已声明的公共源码入口。
- 对仅供工作区使用的库设置 `"private": true`。准备发布到 npm 的包省略 `private`，并且必须包含版本、`"publishConfig": { "access": "public" }`、包 README、项目许可证选定后的许可证元数据以及 `files` 允许列表。
- 按照 [`docs/PROJECTS.md`](PROJECTS.md)，在基于 Schema 的 `project.json` 中声明每个包的职责、非目标、运行时和稳定性。

仅供仓库使用的脚本和应用入口点不是库包，不属于 `packages/*`。

## 命名

- 使用 `@orchbee` npm scope，并让包名与目录匹配：`packages/<name>` 对应 `@orchbee/<name>`。
- 使用能命名稳定领域或能力的小写 kebab-case 名词。
- 优先使用调用方所使用的抽象，而不是实现技术、依赖或当前传输方式。
- 只有在平台、供应商或协议定义了永久公共边界时，才把它包含在名称中。
- 不要创建名为 `common`、`shared`、`utils`、`helpers` 或 `misc` 的兜底包。应命名它所拥有的能力。
- 不要仅仅为了让名称听起来基础就添加 `core`。只有当同级包围绕该 core 构成有意设计的系列时才使用它。

```text
❌ packages/wasm-terminal  -> @orchbee/wasm-terminal
✅ packages/terminal       -> @orchbee/terminal
```

WASM 和 Ghostty 实现了当前终端；调用方依赖的是终端能力。

## 必需的包结构

每个库包包含这些文件：

| 路径              | 职责                       |
| ----------------- | -------------------------- |
| `package.json`    | 身份、脚本、依赖和公共导出 |
| `src/index.ts`    | 有意选择的根公共 API       |
| `rslib.config.ts` | ESM、CJS、声明和运行时目标 |
| `tsconfig.json`   | 包类型检查契约             |
| `README.md`       | 面向使用方的安装和公共 API |
| `project.json`    | 机器可读的范围和职责       |

测试按照 [`docs/TESTING.md`](TESTING.md#源码和测试路径) 放在源码区域旁。生成的构建输出属于 `dist/`，绝不能提交。

每个库公开这些脚本：

| 脚本           | 命令或职责                                   |
| -------------- | -------------------------------------------- |
| `build`        | `rslib build`                                |
| `dev`          | `rslib build --watch`                        |
| `typecheck`    | `tsc --noEmit`                               |
| `test`         | 运行一次包测试套件                           |
| `test:watch`   | 以 watch 模式运行包测试                      |
| `test:exports` | 构建后验证打包导出、ESM/CJS 一致性和声明使用 |

## 公共 API 和包导出

- 每个包都设置 `"type": "module"`。
- 每个包都定义 `package.json#exports`；未出现在 `exports` 中的文件即使存在于发布的 tarball 中也是私有的。
- 把根导出定义为 `"."`。显式添加每个受支持的子路径，包括资产。不要使用通配符子路径导出。
- 把 `"types"` 条件放在 `"import"` 和 `"require"` 之前。
- 让 `"import"` 指向 ESM，`"require"` 指向 CJS。只有仓库专用的 `"@orchbee/source"` 条件可以指向 `src/*`。
- 在每个可进行源码构建的导出中首先声明 `"@orchbee/source"`。这是仓库开发条件，指向同一个公共入口的源码对应项，绝不能公开私有子路径。
- 当包可发布时，把 `@orchbee/source` 目标包含在包的 `files` 允许列表中。私有包可以从打包输出中省略开发源码。
- 让 `main`、`module` 和 `types` 与根条件导出保持一致，以支持未完全支持 `exports` 的使用方。
- 只能通过允许的包名或已导出的子路径导入其他工作区包。禁止从其他工作区的 `src/*` 或 `dist/*` 导入。
- 在包 README 中记录每个公共入口点和资产。

```json
{
	"name": "@orchbee/example",
	"type": "module",
	"main": "./dist/index.cjs",
	"module": "./dist/index.js",
	"types": "./dist/index.d.ts",
	"files": ["dist", "src", "README.md"],
	"exports": {
		".": {
			"@orchbee/source": "./src/index.ts",
			"types": "./dist/index.d.ts",
			"import": "./dist/index.js",
			"require": "./dist/index.cjs"
		},
		"./schema.json": "./schema.json"
	}
}
```

`src/index.ts` 是允许列表，不是自动 barrel：

```ts
// ❌ 新的内部导出无意间变成公共 API。
export * from './session-store';

// ✅ 值和类型分别经过审查。
export { SessionStore } from './session-store';
export type { SessionSnapshot, SessionStoreOptions } from './session-store';
```

删除或修改已导出的符号、类型、子路径或资产属于破坏性 API 变更。在不改变 `exports` 的情况下移动内部文件则不属于。

## 库构建

- 通过 `"build": "rslib build"` 使用 Rslib 构建每个库包。
- 生成 ESM（`dist/index.js`）、CJS（`dist/index.cjs`）、TypeScript 声明（`dist/index.d.ts`）和 JavaScript source map。
- 把 Rslib 运行时目标设为实际使用方环境：`web`、`node` 或单独审查的多目标配置。
- 除非仓库运行时基线发生变化，否则使用 ES2022 语法。
- 只从一个 Rslib 输出生成声明，避免两种格式写入相互竞争的声明文件。
- 对 `dependencies`、`optionalDependencies` 和 `peerDependencies` 保持 Rslib 的默认外部化。有意打包的依赖属于 `devDependencies`；原因不明显时必须在 `rslib.config.ts` 中解释。
- 除非包是直接部署的浏览器产物，否则保持发布输出不压缩。最终压缩和 polyfill 由使用方应用负责。
- `package.json#files` 只包含运行时产物、声明、资产、使用方文档，以及可发布 `@orchbee/source` 条件所需的源码目标。

使用以下基线配置，并且只有为了已记录的包要求才能修改它：

```ts
import { defineConfig } from '@rslib/core';

export default defineConfig({
	lib: [
		{
			format: 'esm',
			syntax: 'es2022',
			dts: true,
			redirect: {
				dts: {
					extension: true,
				},
			},
		},
		{ format: 'cjs', syntax: 'es2022' },
	],
	output: {
		target: 'node',
		sourceMap: true,
	},
});
```

当声明包含相对导入时，在生成声明的输出上启用 `redirect.dts.extension`，让无扩展名的 TypeScript 源生成与 Node 兼容的声明说明符。

修改清单或构建配置后，运行包构建和 `test:exports`。仓库检查器会验证运行时导出目标和可发布的源码条件目标已包含在 `npm pack` 中、私有源码条件目标存在、根 ESM 与 CJS 导出一致，并且严格的外部 TypeScript 使用方能够解析根声明入口。

## 依赖声明

- 对库运行时必需的包使用 `dependencies`。
- 当使用方必须提供一个共享运行时实例或选择兼容框架版本时，使用 `peerDependencies`。
- 只有库能在运行时处理缺失情况时才使用 `optionalDependencies`。
- 对构建、测试、类型和有意打包的仅实现依赖使用 `devDependencies`。
- 使用 `workspace:^` 引用工作区库，除非确实需要并已记录精确的锁步依赖。
- 在导入包的工作区中声明每个直接导入；不要依赖根依赖或传递依赖。
- `pnpm lint:dependencies` 对变更的工作区不得报告未列出或未使用的生产依赖；其 Knip 工作区入口必须包含每个受支持的公共入口和可执行脚本。
- 把 `@rslib/core`、TypeScript 和 Vitest 等共享工具链版本放入 pnpm 默认 catalog，并用 `catalog:` 引用。
- 把生产依赖范围限制在兼容版本内，并在同一个变更中更新 lockfile。
- 只有确认包没有导入时全局注册、样式表导入、polyfill 或其他可观察副作用后，才能设置 `sideEffects: false`。当只有选定入口点有副作用时，使用显式副作用文件列表。
- `pnpm-workspace.yaml#minimumReleaseAge`：要求注册表版本至少成熟一天后才能解析。保持严格模式启用，只添加精确版本且经过审查的例外。

## 版本管理

Orchbee 包遵循语义化版本控制，并采用 Babel 风格的共享发布线：

- 所有可发布的 `@orchbee/*` 包共享一个仓库级主版本。单个包不能独立进入下一个主版本。
- 只发布发生变化的包。未变化的包保留现有版本，因此次版本和修订版本不需要在整个仓库中保持一致。
- `1.0.0` 之后，不兼容的公共 API 变更要求进入下一个协调主版本线，向后兼容的新增功能递增受影响包的次版本，向后兼容的修复递增修订版本。
- `1.0.0` 之前，不兼容的公共 API 变更递增受影响包的次版本，向后兼容的功能也递增次版本，修复递增修订版本。发布说明必须明确标记破坏性变更。
- 依赖范围变更要求发布依赖方包。当公共 API 保持兼容时至少使用修订版本递增；当依赖类型或行为泄漏到公共边界时，使用与 API 影响相符的版本递增。
- 如果存在可行迁移路径，在移除公共 API 前先弃用它。
- 协调预发布使用 `<version>-beta.<n>`。预发布使用非 `latest` npm dist-tag；稳定版本使用 `latest`。
- 发布规划为每个发生变化的可发布包分配 `major`、`minor`、`patch` 或 `none`。该包所需的最高递增等级生效。

重命名已发布包会创建新的包身份。弃用旧 npm 包并提供迁移消息，不要在旧名称下发布无关代码。

仓库尚未自动化包发布。在首次发布到 npm 前、存在需要保护的真实发布流水线和使用方契约时，添加发布规划和兼容性工具。

## 验证和审查

合并包变更前：

1. 确认名称和职责仍然具体且互不重叠。
2. 把 `src/index.ts` 和 `package.json#exports` 作为公共 API 变更审查。
3. 使用 `pnpm --filter <package-name>... build` 构建包及其工作区依赖。
4. 运行 `pnpm --filter <package-name> typecheck`、`pnpm --filter <package-name> test` 和 `pnpm --filter <package-name> test:exports`。
5. 从仓库根目录运行 `pnpm verify`。
6. 在同一个变更中更新 `project.json`、包 README 和受影响的工程文档。

## 外部参考

验证日期：2026-07-18。

- [Rslib 输出格式](https://rslib.rs/guide/basic/output-format)
- [Rslib 配置](https://rslib.rs/guide/basic/configure-rslib)
- [Rslib 依赖外部化](https://rslib.rs/config/lib/auto-external)
- [Rsbuild source-build 插件](https://github.com/rstackjs/rsbuild-plugin-source-build)
- [Babel 固定版本发布说明](https://babeljs.io/docs/faq#why-isnt-a-certain-babel-x-package-updated)
- [语义化版本 2.0.0](https://semver.org/)
- [pnpm 最短发布时间](https://pnpm.io/settings#minimumreleaseage)
