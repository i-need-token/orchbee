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

Utility Process 从不导入或连接 Terminal Host。它不拥有共享文档写入器、绑定注册表、应用生命周期端口、文件系统能力、Client View 存储、产品设置或终端数量策略。Desktop 仅在显式 Browser 访问 Action 后启动它，并在访问停止、模式切换或 Desktop 关闭时终止它；每次转换都会撤销生命周期能力。

## 页面和附件生命周期

| 转换                 | 效果                                                                                                               |
| -------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Utility 启动         | 绑定临时环回端口；只有显式 trusted-LAN 预览才绑定 `0.0.0.0`，并且只接受 Desktop 提供的精确渲染器 Origin 允许列表。 |
| 页面认证成功         | 请求一组全新的 Desktop ClientSession 与投影控制端口。                                                              |
| 页面认证失败         | 关闭套接字，不分配 Desktop 权限。                                                                                  |
| 投影打开             | 注册一个派生附件能力，并且只传输匹配附件的流。                                                                     |
| 附件队列超过字节上限 | 只退役该附件并释放其投影；保留关联关系直至 Main 确认清理，再从完整快照重新打开。                                   |
| 页面控制关闭         | 释放该页面的每个附件和页面会话；保留共享 Workbench 状态和 Host 终端。                                              |
| Utility 停止         | 终止仍在等待认证的套接字；关闭监听器、页面、附件和已传输端口后，才报告停止。                                       |

控制与附件消息在每个进程或网络边界都通过 `@orchbee/gateway-protocol` 解析。终端帧和行 payload 保持二进制；Gateway 不执行 VT 解析、行重建、base64 转换或逐单元处理。传输字节限制保护单个连接，不施加产品数量上限。

## 能力交付与 trusted-LAN 预览

Desktop Main 保留每个完整的访问 URL 并仅将清理后的 HTTP Origin 暴露给其沙盒渲染器。复制链接并打开 Browser 是主要拥有的效果，仅限于当前活动的确切起源。高熵能力保留在 URL 片段中，因此不会在渲染器 HTTP 请求中发送。它永远不会置于渲染器状态、诊断、遥测、文件或就绪输出中； Gateway 本身不发出能力诊断。

环回是默认值。 Trusted-LAN 预览将渲染器 Server 和 Gateway 绑定到 `0.0.0.0`，但仅发布枚举的 RFC 1918 IPv4 起源。它使用未加密的 HTTP，没有帐户登录或 TLS，并向持有链接的任何人授予完全终端控制权，直到访问停止或 Desktop 退出。 Desktop UI 在激活前显示该警告并提供立即撤销。

## 正确性证据

| 声明                                     | 证据                                                                                           |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 身份验证先于 Desktop 页面分配            | 纯生命周期转换测试和真实环回WebSocket测试。                                                    |
| 错误的来源或能力不会产生页面权限         | 真实环回和 trusted-LAN-style exact-Origin WebSocket 拒绝测试和 Desktop-plus-Browser 系统跟踪。 |
| 断开连接仅影响一页                       | 生命周期测试、范围端口测试和多客户端系统重新连接。                                             |
| 实用程序停止无法等待未经身份验证的套接字 | 真正的环回附件关闭回归。                                                                       |
| 一个附件失败不能破坏其他附件             | 附件标识校验、二进制编解码器测试、有界队列退役及重新打开投影测试。                             |
| Gateway 无法绕过 Main 或 Host 所有权     | Workspace 依赖关系边界、项目合同以及缺少 Host 依赖关系。                                       |

生命周期是现有 Desktop 多客户端模型的传输细化；它不添加新的规范 Workbench 或终端生命周期状态机。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/gateway typecheck
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/gateway build
```
