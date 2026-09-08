# Desktop 终端设置

## 所有权与范围

Electron Main 拥有设备本地的终端默认值记录，也是唯一拥有 `SetTerminalDefaults` 权限的
组件。Web 渲染器只接收狭窄、可关联的设置能力；它不会获得 Host 连接、存储路径、通用
文件系统能力，也不能更新正在运行的终端。Gateway 和普通 Browser 客户端不会收到此能力。

当前记录只包含 `scrollbackBytes`，取值为 `0` 到 `1_000_000_000` 之间的整数。产品默认值为
十进制 `10_000_000` 字节。Main 将严格的版本 1 记录存储在 Electron `userData` 下的
`terminal-default-settings-v1.json` 中。

## 启动与恢复

Main 在终端运行时初始化前读取记录，并把选定值提交给单例 Terminal Host。有效的存储值
优先于运行时默认值。存储缺失时使用运行时默认值且不写入。存储无效或不可读时也使用运行时
默认值，保留现有文件，并向“设置”投影一个可恢复问题。

Host 拒绝或无法确认选定值时，启动会失败关闭。在 Host 与内存快照一致之前，Main 不会公开
渲染器能力，也不会创建终端运行时。

## 更新事务

一个渲染器文档最多只能有一个设置请求正在执行。Main 串行处理已接纳的更新，验证完整配置，
先要求 Host 提交，再等待以所有者权限原子替换记录。持久化尝试成功或失败后，Main 才发布
新的会话快照。持久化尚未完成时，Host 已拥有新默认值，而 Desktop 快照仍保留旧值；
此时创建的新终端会捕获 Host 的值。

| 失败点                  | 会话值 | 存储值 | 渲染器结果                               |
| ----------------------- | ------ | ------ | ---------------------------------------- |
| 无效命令                | 不变   | 不变   | 能力退役                                 |
| Host 拒绝或控制不可用   | 不变   | 不变   | 携带之前快照的失败结果                   |
| Host 接受后存储写入失败 | 新值   | 旧值   | 完成，带 `write-failed` 和来源 `session` |
| Host 与存储都成功       | 新值   | 新值   | 完成，来源为 `stored`                    |

再次提交相同的会话值会重复 Host 预提交与存储尝试，因此短暂写入故障无需更换值即可收敛。

## 终端生命周期

终端启动保持 `configuration: null`，因此 Terminal Host 会在创建新终端时捕获当前默认值。
更改 `scrollbackBytes` 只影响未来终端。Main 不会发送 `UpdateTerminalConfiguration`、驱逐已
保留行、重建附件或截断正在运行的终端。恢复的终端保留其现有 Host 会话所拥有的配置。

## 渲染器能力

Desktop bootstrap 的第十个端口是 `terminal-default-settings`。它支持一个快照请求和一个
完整更新请求，两者都通过有界请求标识关联。格式错误的数据或转移端口会让能力退役。替换
渲染器文档会处置旧 broker，而 Main 拥有的控制器与 Host 默认值会跨页面替换继续存在。

只有此 Desktop 能力初始化成功时，“设置”界面才显示该控件。缺少该能力时，共享的
Browser/Gateway 界面会继续把 `terminal.scrollback` 明确标为计划项。

## 验证

从仓库根目录运行：

```sh
pnpm exec vitest run apps/desktop/src/main/terminal-default-settings/__tests__
node scripts/tla/run-models.mjs --models desktop-terminal-default-settings
```

`DesktopTerminalDefaultSettings` 检查 Host 优先提交、Host 拒绝时保持状态、持久化失败时会话值
仍可见、未来终端捕获、不实时驱逐、渲染器重载和处置。

模型使用两个配置值，并独立于请求阶段槽位统计已接纳、尚未完成的更新，因此删除串行化守卫
会违反 `AtMostOneMutation`。`HostAndSessionAgree` 在请求完成后成立；等待持久化的阶段明确
允许 Host 与 Desktop 值不同。一个保留终端可以从任一配置值开始，并保持该值。此抽象检查
配置保持，不证明原生历史行保留。模型覆盖成功启动与明确的 Host 提交前拒绝；控制器测试
覆盖启动失败。控制不可用或提交后的回复丢失不能证明 Host 未改变；原生请求结果模型与
传输测试负责该交付边界。
