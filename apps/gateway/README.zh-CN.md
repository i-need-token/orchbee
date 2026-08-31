# Browser Gateway

按需启动的 Electron Utility Process，将环回或显式授权的 trusted-LAN Browser 页面连接到 Desktop Main 的传输无关 Workbench 客户端边界。

## 运行时边界

```text
Browser page
  ├─ one authenticated control WebSocket
  └─ one binary WebSocket per visible terminal attachment
          │
          ▼
apps/gateway (authenticated listener and framing only)
          │ two page-scoped MessagePorts
          ▼
Desktop Main (Workbench, TerminalPresentation, and lifecycle authority)
          │
          ▼
apps/terminal-host (PTY, process, VT, history, and TerminalId authority)
```

Utility Process 从不导入或连接 Terminal Host。它不拥有共享文档写入器、绑定注册表、应用生命周期端口、文件系统能力、Client View 存储、产品设置或终端数量策略。Desktop 仅在显式 Browser 访问 Action 后启动它，并在访问停止、模式切换或 Desktop 关闭时终止它；每次转换都会丢弃当前 Browser session 签名密钥并撤销所有已准入页面。

## 页面和附件生命周期

| 转换                 | 效果                                                                                                                  |
| -------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Utility 启动         | 绑定临时环回端口；只有显式 trusted-LAN 预览才绑定 `0.0.0.0`，并且只接受 Desktop 提供的精确 renderer Origin 允许列表。 |
| Control Upgrade 到达 | 接受 WebSocket 前要求精确允许的 Origin 和有效签名 Browser session Cookie；失败时不分配 Desktop 权限。                 |
| 页面协议握手到达     | 要求在 10 秒内收到协议版本握手，然后请求一组全新的 Desktop ClientSession 与投影控制端口。                             |
| 附件握手到达         | 要求在 10 秒内收到一个由页面派生的一次性附件 capability，并且只传输匹配附件的流。                                     |
| 已认证套接字保持连接 | 每 30 秒发送一次 ping；若上一 ping 未收到 pong，则终止该套接字。                                                      |
| 投影打开             | 注册一个派生附件 capability，并且只传输匹配附件的流。                                                                 |
| Release 先于投影打开 | 转发一次 release，只关闭一个相关联的迟到 open 端口，保留请求直至清理结算，并保持无关投影可用。                        |
| 附件队列超过字节上限 | 只退役该附件并释放其投影；保留关联关系直至 Main 确认清理，再从完整快照重新打开。                                      |
| 页面控制关闭         | 释放该页面的每个附件和页面 session；保留共享 Workbench 状态和 Host 终端。                                             |
| Utility 停止         | 终止仍在等待握手的套接字；关闭监听器、页面、附件和已传输端口后，才报告停止。                                          |

控制与附件消息在每个进程或网络边界都通过 `@orchbee/gateway-protocol` 解析。终端帧和行 payload 保持二进制；Gateway 不执行 VT 解析、行重建、base64 转换或逐单元处理。传输字节限制保护单个连接，不施加产品数量上限。

## Session 准入与 trusted-LAN 预览

Browser 访问 URL 只包含 renderer 路由、Gateway 端口和协议版本。Desktop Main 把复制链接与打开 Browser effect 限制在当前活动的精确 Origin；URL、renderer state、诊断、遥测与就绪输出都不包含可复用 credential。Server 使用安装范围的个人访问令牌换取 `HttpOnly` 签名 session Cookie。Gateway 在接受 control WebSocket 前，使用同一个仅当前 activation 有效的签名密钥校验该 Cookie。Desktop 只持久化令牌 verifier；重置会先停止 Browser 访问，再替换 verifier，因此旧签名密钥与全部活动 Browser session 都会退出。

环回是默认值。Trusted-LAN 预览将 renderer Server 和 Gateway 绑定到 `0.0.0.0`，但仅发布枚举的 RFC 1918 IPv4 Origin。它使用未加密的 HTTP，不声明支持 TLS、Internet 入口或多用户账号；网络观察者可能读取令牌、Cookie 与终端流量。Desktop UI 会显示这一警告、要求显式个人访问令牌，并允许通过停止访问或重置令牌立即撤销。

## 正确性证据

| 声明                                     | 证据                                                                                               |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Cookie 准入先于 Desktop 页面分配         | 纯生命周期转换测试和真实环回 WebSocket 测试。                                                      |
| 错误的 Origin 或 Cookie 不会产生页面权限 | 真实环回和 trusted-LAN-style exact-Origin WebSocket 拒绝测试和 Desktop-plus-Browser 系统跟踪。     |
| 断开连接仅影响一页                       | 生命周期测试、范围端口测试和多客户端系统重新连接。                                                 |
| 实用程序停止无法等待未经身份验证的套接字 | 真正的环回附件关闭回归。                                                                           |
| 监听取消始终会结算                       | 确定性的 listen/dispose 排序测试和有界 Gateway 传输模型。                                          |
| 认证与心跳计时器不会泄漏                 | 假计时器 WebSocket 测试和 `GatewayTransportLifecycle.tla`。                                        |
| Release/open 重排保持请求局部性          | 环回 WebSocket 与转移端口回归，以及 `ProjectedTerminalIpcReliability.tla` 的 `release-race` 配置。 |
| 一个附件失败不能破坏其他附件             | 附件标识校验、二进制编解码器测试、有界队列退役及重新打开投影测试。                                 |
| Gateway 无法绕过 Main 或 Host 所有权     | Workspace 依赖关系边界、项目合同以及缺少 Host 依赖关系。                                           |

生命周期是现有 Desktop 多客户端模型的传输细化；它不添加新的规范 Workbench 或终端生命周期状态机。`formal/tla/GatewayTransportLifecycle.tla` 负责有界的监听取消、认证期限和心跳细化。Desktop 的 `ProjectedTerminalIpcReliability.tla` 负责共享的有界 release/open 排序模型；Gateway 回归把其迟到端口关闭与无关请求隔离声明映射到网络 adapter。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/gateway typecheck
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/gateway test:tla
pnpm --filter @orchbee/gateway build
```
