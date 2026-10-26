# 应用

## 放置位置

| 产物或决策         | 契约                                                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| 可运行或部署的入口 | 放在 `apps/*` 中；最终用户产品和开发应用都使用该位置。                                                                   |
| 通过导入使用的能力 | JavaScript 和 TypeScript 库放在 `packages/*`；可复用 Rust 库放在 `crates/*`。                                            |
| 应用内部 Rust 边界 | 按 [Rust](RUST.md#放置和工作区所有权)将已准入的私有库放在 `apps/<app>/crates/<seam>`；它没有独立应用生命周期。           |
| 仓库自动化         | 没有独立应用生命周期时放在 `scripts/*` 中。                                                                              |
| 产品/开发区别      | 只保留一个 `apps/*` 根目录；通过工作区身份、package privacy、`project.json` 职责与非目标，以及存在时的发布配置表达区别。 |
| 产品应用名称       | 使用它拥有的用户界面或运行时角色，不使用 `main` 或 `renderer` 等内部进程名。                                             |
| 无头长时间运行入口 | 只有存在具体生命周期和使用方时才创建；架构图不足以作为准入理由。                                                         |

## 必需的应用结构

每个直接应用工作区都包含：

| 路径                           | 职责                                                        |
| ------------------------------ | ----------------------------------------------------------- |
| `project.json`                 | 每个应用的运行时、稳定性、职责和非目标                      |
| `package.json`                 | pnpm 管理应用或 Zig lifecycle bridge 的身份、命令和依赖     |
| `Cargo.toml`                   | Rust 应用身份、target 与依赖；它是显式的根 workspace member |
| `tsconfig.json`                | 应用包含 TypeScript 源码时的类型检查契约                    |
| `build.zig` 和 `build.zig.zon` | Zig 应用的构建图和内容寻址依赖契约                          |

| 应用            | 生命周期契约                                                                                                    |
| --------------- | --------------------------------------------------------------------------------------------------------------- |
| pnpm 管理的应用 | 公开其生命周期所需的脚本。                                                                                      |
| 浏览器应用      | 通常公开 `dev`、`build`、`typecheck`、`test` 和 `test:watch`。                                                  |
| 原生 Zig 应用   | 公开非交互式 `build`、`format`、`format:check` 和 `test`；当它声明支持多个操作系统目标时公开 `test:cross`。     |
| 原生 Rust 应用  | 直接参与根 Cargo workspace 与根 Rust 门禁；不要添加 `package.json`、占位 TypeScript 或重复的应用级 Cargo task。 |
| 专门的开发应用  | 可以添加显式工作流命令；必需 build work 必须保持非交互式，以便 `pnpm verify` 运行。                             |

## 工作区开发

| 关注点     | 契约                                                                                                                                                               |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 开发源码   | 浏览器应用注册官方 `@rsbuild/plugin-source-build`；它解析 `@orchbee/source`，在应用图中编译已声明公共源码入口，并让应用的 Rsbuild 服务器负责 React 和 CSS 热替换。 |
| HMR 进程   | 包和应用源码修改共用原生 HMR 图；不要添加包预构建、并行库 watcher 或仓库进程监督器。                                                                               |
| 编译器插件 | 注册 source-built 包需要的每个插件。例如，source-build `@orchbee/ui` 还需要其 Tailwind CSS 插件，因为不会加载另一个工作区的 Rslib 配置。                           |
| 生产构建   | 只为开发注册源码构建。生产构建解析 `types`、`import`、`require` 和 `default`，并使用 Rslib 输出。                                                                  |
| 导入可见性 | `@orchbee/source` 只改变已声明公共入口的编译方式，不改变应用可以导入的包路径。                                                                                     |
| 根命令     | 公开 `pnpm web` 或 `pnpm storybook` 等命令。                                                                                                                       |
| 应用 `dev` | 保持为直接的 Rsbuild 或 Storybook 命令，使底层工具拥有 CLI 参数和进程生命周期。                                                                                    |

## 原生应用开发

| 关注点 | 契约                                                                                                                                                           |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 工具链 | Rust 使用根目录精确的 `rust-toolchain.toml`；Zig 在所属构建和 CI 配置中固定精确版本。不同 compiler 必须在生成 artifact 前失败。                                |
| 依赖   | Rust 使用根 Cargo workspace 与 lockfile；Zig 在 `build.zig.zon` 中通过不可变 URL 和 content hash 声明原生源码依赖。两者都不使用同级 checkout 或机器本地 path。 |
| 编排   | 根 Rust task 直接操作 Cargo member。Zig 应用只把 `package.json` 用作 pnpm lifecycle bridge。两者都不添加占位 TypeScript 或 `tsconfig.json`。                   |
| 格式化 | Rust 使用根 rustfmt 编排；Zig 通过应用的 `format` 和 `format:check` 脚本运行 `zig fmt`。                                                                       |
| 测试   | 在当前平台运行原生 unit test，并通过显式 cross-target task 编译所维护的 operating-system target；cross-compilation 只证明可编译，不证明平台行为。              |
| 输出   | 保持 `target/`、`.zig-cache/` 和 `zig-out/` 未跟踪。发布打包使用已验证 build output，不提交这些输出。                                                          |

原生应用可以在构建期间解析内容寻址的生产依赖。URL、摘要、工具链和所属工作区必须显式；
仍然禁止可变分支、未验证归档和隐式外部检出。

Rust 专属位置、依赖、安全和命令规则以 [`docs/RUST.md`](RUST.md) 为权威；Zig 源码规则以
[`docs/ZIG.md`](ZIG.md) 为权威。

## 依赖方向

- 应用只能通过声明的导出来使用 JavaScript 和 TypeScript 包，只能通过声明的 Cargo dependency
  使用 Rust crate。
- 包、crate 和应用绝不能导入另一个应用的源码树。
- 应用可以通过已声明的 Cargo 边界使用自己的内部 Rust crate；只有该应用和同级内部 crate
  可以依赖它们。共享 crate 和其他应用不能使用这些私有边界。
- 运行时工作区绝不能依赖开发应用。
- 跨工作区发现非运行时产物需要权威的格式和所有权契约；Storybook 遵循 [`docs/frontend/storybook.md`](frontend/storybook.md)。

## 产品应用和开发应用

产品应用属于 Orchbee 面向用户的运行时或发行版。开发应用支持浏览、测试或记录 UI 代码等可重复的仓库工作流。

开发应用：

- 在 npm metadata 中将 `private` 设为 `true`，或在 Cargo metadata 中将 `publish = false`；
- 在 `project.json` 中说明仅用于开发的角色和产品非目标；
- 为已跟踪源码提供聚焦构建验证，并将其本机测试纳入本地验证门禁；
- 不进入产品打包和发布输出；
- 不得成为可复用运行时代码的存放位置。

只有在构建、发布或策略自动化需要时，才添加机器可读的产品/开发分类。不要添加仅仅重复工作区名称和职责的元数据。

## 验证

`pnpm verify` 为每个提供 test 脚本的 pnpm 应用工作区和每个根 Cargo member 运行本机测试，
对具有 TypeScript 契约的应用执行类型检查，并运行原生格式化。它只准备这些检查消费的产物。
应用包通过显式工作区构建或 `pnpm build:full` 生成；其他平台的编译使用 `pnpm test:cross`。
日常影响范围选择和构建触发条件由[验证规范](VERIFICATION.md#合并门禁)管理。
应用构建输出在本地或 CI 中生成，并保持未跟踪状态。

<!-- documentation-pairing: english-sha256=cabb1f403e71f4a41c0b26d4fc04cdaf129b6763edaf6781354433c85dd3e4c6 zh-cn-sha256=1305559d5073c629df053d1cec62bb75a8b17d973031446510e9493f829d3a08 -->
