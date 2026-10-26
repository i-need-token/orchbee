# 自动 Shell Integration

## 集成范围

Orchbee 为新的本地 Desktop 终端从配置的 executable 识别 Zsh、Bash 和 Fish，只改变子进程 launch，不编辑启动文件便加载已有应用拥有的 OSC 133 与 OSC 7 资源，并恢复正常 Shell 启动路径；无法安全处理时跳过注入。本契约不包含光标／标题变化、补全、sudo 或 PATH 包装、命令传输、Shell 克隆、远程或 SSH 注入、新用户检测、全局 Zsh 环境探测、任意 Bash 调用或其他 Shell。

## 问题与范围

提示符导航、上一条命令输出、当前目录投影与完成提醒都需要语义 Shell 标记。要求每位 Desktop 用户
把 `source` 命令复制到启动文件，会让这些常用终端工作流默认缺失，并让设置耦合到用户管理的配置。

因此，Desktop Main 会为受支持的本地交互式 POSIX Shell 细化已经选定的 launch profile。该细化只
应用于 profile 解析后新建的 terminal binding。它不会修改启动文件、在 Main 中执行设置命令、改变
运行中或恢复的终端、修改选定 executable、向 Windows 或远程连接注入，也不会从终端输出推断 Shell。

## 输入、数据与输出

`AutomaticShellIntegrationConfiguration` 是可信的 Desktop 启动数据：

- `directory` 是代码解析的应用资源目录；环境中的值不能替换它；
- `availableShells` 只包含 Desktop 启动时已确认自动 loader 与共享 Orchbee 集成脚本可读的 Shell；
- `inheritedEnvironment` 在独立启动的 Terminal Host 成为进程父级之前，捕获
  `XDG_DATA_DIRS` 与 `ZDOTDIR` 的存在性和精确值。

主要纯操作是 `resolveAutomaticShellIntegration(launch, platform, configuration)`。在 launch
profile 选择 executable 与字面 arguments 后，`launch` 由 `TerminalLaunchSchema` 准入。结果恰好为：

- `injected`，携带完整且通过 schema 的 launch 与检测到的 Shell；
- `unchanged`，携带原始 launch 与一个稳定原因：不支持的平台、缺失资源、不支持的 executable、
  不支持的 arguments、不安全的资源路径或无效 candidate。

unchanged 结果是有意 no-op。终端使用原始 launch 继续创建，且
`ORCHBEE_SHELL_INTEGRATION_DIR` 继续让现有手动回退可发现。该操作不依赖存储、文件系统、进程、
transport、时间或 UI。

## 决策模型

Shell 身份是选定 executable 的精确 POSIX basename。不同 basename 的符号链接或 wrapper 不会被
猜测。resolver 只准入其启动语义由本契约拥有的交互式 launch 形式：

| Shell | 准入的 arguments                                                                                | 细化                                                                                                       |
| ----- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Zsh   | 空，或只含 `-i`、`--interactive`、`-l`、`--login` 与组合的 `-il` / `-li` flag                   | 保留 arguments；让 `ZDOTDIR` 指向自动 loader，并用一个私有 marker 携带精确旧值                             |
| Fish  | 与 Zsh 相同的有界 interactive/login 形式                                                        | 保留 arguments；把自动 Fish data directory 前置到 `XDG_DATA_DIRS`，并用一个私有 marker 携带精确旧存在性/值 |
| Bash  | 空或 `-i` / `--interactive`，可选 `--noprofile` 及一个完整的 `--rcfile` 或 `--init-file` 参数对 | 用自动 loader 替换用户 rc-file 选择，保留准入 flag，并用一个私有 marker 携带精确选定的用户 rc 文件         |

Bash 拒绝 login 模式、`--norc`、`-c`、`-s`、`--posix`、位置脚本、不完整的 option pair 与未知
option。Zsh 和 Fish 拒绝表格之外的每个 argument。包含 POSIX list separator 的 Fish 资源路径
不安全，并保持 launch 不变。candidate 构建是全有或全无的：协议边界失败会返回完全相同的原始
launch。

## Loader 契约

自动 loader 是应用资源，绝不会变成用户配置：

