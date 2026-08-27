# @orchbee/gateway-protocol

Orchbee 本地开发 Browser Gateway 的严格、运行时无关契约。

## 适用范围

该包负责协议版本、环回 WebSocket 路径、一次性附件 capability schema、Desktop-to-Utility 父消息、页面控制 framing、附件二进制 framing、非秘密 URL query bootstrap 和传输安全限制。它不包含 WebSocket、Electron、Browser、Desktop Main、Terminal Host、Workbench、UI、持久化、Cookie 或特权 effect 实现。人类 Browser 鉴权与签名 session Cookie 契约属于 `@orchbee/remote-client-protocol`。

`@orchbee/workbench-client-protocol` 仍然对页面会话、投影控制和投影终端消息具有权威性。此包仅对本地 Browser 传输的这些消息进行身份验证和框架。

## 渠道

| 边界                            | 合同                                                                                     |
| ------------------------------- | ---------------------------------------------------------------------------------------- |
| Desktop Main 到 Utility Process | 一条启动/停止流、限定作用域的页面打开/页面关闭消息，以及每个已接受页面恰好两个传输端口。 |
| Browser 控制 WebSocket          | 每页一个经过身份验证的连接，承载低速率 Workbench 客户端会话和投影控制消息。              |
| Browser 附件 WebSocket          | 每个可见的投影终端附件有一个经过身份验证的二进制连接，仅携带该附件的命令和事件。         |
| Browser 位置                    | Gateway 端口与协议版本在 URL query 中分别精确编码一次；不含 credential。                 |

控制端点为 `/v1/control`；附件端点为 `/v1/attachment`。两者都要求协议 `1.3`。Gateway 只接受 Desktop Main 提供的精确 renderer HTTP Origin；其 runtime adapter 还会在接受 control WebSocket 前要求签名 Browser session Cookie。

## 模型和不变量

| 关注     | 合同                                                                                                                                                                                    |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | 为普通的 Browser 页面提供与 Electron 相同的客户端含义，而无需直接授予 Host 或应用程序生命周期权限。                                                                                     |
| 输入     | 不受信任的 query 值、WebSocket 消息、传输端口消息、二进制帧、套接字生命周期和传输队列状态。                                                                                             |
| 拥有数据 | 新页面连接 identity、一次性附件 socket identity 与 capability，以及 framing metadata。                                                                                                  |
| 输出     | 验证通用 Workbench 客户端消息、范围 Desktop 父请求、类型化故障和规范二进制帧。                                                                                                          |
| 权威     | Desktop 主要拥有规范的 Workbench 和 TerminalPresentation 状态； Terminal Host 拥有 PTY/VT 状态； Browser 拥有 Client View 和 Browser 效果。                                             |
| 认证     | Main 向 Server 与 Gateway 共享一个仅当前 activation 有效的 Browser session 签名密钥。Cookie 校验先于 control Upgrade；附件 capability 只为一个 socket identity 派生，绝不授权其他附件。 |
| 隔离     | 页面仅接收其客户端会话和投影端口。没有 Gateway 消息携带 Host 连接、绑定存储、生命周期端口、文件系统端口或通用 Main 功能。                                                               |
| 断开连接 | 关闭寻呼控制通道会停用该寻呼会话及其附件，而不会更改共享拓扑或终端生命周期。                                                                                                            |
| 背压     | 有界附件队列在空时接纳一个最大尺寸的有效帧，并在溢出时仅收回确切的附件。重新打开投影会创建新的附件和完整的快照。                                                                        |
| 基数     | 页面、终端或附件不存在产品数量上限。字节和缓冲传输限制会拒绝受影响的消息或连接，而不是驱逐另一个客户端。                                                                                |
| 活性     | 每个已接受套接字都有 10 秒认证期限。已认证套接字每 30 秒收到一次 ping，并在一个周期未响应后被终止。                                                                                     |

协议 `1.3` 在释放后保留投影请求映射，直到 Desktop 返回其相关的释放确认或清理失败。这使 Gateway 可以转发延迟的清理结果，而无需将其视为外部控制流量或停用另一个附件。

嵌套的 Workbench 投影打开命令携带视觉表面的完整测量 `TerminalGeometry`。重复的 Gateway 请求 ID 仅当其表示身份和几何形状都与保留的请求匹配时才有效；改变的几何形状与控制流量发生冲突。 Gateway 不变地转发该值，并且从不提供启动默认值。

页面生命周期为 `awaiting-authentication -> opening-client -> active -> closed`；保留的内部阶段名现在表示 Cookie 已认证 Upgrade 之后的第一次协议版本握手。`gateway-control-authenticate` 只携带协议版本，并且是唯一可以请求 Desktop 页面端口的首条消息。已关闭页面的迟到端口 bundle 会被释放。附件注册遵循 `registered -> authenticated -> retired`；它不能恢复或重新绑定。错过握手期限的 control 或 attachment socket 会在不取得权限的情况下终止。每个活动 socket 参与同一个 ping/pong heartbeat；漏掉一个 pong 只终止该 socket。Gateway shutdown 会终止所有阶段的 socket 并清除其拥有的全部 timer。

该协议是现有 Desktop 多客户端会话和需求附件模型的传输改进。它没有添加新的产品生命周期状态或规范文档转换。`apps/gateway/formal/tla/GatewayTransportLifecycle.tla` 只对有界的监听取消、认证期限和心跳细化具有权威性。确定性的生命周期跟踪、严格的编解码器测试和真实的 Electron-plus-two-Browser 系统跟踪提供具体传输证据。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/gateway-protocol build
pnpm --filter @orchbee/gateway-protocol typecheck
pnpm --filter @orchbee/gateway-protocol test
pnpm --filter @orchbee/gateway-protocol test:exports
```
