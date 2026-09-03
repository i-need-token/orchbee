# Rust

## 范围和权威归属

本标准约束手写 Rust 源码、Cargo 清单和仓库 Cargo workspace。应用放置仍以
[应用](APPLICATIONS.md)为权威，语义所有权和可见性从[模块边界](MODULE-BOUNDARIES.md)开始，
仓库执行仍以[验证](VERIFICATION.md)和[测试](TESTING.md)为权威。

根目录 `rust-toolchain.toml` 固定完全一致的 compiler、Cargo、rustfmt 与 Clippy cohort。根目录
`Cargo.toml` 是唯一 Cargo workspace root。Rust project 不得选择自己的工具链或创建嵌套 workspace。

## 放置和工作区所有权

| 产物                        | 放置和契约                                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 可执行应用                  | 放在 `apps/<name>`，直接包含 `Cargo.toml` 与 `project.json`；在根 `workspace.members` 中显式列出。                        |
| 可复用 Rust 库              | 放在 `crates/<name>`，直接包含 `Cargo.toml` 与 `project.json`；只有至少两个真实使用方，或一个刻意独立的稳定契约时才准入。 |
| 应用私有代码                | 作为所属应用 crate 内的 module 保留；文件数量、测试访问和可能的未来复用都不足以证明需要共享 crate。                       |
| JavaScript 或 TypeScript 库 | 保留在 `packages/*` 并遵循[包](PACKAGES.md)标准；不要仅为复用 npm namespace 就把 Cargo crate 放进去。                     |

Cargo package 名使用 kebab-case。可执行文件名与安装后的进程名一致。`crates/<name>` 下的内部库
使用 `orchbee-<name>`，除非目录本身已命名为 `orchbee`。每个仓库自有 crate 都设置
`publish = false`，直到已接受的发布设计明确将其公开。

`project.json.runtime` 描述生成代码在哪里运行，而不是实现语言。因此 Rust executable 使用
`native`；`Cargo.toml` 是语言权威。不要在 `project.json` 中添加重复的 language 字段。

应用 crate 可以通过 workspace dependency 依赖 `crates/*`。crate 绝不依赖 application crate，
一个应用也绝不导入另一个应用的源码或 Cargo target。只有在行为具备稳定语义 owner 和真实
consumer 后，才把它移到共享边界。

## 清单、依赖和生成状态

- 每个 member 从根清单继承 `edition`、`rust-version` 和 workspace lint。
- 第一个真实 member 创建唯一的根 `Cargo.lock`；提交它，并在必需验证和发布命令中使用
  `--locked`。禁止嵌套 lockfile。
- 多个 member 有意共享 dependency version 或 feature 时，在 `[workspace.dependencies]` 中声明。
  member 仍需通过 `workspace = true` 显式选择该 dependency。
- Registry dependency 由已提交 lockfile 解析。Git dependency 固定不可变 revision。Path
  dependency 必须位于本仓库内；禁止机器本地路径和同级 checkout。
- 只有完整 default feature 行为都属于产品时才保留它。显式声明实际使用的 feature，并把
  feature 扩张视为 dependency 和 binary-size 变更。
- `build.rs` 必须有具体的构建期职责。它只写入 Cargo `OUT_DIR`，声明每个 rerun input，不访问
  网络，也绝不编辑已跟踪源码。
- `target/` 和生成的构建输出保持未跟踪。生成的 protocol binding 标明其来源，并由所属契约
  检查是否陈旧。

## 模块和可见性

- Type 与 trait 使用 `UpperCamelCase`，module、function 与 value 使用 `snake_case`，constant 与
  static 使用 `SCREAMING_SNAKE_CASE`。
- 把 `pub` 视为架构边界。优先使用 private item 和 `pub(crate)`；只有真实 consumer 所需时，
  才通过所属 crate boundary 暴露 public item。
- Composition 与 process startup 留在 binary crate。Domain module 不依赖 CLI、TUI、transport、
  persistence 或 service-lifecycle adapter。
- 不创建 `util`、`common`、`helpers` 或 `shared` 包罗万象模块。按模块唯一所属职责命名；当
  ownership、invariant 或 dependency direction 分叉时将其拆开。
- 生产代码避免 glob import 和 glob re-export。它们会隐藏已审阅 public surface，并让跨模块
  ownership 变更变得非局部。

