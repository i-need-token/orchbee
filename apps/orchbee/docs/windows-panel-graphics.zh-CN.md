# Windows Panel 图形 CLI

## 命令与输入

`panel graphics set PANEL --spec SPEC`、`clear PANEL [--spec SPEC]`、`info PANEL [--client ID]` 和 `stream PANEL [--spec SPEC]` 加入已有本地 Windows Session。四项均支持 `--presentation ID`、`--timeout MS` 和 `--format text|json`。省略 Presentation 选择当前主项；明确 ID 必须属于该 Panel。命令不会启动 Session、改变焦点、注入终端输入或重放结果未知的写入。

Set SPEC 是封闭 JSON 对象，含 `image` 及可选 `layerId`（默认 `primary`）、`zIndex`（默认零）。图片采用共享 camelCase PNG/RGB/RGBA/BGRA 帧，含尺寸、规范 `dataBase64` 和 placement。`@PATH` 有界读取包含对象或 PNG 的普通文件；`-` 读取非交互继承管道或普通 stdin。在 PNG 模式下使用实际 IHDR 尺寸及默认图层/位置。输入在 Session 发现之前读取、提交之前校验。单次原始图片最多 512 KiB；编码 JSON 文档最多 768 KiB。内联 argv 保持已有 64 KiB 参数预算。Clear SPEC 只允许可选 `layerId`；stream SPEC 还允许 `zIndex`；info 不允许 SPEC。

Stream 读取 LF 结尾 JSON 头，再读取恰好 `dataLength` 原始字节。头字段为 `format`、`width`、`height`、`dataLength`，可选 `sequence`、`revision`、`placement`、`damage`；全部头字段和位置字段使用 camelCase。Placement 接纳 `viewportColumn`、`viewportRow`、`gridColumns` 和 `gridRows`。Damage 接受单个矩形或最多 64 个矩形的数组。参照浏览器的 `transport: "direct-kitty"` 字段作为忽略的兼容提示被接受，不作为能力证据或渲染器选择。头最多 64 KiB，每帧主体最多 16 MiB。省略 sequence 递增最后已受理序号；明确 sequence 必须递增，可用十进制字符串或安全 JSON 整数。Revision 支持完整宽度十进制字符串或安全 JSON 整数，默认零。Damage 对照完整提供的帧校验。空头忽略；截断头/主体、重复/未知字段、无效尺寸或过期序号会结束生产者。托管 Windows 的 stream-open 回执公布私有生产者目录。文件头用 `file: {"path":"<directory>/frame.rgba"}` 替代 `dataLength` 和主体，仅接受 RGBA/BGRA。文件必须是目录的精确直接子文件，并具有声明的完整像素长度；提交前关闭写入句柄。Server 持有根目录租约时复制并归一化完整文件。非托管 endpoint 模式不公布目录，任意路径不会成为 Server 读取入口。

Set/clear/info 默认总截止时间五秒；stream 不指定时没有总截止，但单帧读取的空闲/总预算为 30 秒。每次交换与 stdout 记录另有界限。固定的隐藏 Windows 字节中继只转交继承字节；调用拥有者在所有退出路径 kill 并 wait。辅助程序不解码图片，不执行调用者文字。Ctrl+C 停止输入并关闭生产者；EOF 仅关闭本连接的流并释放图层。静态图层在上传 CLI 退出后保留。

## 回执与失败

单次 JSON 使用 `orchbee.panel.graphics` v1；流式 NDJSON 使用 `orchbee.panel.graphics.stream` v1。相关 Server 结果保留目标、资源版本、stream ID、sequence 和生产者 revision。安装与 Client 可见性及实际绘制分开。物理/能力事实在真实 Client 发布前保持未知空值。

已提交写入若回复缺失或无效，返回明确未知回执及退出码 8；不会报告为成功取消或重试。已知拒绝为 3，来源不可用为 4，传输/协议失败为 5，截止超时为 7，取消为 130。流结束保留最后实际回执及清理结果，区分确认关闭、连接结束清理和清理未知。stdout 记录阻塞或只写入部分时，不追加虚构的结束记录。

## 归属与验收

共享契约和 Session 拥有者见 [Windows Panel 图形](../../orchbee-server/docs/windows-graphics.zh-CN.md)。图层、完整帧暂存、来源栅栏、预算及 EOF 归属复用该规范。CLI 和 TUI 消费者在同一独立分支实现，之后进行 Windows 组合验收；协议/领域测试与命令存在不能单独将 R078–R081 判为对齐。macOS、Linux 运行时、TLA 和手动抢焦点 Desktop 测试仍不属于本次工作。

## 当前实现与定向证据

四个原生命令、帮助、补全、严格输入、相关回执与 EOF 清理已实现，使用已有只读/导航授权。真实 Windows CLI 命名管道测试覆盖 15 个交换/流场景及输入先于 Session 发现；三端基础图形契约仍由共享拥有者负责。原生字典 v1 包含 120 份文档，新增两种输出及独立图形失败规范。原生 TUI 已通过独立读取连接消费完整资源、绘制裁剪字符栅格并发布经认证的绘制事实，见 [原生 Panel 图形渲染](native-panel-graphics.zh-CN.md)。私有文件帧租约、真实文件头 CLI 管道及对外部／遍历路径的提前拒绝已实现；物理像素传输及最终组合／全量验收仍待完成。定向证据不替代该验收。空闲且没有未完成帧时不耗用单帧截止预算。

<!-- documentation-pairing: english-sha256=041810d7381fd8c553400223003ff9628acd9c239da26d1fbfeeae357d987a22 zh-cn-sha256=dd7a1b1e62b969c8aa7050a75bd5b70f3565fa2f9e60af8370b86c2e878b89bc -->
