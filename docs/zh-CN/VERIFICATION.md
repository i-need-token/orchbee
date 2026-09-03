# 验证

## 合并门禁

`pnpm verify` 是唯一的本地和 CI 合并门禁。提交一个已完成的变更前，从仓库根目录运行它。它按顺序执行：

1. `pnpm format:check`
2. `pnpm lint`
3. `pnpm test:tla:inventory`
4. `pnpm build`
5. `pnpm typecheck`
6. `pnpm test`
7. `pnpm test:exports`

迭代期间可以使用聚焦命令，但完成工作必须通过根级门禁。

## 必需任务

| 任务                 | 契约                                                                                                                                     |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `format:check`       | Oxfmt 管理的文件与 `.oxfmtrc.json` 一致，并且 rustfmt 与每个应用自有原生 formatter 都接受其所属文件。                                    |
| `lint`               | 代码、Rust workspace 与 Clippy、文档配对、样式、存储、架构、依赖、项目契约、体验目录、导出和清单规则全部通过。                           |
| `test:tla:inventory` | TLA+ 执行清单、产物登记、owner 命令、触发映射以及 runner 负向控制彼此一致。                                                              |
| `build`              | 每个可构建 pnpm 与 Cargo workspace 都使用其固定工具链和依赖生成所支持的产物。                                                            |
| `typecheck`          | 每个具有 TypeScript 契约的工作区都满足该契约。                                                                                           |
| `test`               | 契约检查器测试、文档配对负向测试、自定义 lint 规则测试、pnpm 与 Cargo 测试套件、必需的受控本地系统见证和已声明原生交叉编译检查全部通过。 |
| `test:exports`       | 每个库的打包导出、运行时格式和声明均可被使用。                                                                                           |

- `build` 先于 `typecheck`，因为验证和生产使用方解析库产物，不解析另一个包的源码。
- 递归构建采用拓扑顺序：全新检出先构建依赖，再让依赖方解析其导出。
- 应用模块开发不消费库的 `dist`；Rsbuild 把已声明源码入口解析到开发图中。通过 Node 执行、且消费库 runtime export 的开发准备流程，必须在进程启动前显式构建该依赖。
- `pnpm verify` 可以重建包输出而不中断正在运行的应用服务器；验证和生产继续使用按拓扑顺序构建的产物。
- Rust workspace 工作使用精确的根工具链与已提交 Cargo lockfile；Zig 源码依赖使用不可变 URL 与
  content hash。必需 build 必须在生成 artifact 前拒绝不同 compiler 或 dependency resolution。
- `scripts/run-rust-workspace.mjs` 把根 pnpm 门禁连接到 Cargo。第一个已接受 Rust member 存在前，
  它验证固定工具链和 empty workspace metadata；之后同一批 task 使用 `--locked` 对每个 member
  运行 rustfmt、Clippy、build 与 test。
- 原生交叉编译检查只证明源码和依赖可为已声明目标编译；平台行为仍然需要原生 runner。
- 必需的 Desktop SSH Project-enrollment 见证会打包真实 Dev profile，并在 macOS arm64 或 glibc
  Linux x64 上针对临时 loopback `sshd` 运行。不支持的本地平台会明确报告跳过；必需的 Linux CI
  job 会配置 `sshd`、Xvfb 与 X authentication 并执行该见证。见证会移除自己的临时 home、key、
  已安装 runtime 与 application data，而且绝不会调用 manual native-window test。

## 变更触发的 TLA+ 验证

`pnpm test:tla` 会通过共享的有界 runner 运行所有已登记模型。`scripts/tla/models.json` 中的
执行清单为每项检查登记模块/配置、语义 owner、claim 权威、变更路径触发器、超时、配置变体和
必需 action coverage。它是执行索引，而不是第二份行为规范；package 自有契约和 TLA+ 模块
仍然是模型语义的权威。

