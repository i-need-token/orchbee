# Rust

## 范围和权威归属

本标准约束手写 Rust 源码、Cargo 清单和仓库 Cargo workspace。应用放置仍以
[应用](APPLICATIONS.md)为权威，语义所有权和可见性从[模块边界](MODULE-BOUNDARIES.md)开始，
仓库执行仍以[验证](VERIFICATION.md)和[测试](TESTING.md)为权威。Rust 可执行文件的命令行行为
遵循从 [CLI 命令面](cli/command-surface.md)开始的 `cli/` 标准；其全屏行为遵循 `tui/` 标准。

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

## 必需的 crate 形态

每个 Cargo member 都包含这些文件：

| 路径                             | 职责                                                    |
| -------------------------------- | ------------------------------------------------------- |
| `Cargo.toml`                     | 身份、target、feature 和依赖；继承根 workspace package  |
| `project.json`                   | 机器可读的范围与职责                                    |
| `README.md` 和 `README.zh-CN.md` | 面向使用方的职责、公共边界和维护命令                    |
| `src/lib.rs` 或 `src/main.rs`    | 唯一的 crate root；库 root 是显式的 re-export allowlist |
| `tests/`                         | 集成与 conformance 测试；crate 声称公共边界时存在       |

应用 crate 只有为了让自己的 `tests/` 树能通过一个已审阅边界触达 binary 的领域操作时，才可以在
`src/main.rs` 旁添加 `src/lib.rs`。该库 target 不是公共 API：没有其他 workspace member 依赖它，
其 `pub` 面仅限于该应用的测试和 binary 所消费的内容，而且当第二个使用方出现时，它绝不能成为
跳过准入真实 `crates/*` 库的理由。

## 清单、依赖和生成状态

- 每个 member 从根清单继承 `edition`、`rust-version` 和 workspace lint。
- 提交唯一的根 `Cargo.lock`，并在必需验证和发布命令中使用 `--locked`。禁止嵌套 lockfile。
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

## 依赖准入

