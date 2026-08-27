# `@orchbee/remote-client-protocol`

`@orchbee/remote-client-protocol` 负责严格且与运行时无关的契约，让人类 Browser 用一个个人访问令牌换取不透明的 Orchbee Browser session。当前消费者是由 Desktop 显式启用的浏览器访问预览。HTTP 与 WebSocket adapter 仍由各自应用负责。

## 鉴权模型

| 字段   | 契约                                                                                                                                                                  |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Browser 用户必须完成鉴权，同时可复用 credential 不得进入 URL，登录后也不得暴露给同源脚本。                                                                            |
| 输入   | 一个不受信任的 43 字符 base64url 个人访问令牌、可选 Cookie header、同源登录请求，以及当前浏览器访问 session 的签名密钥。缺失表示未鉴权。                              |
| 数据   | Desktop 拥有一个安装范围的令牌 verifier；每个活动浏览器访问 runtime 拥有一个随机签名密钥；每次成功交换创建一个随机、不透明的 session identity。                       |
| 输出   | 成功交换返回一个 `HttpOnly`、`SameSite=Strict`、仅限当前 host 的 session Cookie。无效输入返回一个有界鉴权错误且不创建 session。                                       |
| 不变量 | 原始个人访问令牌绝不进入 URL 或 Cookie；只有精确 payload signature 校验成功时才接受 Cookie；重复的 session Cookie 必须关闭失败；签名密钥轮换会使全部旧 session 失效。 |
| 决策   | 静态 renderer asset 与登录 document 可公开访问；产品 document 要求有效 session；当前 HTTP 预览 Cookie 不带 `Secure`，并继续显示局域网未加密警告。                     |
| 模型   | 严格 schema 校验交换请求与 credential；纯 Cookie 函数编码、提取并校验一个规范的签名 session 值。                                                                      |
| 依赖   | 调用方注入密码学签名、常量时间校验、随机性、有界内存尝试限流、HTTP response 处理和 process lifecycle。本 package 不读取 global。                                      |
| 所有权 | Desktop 负责令牌创建、verifier 持久化和轮换。Server 负责令牌校验与 Cookie 签发。Gateway 负责依据同一个签名 session 准入 WebSocket。                                   |
| 验证   | 契约测试覆盖规范及畸形令牌、credential 篡改、重复 Cookie、必需属性和可选的未来 TLS 属性。应用测试覆盖密码学、路由、限流与 WebSocket 准入。                            |

## 生命周期与转换

| 状态       | 事件                                 | 结果                                                                          |
| ---------- | ------------------------------------ | ----------------------------------------------------------------------------- |
| 令牌缺失   | 创建                                 | 原子持久化 verifier，并只展示一次新令牌。                                     |
| 令牌已配置 | 重置                                 | 停止活动浏览器访问，原子持久化新 verifier，只展示一次新令牌，并要求重新登录。 |
| 未鉴权     | 提交匹配令牌                         | 创建新的签名 session Cookie，并恢复保留的同源产品路由。                       |
| 未鉴权     | 提交畸形、不匹配、跨源或被限流的请求 | 保持未鉴权并返回有界错误。                                                    |
| 已鉴权     | 重新加载产品路由                     | 当前签名密钥可以校验 Cookie 时，准入 document 与 Gateway control connection。 |
| 已鉴权     | 停止访问、重置令牌或重启访问         | 轮换或丢弃签名密钥、关闭 Gateway connection，并拒绝旧 Cookie。                |

令牌轮换在 Desktop 原子替换 verifier store 时提交。关闭浏览器访问是重置的前置条件。如果关闭或持久化失败，则不提交新 verifier。如果 verifier 持久化已提交但一次性 renderer 交付失败，令牌仍不可恢复，用户通过再次重置来恢复。

## 正确性主张与边界

| 主张                                                            | 证据                                                                     | 边界                                                             |
| --------------------------------------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------- |
| 原始个人访问令牌不会成为 Browser session credential。           | Cookie 契约测试与 Server 交换集成测试。                                  | 受信任的 Desktop renderer 会看到一次新创建的令牌，以便用户复制。 |
| 伪造、畸形、过期或重复的 Cookie 不会授予 Browser 权限。         | 纯 credential 测试、常量时间 adapter 测试与真实 HTTP/WebSocket 集成。    | 同用户 process 检查以及签名密钥泄露不在此主张范围内。            |
| 重置与浏览器访问 lifecycle 退出会撤销全部活动 Browser session。 | Desktop workflow 测试与 Gateway 断开/重连集成。                          | 当前预览在 Server 停止期间不提供远程登出传播。                   |
| 预期产品路由在登录后保留，且不存在开放重定向。                  | return-target policy 测试，以及两个已交付 locale 下的 Browser 登录渲染。 | 仅支持同源相对产品路由。                                         |

当前集成仍是 loopback 或显式 RFC 1918 trusted-LAN HTTP 预览。它不声明网络机密性、Internet 安全、多用户授权、账号恢复、持久化限流状态、TLS 终止或设备鉴权。Capability node 使用独立的非 Cookie credential，不属于本 package。

## 公共 API 与验证

根导出提供令牌、verifier、session identity、签名密钥、交换、错误、路由和 Cookie schema 与 helper。它有意不公开任何密码学或持久化 adapter。

运行：

```sh
pnpm --filter @orchbee/remote-client-protocol build
pnpm --filter @orchbee/remote-client-protocol typecheck
pnpm --filter @orchbee/remote-client-protocol test
pnpm --filter @orchbee/remote-client-protocol test:exports
```