TLA+ 模型检查属于变更触发的证据：`pnpm test` 和必需的 CI `verify` Job 不会隐式运行 TLC。
`pnpm verify` 只运行低成本的 `test:tla:inventory` 一致性与负向控制检查。独立的 CI `tla`
Job 会把 Git diff 映射到已登记模型，并只运行这些检查。diff base 缺失或无效时会安全地
退化为选择所有模型。

当变更修改了所属工作区的 TLA+ 模块、配置、owner 契约、转换、不变量或精化映射时，运行
该工作区的 `test:tla` 命令。当多个模型 owner 或共享模型基础设施发生变化时，运行根命令。
`pnpm test:tla:changed -- <ref>` 使用与 CI 相同的清单映射。包自有的 `verify:correctness`
命令可以在该命令属于包的完整正确性工作流时包含 TLA+ 检查。

每项已登记检查都使用固定 SHA-256 的 TLC、单 worker、固定指纹多项式、已声明的有限边界、
逐模型超时和 coverage 输出。runner 会在必需语义 action 缺失或没有可达 witness 时失败，
并输出机器可读的状态数、耗时以及模型/配置 SHA-256 报告。配置边界和 TLC 的有限状态假设
仍是证据限制；action coverage 不是实现精化证据。

本地 TLC 执行需要 Java 17 或更高版本，并使用 `PATH` 中的 `java`。设置
`ORCHBEE_TLA_JAVA` 可指定可执行文件，设置 `ORCHBEE_TLA2TOOLS_JAR` 可使用已配置且具有固定
SHA-256 的副本，设置 `ORCHBEE_TLA_TIMEOUT_MILLISECONDS` 可对所有选中检查应用正整数超时覆盖。

无关实现变更不需要重放未修改的模型。未修改模型通过也不能证明其抽象仍与实现对应；
确定性测试和可追踪性证据负责这种对应关系。影响模型的变更只有在选定的 TLA+ 命令通过，
且完成报告指出已检查模型和边界后才算完成。

## CI 契约

- `.github/workflows/verify.yml` 安装冻结的 lockfile，并在 Pull Request、向 `main` 推送和手动运行时执行 `pnpm verify`。
- 必需的 `verify` Job 不运行 TLC。必需的 `tla` Job 从已跟踪清单中选择并运行变更触发检查；
  无关变更会在不配置 Java 或 TLC 的情况下完成该 Job。
- 工作流通过固定的官方工具链机制安装每个已跟踪原生 workspace 要求的精确 compiler version。
  直接下载的 archive 在加入 `PATH` 前必须校验 digest。
- 在仓库 ruleset 中要求工作流的 `verify` 和 `tla` Job。
- CI 在验证后运行 `git diff --exit-code`；已跟踪的生成变更在提交前会导致失败。
- 工作流使用只读仓库权限。
- 凭据、写权限、发布操作、外部检出和其他副作用需要单独人工审查，不得隐式进入必需 Job。

## 外部来源审计

- 生产构建使用的内容寻址原生源码依赖是已声明构建输入，不是外部来源审计。其不可变 URL 和摘要属于所属原生清单。
- 需要外部检出、凭据、服务、设备或未声明环境输入的包专属审计，继续作为所属包记录的显式维护命令，不属于 `pnpm verify`。
- 只有当独立 CI Job 固定并配置每个外部输入、拥有稳定所有者且不为必需合并门禁添加副作用时，才提升这类审计。

## 扩展阈值

| 决策           | 契约                                                             |
| -------------- | ---------------------------------------------------------------- |
| 默认           | 必需门禁保持快速时，运行完整工作区任务。                         |
| 受影响任务阈值 | 至少三个可独立变更的工作区，或冷启动 CI 关键路径持续超过 30 秒。 |
| 变更前         | 先测量瓶颈。                                                     |
| 全局策略检查   | 即使包任务改为仅运行受影响项，也保留显式输入。                   |
