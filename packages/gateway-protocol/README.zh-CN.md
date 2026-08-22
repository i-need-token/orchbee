# @orchbee/gateway-protocol

Orchbee 本地开发 Browser Gateway 的严格、运行时无关契约。

## 适用范围

该包拥有协议版本、环回 WebSocket 路径、生命周期能力模式、Desktop-to-Utility 父消息、页面控制框架、附件二进制框架、URL 片段引导和传输安全限制。它不包含 WebSocket、Electron、Browser、Desktop Main、Terminal Host、Workbench、UI、持久性或特权效果实施。

`@orchbee/workbench-client-protocol` 仍然对页面会话、投影控制和投影终端消息具有权威性。此包仅对本地 Browser 传输的这些消息进行身份验证和框架。

## 渠道

| 边界                            | 合同                                                                                     |
| ------------------------------- | ---------------------------------------------------------------------------------------- |
| Desktop Main 到 Utility Process | 一条启动/停止流、限定作用域的页面打开/页面关闭消息，以及每个已接受页面恰好两个传输端口。 |
| Browser 控制 WebSocket          | 每页一个经过身份验证的连接，承载低速率 Workbench 客户端会话和投影控制消息。              |
| Browser 附件 WebSocket          | 每个可见的投影终端附件有一个经过身份验证的二进制连接，仅携带该附件的命令和事件。         |
| Browser 位置                    | Gateway 端口、协议版本和生命周期功能仅编码在 URL 片段中。                                |

控制端点为`/v1/control`；连接端点是`/v1/attachment`。两者都需要协议 `1.2`。 Gateway 仅接受 Desktop Main 提供的精确环回 HTTP Origin。

## 模型和不变量

| 关注     | 合同                                                                                                                                        |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | 为普通的 Browser 页面提供与 Electron 相同的客户端含义，而无需直接授予 Host 或应用程序生命周期权限。                                         |
| 输入     | 不受信任的片段值、WebSocket 消息、传输端口消息、二进制帧、套接字生命周期和传输队列状态。                                                    |
| 拥有数据 | 一种进程生命周期功能、新页面连接标识、一次性附件套接字标识和帧元数据。                                                                      |
| 输出     | 验证通用 Workbench 客户端消息、范围 Desktop 父请求、类型化故障和规范二进制帧。                                                              |
| 权威     | Desktop 主要拥有规范的 Workbench 和 TerminalPresentation 状态； Terminal Host 拥有 PTY/VT 状态； Browser 拥有 Client View 和 Browser 效果。 |
| 认证     | Main 创建一种 256 位生命周期功能。连接功能是针对一个套接字身份派生的，并且从不授权另一连接。                                                |
| 隔离     | 页面仅接收其客户端会话和投影端口。没有 Gateway 消息携带 Host 连接、绑定存储、生命周期端口、文件系统端口或通用 Main 功能。                   |
| 断开连接 | 关闭寻呼控制通道会停用该寻呼会话及其附件，而不会更改共享拓扑或终端生命周期。                                                                |
| 背压     | 有界附件队列在空时接纳一个最大尺寸的有效帧，并在溢出时仅收回确切的附件。重新打开投影会创建新的附件和完整的快照。                            |
| 基数     | 页面、终端或附件不存在产品数量上限。字节和缓冲传输限制会拒绝受影响的消息或连接，而不是驱逐另一个客户端。                                    |

协议 `1.2` 在释放后保留投影请求映射，直到 Desktop 返回其相关的释放确认或清理失败。这使得 Gateway 可以转发延迟的清理结果，而无需将其视为外部控制流量或停用另一个附件。

嵌套的 Workbench 投影打开命令携带视觉表面的完整测量 `TerminalGeometry`。重复的 Gateway 请求 ID 仅当其表示身份和几何形状都与保留的请求匹配时才有效；改变的几何形状与控制流量发生冲突。 Gateway 不变地转发该值，并且从不提供启动默认值。

页面生命周期为`awaiting-authentication -> opening-client -> active -> closed`。身份验证是唯一可能请求 Desktop 页面端口的转换。已关闭页面的后期端口束被处置。附件注册遵循`registered -> authenticated -> retired`；它无法恢复或反弹。 Gateway shutdown 终止仍在等待附件身份验证的套接字。

该协议是现有 Desktop 多客户端会话和需求附件模型的传输改进。它没有添加新的产品生命周期状态或规范文档转换，因此没有单独的 TLA+ 模型对此包具有权威性。确定性的生命周期跟踪、严格的编解码器测试和真实的 Electron-plus-two-Browser 系统跟踪提供了传输证据。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/gateway-protocol build
pnpm --filter @orchbee/gateway-protocol typecheck
pnpm --filter @orchbee/gateway-protocol test
pnpm --filter @orchbee/gateway-protocol test:exports
```
