# Windows 原生插件

## 目录模型

原生插件目录属于用户共享配置，独立于 Site 生命周期。Windows 的 `plugin link PATH`、`unlink ID`、`enable ID`、`disable ID`、`list [ID]`、`config-dir ID` 和 `action list [ID]` 不启动 Site，直接使用此目录。链接仅登记清单，不执行 build 或 startup 命令。解除链接仅移除登记，保留源文件和用户配置/状态目录。

local-site-runtime 插件边界负责有界 TOML 校验和持久目录。目录参数优先解析 `orchbee-plugin.toml`，仅当其缺失时使用 `herdr-plugin.toml`；也可直接指定清单文件。导入 Herdr 元数据不代表兼容 Herdr 专用可执行文件或 API 调用。标识符去除首尾空白，区分大小写，最多 120 个 ASCII 字符；插件 ID 允许字母、数字、冒号、点、下划线和连字符，action ID 不允许点。参数保留为独立 argv 元素。未知字段、重复条目 ID、无效命令和无效平台名称均在登记前拒绝。

清单最多 128 KiB，目录最多 32 项且不超过 1 MiB。条目保留规范源路径、启用状态和最后一份有效清单。读取会刷新清单；源缺失或无效时保留检查元数据并明确警告，不能据此授权执行。条目平台列表优先于插件列表；均缺省时允许 Windows。其他平台标签仅为元数据。

## 持久化与转换

目录存储位于已准入的用户配置父目录下的 `plugins`；复用 Windows 私有缓存的祖先路径校验、独占写锁、落盘后原子替换和中断写入清理。只读操作不创建目录。修改在锁内重新读取，校验完整新目录，仅在实际变化时递增安全整数 revision。损坏、竞争和容量上限均返回错误，不当作空目录。解除不存在 ID 的链接不产生变化；启用不存在的 ID 失败。带前缀和哈希的 ID 路径组件使配置/状态目录稳定，并避开 Windows 保留名。解除链接后保留用户目录。

## 交付与证据

此目录是插件能力的第一组集成。action 执行和日志、托管安装/build、startup/event hook、链接处理器及插件 Pane 生命周期仍需运行时集成，之后才能认定插件完整对齐。目录接受不代表命令已执行。参照固定 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`，主要为 `cli/plugin.rs`、`app/api/plugins/manifest.rs` 和 `persist/plugin_registry.rs`。

定向证据覆盖解析上限、标识符/路径碰撞、源不可用时保留条目、缺失目录只读、目录原子转换及真实 Windows CLI 回执。测试夹具拥有并回收临时路径。完整 Windows 视觉/provider/多 Client 与全量回归验收另行执行。
