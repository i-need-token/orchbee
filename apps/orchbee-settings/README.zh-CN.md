# 设置桥接

## 职责

`orchbee-settings --stdio-v1` 从已关闭的标准输入读取一个有界 JSON 请求，输出一个 JSON 响应。
它不会启动 Site，也不接受请求提供的路径或命令。Desktop 负责精确的可执行文件身份、两秒交换
期限、取消和进程回收。该程序没有交互模式或面向人的本地化输出。退出码 0 对应快照；退出码 1
对应拒绝、结果未知或输出失败。缺失或畸形输出表示结果未知，绝不构成重放写入的许可。

[共享偏好契约](../../schemas/workbench/README.zh-CN.md)负责值与传输语义。
[`orchbee-local-site-runtime`](../../crates/local-site-runtime/README.zh-CN.md)负责配置作用域内的
存储与原生初始来源准入。原生 Site 直接调用同一个所有者。

独立的 `--system-shell-v1` 模式只接受已关闭输入的空 JSON 对象，
返回一个 JSON 可执行文件路径或 null。它调用共享原生解析器，不启动 Shell。
Desktop 为每个新终端解析系统 Shell；自定义配置绕过发现。

## 配置作用域

显式提供 `ORCHBEE_CONFIG_FILE` 时，以其冻结作用域；否则采用平台用户配置位置：
`%APPDATA%/Orchbee/config.toml`、`$XDG_CONFIG_HOME/Orchbee/config.toml`（回退到 `$HOME/.config`），
或 macOS 的 `$HOME/Library/Application Support/Orchbee/config.toml`。配置文件名派生出独立的
私有目录；同一父目录内的两个显式配置文件仍然隔离。Windows 原生 Shell 迁移可以通过 `PATH`
解析显式配置的可执行文件，但绝不执行它。任何 Workspace、Site、远程设备或浏览器 origin 都
不会被扩大到此设备作用域。

## 维护

使用 `cargo build --locked -p orchbee-settings` 构建。契约测试归共享协议所有，存储测试归运行时
所有者所有，进程边界测试位于此应用的 `tests/`。请求、值或失败类型的变更必须同步更新两种语言
绑定及共享一致性夹具。Desktop 的 build-settings-bridge.mjs 以 release 模式构建显式原生 Rust 宿主目标，
在 out/settings-bridge 暂存可执行文件与 SHA-256 清单，打包复制完整资源目录。
开发编译构建相同的匹配产物。
