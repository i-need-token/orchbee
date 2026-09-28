# Agent 重启引用

## 归属与范围

Windows 干净停止检查点按稳定的 Workbench Presentation 身份保留外部 provider 会话引用。这是重启意图，不是 Orchbee 规范 Agent 会话、存活进程、名称绑定、来源权威，也不授权重放 hook。既有终端上报所有者提供引用；检查点编解码器在任何运行时启动前校验它。

固定参考为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `persist/snapshot.rs` 和 `terminal/state.rs`：优先采用当前权威的 provider 引用，否则采用保留的 provider 引用。已清除或释放的引用不能从旧检查点种子重新出现。主 Presentation 和子 Presentation 各有独立身份与引用。

## 存储与转换模型

检查点版本 2 增加必需的 `agentSessions` 列表。每条记录携带 `presentationId` 和 `session`，后者包含精确官方来源、规范化 Agent 标签及 provider 引用种类／值。版本 1 仍可读取，但不允许此字段。未知字段、重复记录、成员缺失、不支持的版本、错误来源／Agent 组合、控制字符、引用越界或非绝对本机路径均拒绝整个检查点。沿用既有 2 MiB 编码上限。

| 转换 | 必需观察 | 结果 |
| --- | --- | --- |
| 捕获实时引用 | 已提交的 Presentation 与精确保留的 Worker 代际／Host 身份 | 仅保存该上报所有者的 provider 引用 |
| 捕获待处理重启意图 | Presentation 仍保留，且没有取代它的上报所有者 | 保留已准入引用，不虚构进程状态 |
| 解析并重建 | 完整有效检查点和新 Site／Worker | 重建未绑定拓扑及独立的待处理引用 |
| 准备新绑定 | 精确且未变化的 Presentation 成员与引用 | 在类型化恢复目标中携带引用 |
| 物理创建后播种 | 当前逻辑绑定及唯一保留的新物理身份 | 仅将保留引用播种到尚为空的上报所有者 |
| 上报所有者已经存在 | 当前物理上报所有者具有权威 | 消费待处理种子，不替换当前状态 |
| 播种后清除或退出 | 普通上报转换 | 后续捕获反映当前状态，包括缺失 |

播种先准备再修改，遵守上报资源／修订上限，且只推进上报修订。它不设置识别 Agent、回退状态、权威、序号水位、抑制历史或规范 Agent 观察。存储引用不含可执行路径、shell 命令、环境或凭据。

## Windows 恢复启动

尚未发布的 Site 按 Presentation 遍历顺序，为每个不同的官方来源、Agent、引用种类和值准备一次恢复启动。重复 Presentation 启动普通 shell，不保留 provider 引用。禁用恢复时保留引用并启动普通 shell。ORCHBEE_RESUME_AGENTS_ON_RESTORE 只接受精确的 true 或 false，默认为 true。Client 激活在持久 Active 发布或标记提交前拒绝非法值；已有 Site 的准入不受影响。Server 在 Worker 启动前独立验证。修正非法值后可以再次激活，不需要恢复操作，也不丢失检查点。此启动输入不实现独立的 config.toml 会话设置或热重载。

Server 选择当前 Windows provider 命令及字面恢复参数，然后在既有终端创建事务内，用 -NoLogo -NoProfile -NoExit -EncodedCommand 启动系统 Windows PowerShell。共享字面编码支持原生程序、.cmd/.bat 和 .ps1。程序发现仅接受应用程序和外部脚本，不接受函数或别名。provider 退出或解析失败后 shell 仍可用；shell 存在不证明 provider 就绪。

| 阶段 | 守卫与结果 |
| --- | --- |
| 规划 | Host 副作用前验证全部启动并保留不同引用 |
| 分派 | 每个已准备终端在既有创建身份下仅提交一次 |
| 完成或结果未知 | 结算终端记录，不重放 |
| 新物理绑定 | 仅播种引用；新观察建立 Agent 状态 |
| 重复引用 | 保留拓扑，启动 shell 并省略引用 |
| provider 失败或退出 | 保留终端及其实际错误／输出 |

Workbench/OBCS/OBCN 启动选择接受 executable/argument 合计 30 KiB UTF-8 字节和 128 个参数，另有 30 KiB 最坏 JSON 字符串内容预算：ASCII 控制字符计六字节，引号／反斜杠计两字节，其他 UTF-8 字节计一字节。OBCS JSON 帧在既有 u16 头内允许 63 KiB；OBCN 二进制帧允许 40 KiB，Workbench 启动请求允许 96 KiB。独立的 Host 64 KiB 控制文档及 Windows 命令行限制仍适用。编码后的恢复参数为 ASCII，限制为 30,000 字符；batch 分派拒绝超过 8,000 字符的命令行。旧 peer 拒绝更大的 payload；必须整体启用及回退匹配 cohort。

已知创建失败和结果未知均保守地保留预留。Herdr 在可证明尚未启动进程时尝试另一个重复面板的行为仍未实现。实际 cwd、配置文件恢复开关、名称和托管状态恢复仍是必需集成工作。

## 证据与兼容性

测试覆盖官方 ID／路径引用、主／子隔离、精确物理代际准入、畸形及重复记录、成员删除、v1／v2 兼容、有界编码、新身份播种、候选回滚和清除后不复活。Windows 验收必须执行实际干净停止、新 Site 重启及公开上报读取；存储和播种证据不能证明 provider 已恢复或通过认证。

旧二进制显式拒绝版本 2。版本 1 文件可在下次成功干净停止时升级；捕获／发布失败保留之前的已提交文件。部署匹配 cohort。不得删除不支持的检查点来使启动成功。完成 Windows 对齐目标前，仍需全量回归及真实 provider 恢复验收。