## 失败、资源和并发

- 对 invalid input、protocol rejection、operating-system failure、service unavailable、
  cancellation、timeout 与 exhausted limit 返回类型化错误。在 adapter boundary 增加上下文，
  但不要把稳定的 machine-readable cause 转换成 presentation text。
- 对外部输入或可恢复 runtime state，不使用 `unwrap`、`expect`、可能 panic 的索引或故意 panic。
  Panic 只代表内部 invariant violation，而且只能终止 process design 承诺的所属 failure domain。
- 每个 file、socket、process、PTY attachment、task、channel、timer 与 temporary artifact 只有一个
  lifecycle owner。Shutdown 会停止 admission、取消或排空有界工作、join 自有 task，并且每项
  resource 只释放一次。
- 限制 input length、decoded collection、queue、retry、concurrency 与 per-dispatch work。每个
  bound 都有可观察的 rejection、backpressure、truncation 或 cancellation outcome。
- 绝不跨 `.await`、IPC、filesystem/process wait、callback 或 blocking computation 持有同步锁。
  无法避免的 blocking work 必须与 async executor 隔离。

Workspace policy 禁止手写 `unsafe` code。如果无法用安全方式表达某个不可避免的操作系统或
foreign-code boundary，则需要单独接受的变更：先把例外收窄到一个 adapter，记录每项 safety
invariant，并添加 boundary-focused test，然后才能修改 workspace lint policy。

## 跨语言契约

Rust、TypeScript 与 Zig binding 绝不能成为同一 serialized contract 的相互竞争权威。每个已接受
的跨语言切片都要明确：

- 一个 semantic owner，以及一个 language-neutral schema 或 encoding specification；
- 与该权威放在一起的 canonical valid、boundary 与 invalid fixture；
- identifier width、byte order、limit、version negotiation 与 unknown-field behavior；
- 哪些 binding 由生成得到，哪些独立实现；以及
- 每种 implementation 的 stale-generation check、differential test 或 conformance test。

已有语言所属 protocol 在其已接受 migration 另有说明前保持当前权威。Rust implementation 使用
该权威及其 fixture；它不会复制 model 后静默重新解释。Presentation text 和本地化 diagnostic
不是 wire contract。

## 测试

- Unit-test body 放在相邻 test module 文件或 crate 的 `tests/` 树中。Production module 可以保留
  挂载该文件所需的窄化 `#[cfg(test)] mod tests;` 声明，但不放 inline scenario 或 fixture。
- 测试 public behavior、invalid 与 maximum input、partial failure 后的 cleanup、cancellation、
  stale identity、backpressure 和 deterministic shutdown。当 ordering 或 state-space 风险值得时，
  使用 property 或 model-based evidence。
- Integration test 使用 public crate 或 process boundary。Test-only visibility 不得扩大 production
  API 或引入 product behavior。
- Cross-compilation 只证明可编译。Runtime、IPC、PTY、installer 与 service-lifecycle 主张需要在
  每个声称支持的 operating-system family 上使用 native runner。

## 必需命令和发布输入

根 pnpm 门禁调用 `scripts/run-rust-workspace.mjs`，使一个命令继续验证整个仓库。当已接受的 Cargo
workspace 尚无 member 时，runner 会验证精确工具链和 workspace metadata，并报告 empty state。
添加第一个 member 后，同一批命令会执行真实 workspace 工作：

| 仓库任务            | Cargo 证据                                                                      |
| ------------------- | ------------------------------------------------------------------------------- |
| `pnpm format:check` | `cargo fmt --all -- --check`                                                    |
| `pnpm lint`         | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| `pnpm build`        | `cargo build --workspace --all-targets --all-features --locked`                 |
| `pnpm test`         | `cargo test --workspace --all-targets --all-features --locked`                  |

聚焦迭代可以使用按 package 选择的 Cargo 命令，但完成时使用根门禁。发布自动化通过固定工具链、
`--locked` 和 `--release` 构建确切 package 与 binary，再把已验证 artifact 暂存到所属 distribution
cohort。它绝不把 `target/` 下碰巧已有的文件视为 release input。Supported target、signing、
install path、update cohort 与 rollback 仍由每个已接受的应用发布切片负责。