标准库是默认实现。第三方 crate 只为一项 `std` 无法正确提供的具名能力而准入，例如操作系统
绑定、序列化、终端控制或 Unicode 分词；便利、熟悉和更短的代码都不是能力。添加依赖是
[`DEVELOPMENT.md`](DEVELOPMENT.md#决策权威)下的保留决策。

| 关注点     | 契约                                                                                                                                      |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 运行时框架 | workspace 不使用 async runtime、参数解析框架、错误聚合 crate 或日志框架。引入其中任何一个都是 Consequential 工作，而非单个 crate 的选择。 |
| Feature    | 声明 `default-features = false` 并列出实际使用的 feature；启用一个 feature 按依赖变更评审。                                               |
| 版本统一   | 直接依赖在整个 workspace 内解析为一个版本；给 `Cargo.lock` 新增重复传递版本的变更必须在评审中指出它，并说明无法统一的原因。               |
| 传递评审   | 接受前评审新增的传递依赖树、其 `unsafe` 足迹、build script、proc macro、维护状态和许可证。                                                |
| 开发范围   | 仅被测试、conformance 检查或构建工具使用的 crate 留在 `[dev-dependencies]` 或 `[build-dependencies]`。                                    |
| 平台范围   | 只在一个平台需要的 crate 声明在对应的 `[target.'cfg(...)'.dependencies]` 表中。                                                           |
| 移除       | 最后一个使用方消失的依赖在同一变更中移除。                                                                                                |

## 模块和可见性

- Type 与 trait 使用 `UpperCamelCase`，module、function 与 value 使用 `snake_case`，constant 与
  static 使用 `SCREAMING_SNAKE_CASE`。
- 把 `pub` 视为架构边界。选择最小范围：先考虑 private、`pub(super)` 或 `pub(in ...)`，再考虑
  `pub(crate)`。crate 范围或公共 item 必须在该边界有真实使用方。优先使用私有子模块，并显式
  重导出所属操作。
- 编译器可见性是必要条件，但不授权横向依赖。明确所属模块契约与依赖方向；测试访问不得
  扩大生产可见性。
- Composition 与 process startup 留在 binary crate。Domain module 不依赖 CLI、TUI、transport、
  persistence 或 service-lifecycle adapter。
- 不创建 `util`、`common`、`helpers` 或 `shared` 包罗万象模块。按模块唯一所属职责命名；当
  ownership、invariant 或 dependency direction 分叉时将其拆开。
- 生产代码避免 glob import 和 glob re-export。它们会隐藏已审阅 public surface，并让跨模块
  ownership 变更变得非局部。
- 带子模块的模块是与目录 `name/` 并列的文件 `name.rs`；`src/` 下不使用 `mod.rs`。在 `tests/`
  下，共享 helper 放在 `tests/support/mod.rs`，以免 Cargo 把它们编译成独立的测试 crate。
- 父文件拥有其子模块的 `mod` 声明、可见性和 re-export；子模块绝不声明兄弟模块。

Rust lint runner 会在调用 Clippy 前检查每个 workspace member 的 `src/` 树中是否有 `mod.rs`，
包括当前平台不启用的文件。此文件名检查不覆盖集成测试的 `tests/support/mod.rs` 或非 member
依赖树；模块内容与边界仍由编译器检查和语义审阅负责。
[Runner 夹具](../../scripts/run-rust-workspace.test.mjs)覆盖被拒绝和被接受的布局。

## 平台特定代码

- 操作系统 API 与行为放在路径命名了平台或能力的适配器模块中，例如 `linux_permissions` 或
  `connection/native`，或放在 composition root 中。确定性领域、状态所有者和协议模块不含
  `cfg(unix)`、`cfg(windows)` 或 `cfg(target_os = ...)`。
- 用 `#[cfg(...)]` 门控到 item 粒度：`mod` 声明、import、字段、函数、`impl` 或 match 分支。另一
  平台的代码不得编译进本平台的构建。`cfg!(...)` 只用于每个分支在每个 target 上都能编译的策略
  常量。
- 选择正确范围内最宽的谓词。`cfg(unix)` 覆盖 POSIX 行为；只有 API 仅存在于一个操作系统时才
  收窄到 `cfg(target_os = "linux")`，并在调用处看不出原因时用注释指明该 API。
- 不受支持的平台通过与其他拒绝相同的类型化结果失败。`#[cfg(not(unix))]` 存根只存在于适配器
  边界，返回类型化的 unsupported-platform 结果，绝不 panic、静默降级或伪造结果。
- 测试携带与其验证行为相同的谓词。平台行为按[测试](TESTING.md#原生应用测试)在该平台的
  原生 runner 上证明；在其他地方编译门控代码不是证据。

## 文档注释

- 每个 crate root，以及每个拥有契约、状态或适配器的模块文件，都以一段 `//!` 注释开头，用一个
  段落说明其职责和非目标。它不复述模块名，也不列举子模块。
- crate 或模块边界上的每个 `pub` 和 `pub(crate)` item 都带有 `///` 注释，说明该 item 保证什么。
  返回的错误有调用方必须区分的变体时添加 `# Errors`；断言了已证明不变量时添加 `# Panics`。
- 私有 item 遵循[代码风格](CODESTYLE.md#注释)：注释解释约束或非显而易见的决策，而不是签名。
- 注释描述当前行为。任务历史、分阶段切片标记和对话引用不属于源码。
- 声称可编译的文档示例按[测试](#测试)参与 doctest。
- 缺少文档是评审发现项；`missing_docs` 未启用，启用它属于[Lint 策略与执行边界](#lint-策略与执行边界)
  下的 lint 策略变更。

## 类型与受检边界

使用 Rust 原生类型落实共享的[领域建模规则](MODELING-AND-CORRECTNESS.md#领域契约)；不要
复制 TypeScript 的文件数量或语法惯例。

- 为含义不同的身份使用不同的 newtype。任意构造或修改会破坏不变量时，保持字段私有，并
  暴露受检构造函数或 `TryFrom`。当 `NonZero*` 和领域枚举能直接表示已接受值域时使用它们。
- 用携带必需数据的枚举变体表示不同情况。不要用互不关联的布尔值和可选字段编码状态机，
  从而允许不可能的组合。
- 在所属边界校验不可信的 CLI、环境、文件、IPC 和外部代码数据。反序列化只证明该反序列化器
  实施的检查。派生的 `Deserialize` 不得绕过领域值的受检构造；进入领域逻辑前，校验传输
  表示，或让自定义反序列化委托给同一个构造函数。
- 必需事实通过准入后不再可选。区分缺失、拒绝、不可用和效果结果未知；不要用貌似合理的
  默认值替代格式错误的输入。
- API 只检查数据时借用，保留或消费数据时取得所有权。根据语义和生命周期选择 `Clone`、
  共享所有权和内部可变性，不要仅为消除借用错误而使用它们。克隆句柄不授予另一个权威写者。
- 为实际能力或可替换行为引入 trait 和泛型抽象。不要求每个 struct 对应 trait、每个 module
  对应共享 crate，或每个 helper 都使用类型状态编码。保持能表达已接受不变量的最小设计。

## 数值与布局

- 传输、ABI 和持久化值使用固定宽度整数，内存长度和索引使用 `usize`。Rust 默认内存布局
  不是序列化格式。
- 无损转换优先使用 `From`，可能失败的转换使用 `TryFrom`。数值 `as` 转换需要局部证明，
  确保截断、符号变化或精度损失不会违反契约。不要通过转换把不可信长度或身份强行压入范围。
- 分配、索引或切片前检查长度、标签、范围和容量运算。溢出表示失败时使用 `checked_*`；
  只有截断到边界或模运算是规定行为时才使用 `saturating_*` 或 `wrapping_*`。身份、代次和
  序列耗尽必须有明确结果，不得静默回绕或复用值。
- 在名称和操作中区分字节数、Unicode 标量数和终端显示单元格宽度；字节边界不一定是合法
  文本边界。
- 外部 ABI 需要精确布局时，将其隔离在所属适配器，并验证大小、对齐和偏移。按协议权威
  显式序列化各字段。

## 错误与 panic 边界

- 领域和协议操作为预期拒绝与失败返回可匹配的错误变体。通过适配器转换和错误链保留机器
  可读原因；呈现文本不是判别值，不得驱动重试或恢复。
- I/O 边界的语义足够时使用 `io::Error`。只报告或终止的最外层应用边界可以聚合类型擦除
  错误；调用方必须区分领域结果的操作不得传播这类错误。
- 对外部输入或可恢复运行时状态，不使用 `unwrap`、`expect`、可能 panic 的索引或故意 panic。
  局部已证明的不变量可以支持断言或 `expect`，证明应体现在类型或邻近说明中。测试断言可以
  直接让测试失败。
- panic 表示内部不变量违反，而且只能终止进程设计承诺的所属失败域。定义线程或任务 panic
  如何传达给所有者；捕获 panic 或忽略 join 结果都不能证明恢复成功。
- 不得静默丢弃操作或清理失败。有意尽力而为的操作需要归属明确的理由，并在契约要求时
  提供可观察的失败处理。

## 资源与并发

- 每个文件、socket、进程、PTY attachment、任务、channel、timer 和临时产物只有一个生命周期
  所有者。使用 RAII guard 管理获取和部分初始化回滚。共享引用或克隆句柄不能免除唯一关闭
  所有者的要求。
- 清理可能失败或需要等待工作时，提供显式 close、shutdown 或 join 操作。定义适合该所有者
  的 `Drop` 兜底职责；它不得 panic 或隐藏无界阻塞等待。丢弃线程或任务句柄不证明工作已停止。
- 关闭先停止准入，再取消或排空有界工作、join 自有工作，并且每项资源只释放一次。取消本地
  等待不会撤回已提交的外部效果；保留其身份以及契约规定的对账或失败结果。
- 限制输入长度、解码集合、队列、重试、并发和每次分发的工作。每个上限都有可观察的拒绝、
  背压、截断或取消结果。
- 说明每把锁保护的不变量、多锁获取顺序以及锁中毒的失败策略。绝不跨 `.await`、IPC、
  文件系统或进程等待、回调或阻塞计算持有同步锁。将不可避免的阻塞工作与异步执行器隔离；
  截止时间不会让阻塞工作变成异步工作。

## 本地进程与 IPC 边界

以下规则适用于每个打开本地端点、连接本地端点或启动兄弟进程的 Rust 可执行文件。

| 关注点     | 契约                                                                                                                                                                                                                                                                  |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 进程退出   | `main` 返回 `ExitCode`，并把一个类型化结果映射到它。库、领域和适配器代码绝不调用 `process::exit`；失败作为值传到 `main`，以便所有者析构并执行恢复。                                                                                                                   |
| 对端身份   | 本地端点在读取协议字节之前，先把连接对端的操作系统凭据与预期的有效用户核对。知道端点路径不授予任何权限。                                                                                                                                                              |
| 运行时路径 | 运行时目录和 socket 派生自一个显式配置的 base，绝不来自当前目录、仓库或 `PATH`。创建时设置显式 mode；遍历不跟随符号链接，并检查所有者与 mode。                                                                                                                        |
| 路径缺陷   | 意外的所有者、mode、符号链接或文件类型是报告给调用方的类型化拒绝。进程绝不修复、删除或替换其他所有者的路径来继续执行。                                                                                                                                                |
| 兄弟启动   | 启动另一个仓库可执行文件的进程，先解析确切的预期 artifact 并在启动前验证其身份；它绝不运行 `PATH` 或当前目录碰巧解析到的东西。                                                                                                                                        |
| 信号       | 由一个所有者安装信号处理，并把终止信号转换为一个类型化的关闭事件。处理器除发出信号外不做任何工作；中断退出遵循惯例的 `128 + signal` 退出码。                                                                                                                          |
| 子进程     | 每个派生的子进程都有一个负责回收它的生命周期所有者。有意保留的子进程是显式、具名的所有权转移，并有文档化的存活契约，而不是被遗忘的句柄。                                                                                                                              |
| 环境       | 进程读取的变量命名为 `ORCHBEE_*`，由所属 workspace 记录，并在启动时校验。除此之外，只有为用户路径操作数显式声明的平台用户目录输入、[本地化产品文案](#本地化产品文案)中的 locale 变量和 [TUI 终端环境](tui/terminal-environment.md#能力探测)中的终端能力变量影响行为。 |

## Lint 策略与执行边界

Workspace policy 对自有代码使用 `unsafe_code = "forbid"`。局部 `allow` 或 `expect` 无法降低它。
如果无法用安全方式表达不可避免的操作系统或外部代码边界，需要单独接受的策略变更：将例外
收窄到一个适配器，记录每项安全不变量，并添加边界测试。此策略不审计第三方依赖内部的
unsafe 代码；依赖与 feature 评审仍然必要。

已接受的操作系统例外是 `crates/windows-console-host` 中的 `orchbee-windows-console-host`
package。该 crate 声明 `unsafe_code = "deny"`，而不继承 workspace `forbid`；只有其私有
`src/sys.rs` 模块带有说明理由的 `allow`，用于 safe API 无法表达的 Win32 call 与 tagged-union
read。Rust runner 固定 package、manifest、私有 module declaration 与一致的 Clippy policy；其他
member、location、module 或更宽的 lint escape 都会被拒绝。该 crate 的 safe module 与 test 仍为
deny。[package contract](../../crates/windows-console-host/README.zh-CN.md) 负责 handle、count 与
Unicode safety invariant 及其 boundary evidence。

`clippy::all` 是默认 lint 组，并非全部可用的 Clippy 检查。针对已证明的风险选择额外检查，并
区分生产不变量和刻意无效的测试输入。不要仅为增加 lint 数量而启用整个可选组。每次执行规则
变更都在固定工具链下同步更新权威、配置、自有诊断以及有意义的正向和负向回归证据。

Workspace 还拒绝 `clippy::wildcard_imports`，使普通生产模块导入保持显式。真实 Cargo runner
夹具会拒绝一个实际使用的模块通配导入，并接受改为显式导入、同时在测试模块中使用
`use super::*` 的同一源码。这项单独启用的检查比源码策略窄：Clippy 的默认豁免仍然存在，
包括 prelude、重新导出、宏展开及测试上下文；枚举变体通配导入属于另一项 lint。仍须评审这些
表面，不能声称所有通配导入或公共边界都已自动检查。

新增或变更的局部 lint 例外必须列出具体 lint，使用最小适用范围，并通过 `reason` 说明不变量或
取舍。告警可靠存在时优先使用 `#[expect(...)]`，以便诊断过期例外。解释依赖配置的 `allow`
用法；不得抑制整个模块的正确性检查或绕过仓库硬约束。`reason` 要求是评审义务：
`clippy::allow_attributes_without_reason` 未启用，启用它属于 lint 策略变更，且必须在同一变更中
修复每个现有例外。

| 关注点              | 证据与限制                                                                                                                 |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| 工具链与 Cargo 执行 | 根 runner 检查编译器固定版本和 locked Cargo metadata，再执行所选 workspace task。                                          |
| 编译器与 lint 诊断  | 成员继承 workspace lint；rustfmt 检查格式，Clippy 拒绝已启用的告警。这不证明全部源码策略义务。                             |
| 清单与架构策略      | 评审成员注册、lint 继承、依赖方向、版本与 feature 共享、嵌套 workspace 和路径边界。Cargo metadata 本身不检查这些仓库契约。 |
| 语义安全与测试价值  | 评审受检构造、不变量证明、取消、关闭和对应主张的证据；编译与测试文件数量无法判断它们。                                     |

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

## 本地化产品文案

原生产品文案——展示给人的诊断、帮助、标签和确认——与其他产品消息一样以 `en-US` 和
`zh-CN` 交付。前端目录遵循[前端本地化](frontend/localization.md)；原生可执行文件遵循以下规则。

- 每个可执行文件有一个 `localization` 模块，拥有全部面向人的字符串。领域、协议和适配器模块
  返回类型化结果，绝不格式化散文。
- 受支持的 locale 是一个封闭枚举。每条消息都是对该枚举的 `match`，没有通配分支，因此新增
  locale 在每条消息都翻译完成前无法编译。
- Locale 选择按 `LC_ALL`、`LC_MESSAGES`、`LANG` 的顺序读取，忽略空值，并在值缺失、非 Unicode
  或不受支持时回退到 `en-US`。繁体中文变体解析为 `zh-CN`。
- 机器可读输出绝不本地化：wire 字段、schema 标识符、枚举值、退出码和标识符在每种 locale 下
  保持一致。用户控制的文本作为数据携带，按所在表面转义，绝不翻译。
- 翻译精确保留命令、flag、路径、标识符和占位符，并使用目标语言的标点。测试按
  [测试](TESTING.md#本地化契约)在两种 locale 下渲染每条消息。

## 测试

遵循共享的[测试义务](TESTING.md#必需的单元测试)和[测试层次与成本](TESTING.md#测试层次与成本)。
一个行为套件可以覆盖多个 Rust 模块；新增源码文件或函数都不要求新增伴随测试文件。

- Unit-test body 放在相邻 test module 文件或 crate 的 `tests/` 树中。Production module 可以保留
  挂载该文件所需的窄化 `#[cfg(test)] mod tests;` 声明，但不放 inline scenario 或 fixture。
- 挂载的模块命名为 `tests`，位于模块旁的 `tests.rs`，或位于一个 `tests/` 目录中，由其 `tests.rs`
  声明按行为命名的子模块，例如 `mod backpressure;`。不要给生产文件挂载其他名称或缩写的测试
  模块。共享单元测试夹具作为同一个 `tests` 所有者的子模块，只提供测试使用者需要的可见性。
- 集成与 conformance 测试位于 crate 根的 `tests/` 树中，每个边界或 fixture 族一个文件；共享
  helper 位于 `tests/support/mod.rs`。
- 测试 public behavior、invalid 与 maximum input、partial failure 后的 cleanup、cancellation、
  stale identity、backpressure 和 deterministic shutdown。当 ordering 或 state-space 风险值得时，
  使用 property 或 model-based evidence。
- Integration test 使用 public crate 或 process boundary。Test-only visibility 不得扩大 production
  API 或引入 product behavior。
- 声称可编译或运行的 Rust 文档示例必须参与 doctest。不要用 `ignore` 隐藏错误示例；
  `compile_fail` 或 `no_run` 示例要说明其真实主张。
- Cross-compilation 只证明可编译。Runtime、IPC、PTY、installer 与 service-lifecycle 主张需要在
  每个声称支持的 operating-system family 上使用 native runner。

## 必需命令和发布输入

根 pnpm 门禁调用 `scripts/run-rust-workspace.mjs` 验证每个 Cargo member。测试任务分别运行
各 target 测试和库文档测试；`--all-targets` 不包含 doctest。`pnpm test:rust-runner` 使用隔离的
Cargo fixture 验证该边界、源模块文件名及自有 unsafe 代码策略，包括必须导致 runner 失败的情况。

| 仓库任务            | Cargo 证据                                                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm format:check` | `cargo fmt --all -- --check`                                                                                                    |
| `pnpm lint`         | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`                                                 |
| `pnpm build:full`   | 显式本机构建：`cargo build --workspace --all-targets --all-features --locked`                                                   |
| `pnpm test`         | `cargo test --workspace --all-targets --all-features --locked`，随后执行 `cargo test --workspace --doc --all-features --locked` |

Clippy 和 Cargo 测试自行编译输入；普通验证不预先执行完整 Cargo 构建。根测试门禁只准备
被选中的真实进程测试所需的 Host 和配套可执行文件。变更涉及的交付或平台契约需要此类证据时，
才显式构建或交叉编译，规则见[验证](VERIFICATION.md#合并门禁)。

`--all-features` 不证明每种受支持的 feature 组合或操作系统 target。引入 feature 或扩大支持
范围时，所属方声明受支持的默认、最小和发布配置，以及所需的编译和原生运行证据。
workspace 构建统一后的依赖 feature，不能证明独立交付的 package 仅凭自身声明的配置也能构建。

聚焦迭代可以使用按 package 选择的 Cargo 命令，但完成时使用根门禁。发布自动化通过固定工具链、
`--locked` 和 `--release` 构建确切 package 与 binary，再把已验证 artifact 暂存到所属 distribution
cohort。它绝不把 `target/` 下碰巧已有的文件视为 release input。Supported target、signing、
install path、update cohort 与 rollback 仍由每个已接受的应用发布切片负责。
