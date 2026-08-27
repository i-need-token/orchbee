# `@orchbee/server`

`@orchbee/server` 是 Orchbee 可运行的 HTTP 渲染器基础设施。当前范围负责显式的环回或 trusted-LAN-preview 监听器、个人访问令牌交换、签名浏览器会话 Cookie 准入、同一监听器上的 WebSocket Upgrade 集成、受信任 Web 渲染器交付，以及经过身份验证的 Browser Router 回退。它不公开通用产品 API，也不构成生产部署契约。

已接受的未来方向和保留的安全决策见 [`proposals/platform/connected-client-capability-service.md`](../../proposals/platform/connected-client-capability-service.md)。

## 当前边界

输入是受信任的渲染器发布产物、个人访问令牌验证值、新生成的会话签名密钥和本地进程配置。输出是 HTTP 响应、由所属组合模块注册的可选应用路由，以及可关闭的监听器句柄。两项机密输入都不会出现在就绪输出或浏览器可见状态中。

| 请求类                                         | 当前结果                                                            |
| ---------------------------------------------- | ------------------------------------------------------------------- |
| 现有 `/static/*` 渲染器资源                    | 公开资源响应，并使用长期不可变缓存                                  |
| 接受 HTML 的 `GET` 或 `HEAD` `/login`          | 公开渲染器文档，并要求重新验证                                      |
| 携带有效令牌的同源 `POST /api/v1/auth/session` | `204`，并下发签名的 `HttpOnly; SameSite=Strict; Path=/` 会话 Cookie |
| 无效或受到速率限制的会话交换                   | 带 `no-store` 的有界 JSON 错误；不下发 Cookie                       |
| 接受 HTML 的已认证不匹配 `GET` 或 `HEAD`       | 要求重新验证的渲染器文档，允许 Browser Router 解析 URL              |
| 未认证的产品文档请求                           | `302` 到 `/login`，携带一个编码后的相对返回目标；不返回渲染器 HTML  |
| 缺少 `/static/*` 渲染器资源                    | `404`；从不返回渲染器 HTML                                          |
| 不匹配的 `/api`、`/api/*`                      | JSON `404`；从不返回渲染器 HTML                                     |
| 普通 HTTP 请求到不匹配的 `/ws`、`/ws/*`        | JSON `426 Upgrade Required`；从不返回渲染器 HTML                    |
| 未知 WebSocket Upgrade                         | 拒绝；不存在通用或隐式 WebSocket capability                         |
| 其他不匹配的请求                               | JSON `404`                                                          |

`/api/v1/*` 和 `/ws/v1/*` 是第一个保留的版本化应用程序命名空间。保留其父前缀可防止拼写错误或不受支持的协议路径意外成为 HTML 导航。

## 生命周期和不变量

1. 在打开任何套接字之前解析进程配置。
2. 在 Fastify 准备就绪之前，渲染器根目录 `index.html` 和 `static/` 目录必须作为受信任的本地构建输入存在。
3. 监听器默认为`127.0.0.1`；配置解析时仅接受显式的 trusted-LAN 预览值 `0.0.0.0`。
4. 会话交换要求精确的请求 Origin，以常量时间验证令牌，并按来源地址应用有界的内存失败限制。
5. Cookie 只包含随机签名会话标识，绝不包含个人访问令牌；浏览器访问重启或令牌重置时，新签名密钥会使全部旧会话失效。
6. HTTP 和 WebSocket 路由共享一个 HTTP/1.1 侦听器，因此共享一个源和端口。
7. API、WebSocket 和静态命名空间永远不会落入 Browser 路由器条目。
8. 关闭返回的运行服务器句柄将关闭共享侦听器及其拥有的每个连接。

渲染器分发是部署输入，而不是 HTTP 上传或用户选择的目录。外部请求数据永远不会用于选择另一个文件系统根。

## 本地运行

构建 Web 渲染器和 Server，然后提供渲染器根作为绝对路径：

```sh
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/server build
ORCHBEE_SERVER_RENDERER_ROOT=/absolute/path/to/apps/web/dist \
ORCHBEE_SERVER_PERSONAL_ACCESS_TOKEN_VERIFIER=<43-character-base64url-verifier> \
ORCHBEE_SERVER_SESSION_SIGNING_KEY=<43-character-base64url-key> \
  pnpm --filter @orchbee/server start
```

`ORCHBEE_SERVER_PORT` 默认为 `0`，因此操作系统选择可用端口。该进程会发出一份结构化的、非秘密的就绪记录，其中包含实际的主机和端口。`ORCHBEE_SERVER_HOST` 可以省略、设置为 `127.0.0.1`，或明确设置为 `0.0.0.0`。Desktop 仅在用户选择 trusted-LAN 预览并将此静态服务器与 exact-Origin、Cookie 认证的 Gateway 配对后才使用最后一个值。Desktop 生成并传输两项必需的认证输入；它们不是面向运维者的部署凭据。

## 有意省略

`0.0.0.0` 模式是 trusted-LAN HTTP 预览，不是生产部署模式。Server 保持静态资源和登录文档公开，使未认证页面能够呈现登录流程。它不承诺 TLS、互联网暴露、发现、多用户账号、令牌恢复、持久化速率限制协调、后台服务运行、通用产品 API 或生产 WebSocket 托管。同源令牌交换和 Gateway 的 exact-Origin 检查可以降低浏览器请求伪造风险，但不会加密局域网中的 HTTP 流量。