1. Zsh `.zshenv` 恢复或取消设置原始 `ZDOTDIR`，按 Zsh 的缺席语义 source 用户原始
   `.zshenv`，并且只为交互式 Shell source `orchbee.zsh`。随后正常 Zsh 启动使用恢复后的目录继续。
2. Fish vendor 配置在为交互式 Shell source `orchbee.fish` 前，恢复 `XDG_DATA_DIRS` 的精确原始
   存在性/值。临时 data-directory 条目不能泄漏到后续命令或嵌套 Shell。
3. Bash 自己先执行正常的非 login 交互式系统启动，再把自动 loader 选作唯一 rc 文件。loader 清除
   marker，source 精确的用户选定 rc 文件或正常的可读 `~/.bashrc`，最后 source
   `orchbee.bash`。它绝不改变 POSIX、login、`ENV` 或 `HISTFILE` 语义。低于 4.4 的 Bash 保留正常
   启动行为，同时共享脚本报告现有类型化 unsupported 状态且不安装 hook。

每个 loader 都会在返回前清除其私有 marker。共享脚本保持幂等，因此现有的手动 `source` 行不会安装
重复 hook。运行时 loader 失败可能让集成不可用，但它必须先恢复 Shell 拥有的环境与启动路径，再尝试
可选的 Orchbee 脚本。

## 不变量、所有权与生命周期

- launch profile 仍是 executable 与字面用户 arguments 的唯一所有者；自动集成是 profile 之后的
  Desktop 细化。
- Desktop runtime composition 拥有可信资源准入与继承环境捕获。纯 resolver 是自动 launch delta
  的唯一 writer。
- Terminal Host 接收一个完整解析的 launch，并继续拥有 PTY/进程。它不检测 Shell 或 source 集成。
- 新 terminal binding 在第一次进程副作用前捕获细化后的 launch。重试、renderer 替换、Main 重启、
  profile 编辑与恢复都会复用该捕获 launch。
- 不支持和 preparation 失败绝不会改变 executable、arguments、environment、CWD、geometry、
  terminal configuration 或 terminal profile。
- 自动注入绝不会在 Main 中读取、创建、编辑、备份或删除用户启动文件。只有被启动的 Shell 会在正常
  初始化期间读取它本来拥有的同一批启动文件。
- 命令文本、输出、目录、环境值或集成 marker 都不会跨入 Web settings capability 或产品诊断。

自动集成没有长期状态机。启动执行只读资源准入，profile 解析执行一次确定性细化，binding 创建提交
完整 launch，子 loader 消费并清除其一次性环境 marker。

## 验证与可追溯性

| 声明                                                                     | 证据                                                 | 限制                                                                 |
| ------------------------------------------------------------------------ | ---------------------------------------------------- | -------------------------------------------------------------------- |
| Shell 与 argument 准入、精确环境恢复、全有或全无回退及边界               | 确定性自动集成 resolver 测试                         | 合成的可信配置与 launch 值                                           |
| profile 选择先于细化；不支持的 profile 保持不变；捕获的 binding 保持稳定 | launch-profile controller 与 terminal-lifecycle 测试 | 模拟的 Host 进程边界                                                 |
| loader 资源保留启动行为、清除 marker、保持幂等并发出 OSC 133/7           | 在可用时执行本地已安装 Shell 的资源测试              | 测试操作系统安装的原生 Shell 版本                                    |
| 新 Desktop Zsh 终端无需手动设置便收到标记，同时保留用户启动状态          | `terminal-parity/SI-03-automatic-shell-integration`  | macOS Zsh、真实 Electron/PTY/Terminal Host；无全局 `zshenv` override |
| Settings 用两种 locale 解释自动范围与手动回退                            | i18n 与 Settings component 测试                      | 仅 presentation；不证明子进程 launch                                 |

未选择 TLA+ 模型，因为细化没有可变状态、并发、异步完成、重试排序或取消。未选择 Lean 证明，因为
schema 验证与有限的 Shell/argument 决策表会被直接执行。真实 Shell 与 Desktop trace 是有界的平台
证据，不声明覆盖每个 Shell 版本、启动插件、系统级配置、符号链接 executable 或损坏的安装。
