# Windows 外层终端标题

`terminal title set <title>` 和 `terminal title clear` 向已有 Site 提交一次变更，均接受 `--format text|json`。Set 移除控制字符、截取至多 200 个 Unicode 标量再 trim；结果为空则在提交前失败，内部空格保留。不创建 Site 或终端。

## 状态与投递

Server 拥有一个临时覆盖值及单调修订，与 Panel 标签独立。没有 Client 时 set/clear 仍发布；重新连接的 Client 获得最新状态。Clear 目前发布应用回退标题 `Orchbee`。首次发布前保持宿主标题。完整 `ui.window_title` 模板和重启/live-handoff 转移仍属后续对齐工作。

每个认证监听连接拥有一个槽位（至多 32 个，五秒心跳期限）。新连接或焦点从 false 变 true 时提升该连接；空闲轮询不抢前台。断连/过期时选择最近的剩余连接。只有该前台接收发布；这里依据 Client 上报焦点，不查询 OS 焦点。Client 提升为前台时重新应用当前标题；空闲重复投递被去重，降为后台时丢弃未绘制状态。最新快照替换未绘制旧状态，不保留无界事件队列。最多保留 256 个请求回执、期限 60 秒；精确重放只返回原回执，不再发布；身份冲突拒绝；容量耗尽在提交前拒绝。`changed` 表示提交时前台可接收，不确认物理标题。无前台时发布仍已提交，回执明确报告。已提交但丢失响应时结果未知，不自动重试。

## Windows 生命周期

独立、可取消、两秒超时的交换每 100 毫秒轮询。线程拥有传输及单个最新交接槽；只有 UI 所有者请求生命周期写入。关闭时取消阻塞的接入/轮询，join 后释放资源。标题变更不改变输入、选择、草稿或布局。

生命周期在首次修改前捕获继承控制台标题，再使用安全 Windows 标题 API。`win32console` 0.1.5 仅为捕获/写入能力引入：标准库和既有安全绑定没有捕获接口；无构建脚本，使用既有 winapi 版本。适配器检查固定 UTF-16 缓冲的填充及截断歧义；原始标题达到 259 个 UTF-16 单元或含无效 UTF-16 时在标题变更前拒绝捕获。Last-error 为零用于识别合法空标题。恢复使用精确原文，不使用展示转义文本。既有有界恢复 lease 覆盖正常退出、错误及所有者线程 panic，失败后继续尝试其余清理，并阻止恢复后的标题写入。OS/宿主设置仍可能忽略标题变更。

## 契约与证据

匹配私有 cohort 新增 OBWP kind 43/44，整体升级/回退。`orchbee.terminal.title` v1 是封闭公开回执，无新增持久化迁移。Herdr 参照为 `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `cli.rs`、`config/window_title.rs` 和 `server/headless.rs`；入队不等于显示确认。[Windows 标题 API](https://learn.microsoft.com/en-us/windows/console/setconsoletitle) 负责物理宿主行为。

定向检查覆盖 Unicode/控制字符边界、重放/焦点/过期/容量轨迹、独立 Rust/TypeScript/JSON 一致性、真实 Windows CLI 管道回执、取消、生命周期恢复/失败及语法/帮助/补全。这些证据不替代最终 provider、多 Client、物理终端或全量回归验收。
