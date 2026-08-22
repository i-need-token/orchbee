# `@orchbee/server`

`@orchbee/server` 是 Orchbee 可运行的 HTTP 渲染器基础设施。当前范围负责显式的环回或 trusted-LAN-preview 监听器、同一监听器上的 WebSocket Upgrade 集成、受信任 Web 渲染器交付，以及 Browser Router 回退。它不公开产品 API 或生产 WebSocket 契约。

已接受的未来方向和保留的安全决策见 [`proposals/platform/connected-client-capability-service.md`](../../proposals/platform/connected-client-capability-service.md)。

## 当前边界

输入是受信任的渲染器发布产物和本地进程配置。输出是 HTTP 响应、由所属组合模块注册的可选应用路由，以及可关闭的监听器句柄。

| 请求类                                 | 目前结果                                                       |
| -------------------------------------- | -------------------------------------------------------------- |
| 现有 `/static/*` 渲染器资源            | 具有长期不可变缓存的资产响应                                   |
| 缺少 `/static/*` 渲染器资源            | `404`;从不渲染器 HTML                                          |
| 不匹配的 `/api`、`/api/*`              | JSON `404`;从不渲染器 HTML                                     |
| 普通HTTP请求到不匹配的`/ws`、`/ws/*`   | JSON `426 Upgrade Required`;从不渲染器 HTML                    |
| 未知 WebSocket 升级                    | 已拒绝；无通用或隐式 WebSocket 功能                            |
| 其他不匹配的 `GET` 或 `HEAD` 接受 HTML | 需要重新验证的渲染器 `index.html`，允许 Browser 路由器解析 URL |
| 其他不匹配的请求                       | JSON `404`                                                     |

`/api/v1/*` 和 `/ws/v1/*` 是第一个保留的版本化应用程序命名空间。保留其父前缀可防止拼写错误或不受支持的协议路径意外成为 HTML 导航。

## 生命周期和不变量

1. 在打开任何套接字之前解析进程配置。
2. 在 Fastify 准备就绪之前，渲染器根目录 `index.html` 和 `static/` 目录必须作为受信任的本地构建输入存在。
3. 监听器默认为`127.0.0.1`；配置解析时仅接受显式的 trusted-LAN 预览值 `0.0.0.0`。
4. HTTP 和 WebSocket 路由共享一个 HTTP/1.1 侦听器，因此共享一个源和端口。
5. API、WebSocket 和静态命名空间永远不会落入 Browser 路由器条目。
6. 关闭返回的运行服务器句柄将关闭共享侦听器及其拥有的每个连接。

渲染器分发是部署输入，而不是 HTTP 上传或用户选择的目录。外部请求数据永远不会用于选择另一个文件系统根。

## 本地运行

构建 Web 渲染器和 Server，然后提供渲染器根作为绝对路径：

```sh
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/server build
ORCHBEE_SERVER_RENDERER_ROOT=/absolute/path/to/apps/web/dist \
  pnpm --filter @orchbee/server start
```

`ORCHBEE_SERVER_PORT` 默认为 `0`，因此操作系统选择可用端口。该进程会发出一份结构化的、非秘密的就绪记录，其中包含实际的主机和端口。 `ORCHBEE_SERVER_HOST` 可以省略，设置为 `127.0.0.1`，或明确设置为 `0.0.0.0`。 Desktop 仅在用户选择 trusted-LAN 预览并将此静态服务器与 exact-Origin（经过功能验证的 Gateway）配对后才使用最后一个值。

## 有意省略

`0.0.0.0` 模式是 trusted-LAN HTTP 预览，不是生产部署模式。此 Server 不认证静态渲染器请求；终端权限仍位于独立 Gateway 的 exact-Origin 和进程生命周期能力检查之后。Token-to-Cookie 登录、Cookie 作用域、TLS、互联网暴露、发现、CSRF 防护、速率限制、后台服务运行、真实 API 路由和真实 WebSocket 消息均有意省略。因此，不在此处创建未来的 `remote-client-protocol` 和 `capability-node-protocol` 包。
