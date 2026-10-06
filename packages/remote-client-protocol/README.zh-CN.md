# `@orchbee/remote-client-protocol`

保留用于把 Personal Access Token 换成不透明 Browser session 的严格、与运行时无关的 schema
和纯 Cookie helper。旧 Desktop Browser-access、Server 与 Gateway 消费者已退役。本包当前没有
应用消费者，且不属于此次退役的六个 workspace；保留此包不代表提供 Browser 接入或鉴权。

## 公共契约

Root export 负责：

- 有界的 Personal Access Token、verifier、session identity 与 signing-key schema；
- 严格 authentication request 与 error schema，以及旧 route constant；
- 通过注入 callback 创建并验证带版本的签名 session credential；
- 拒绝重复或空 session Cookie 的 Cookie 提取；以及
- 携带 `HttpOnly`、`SameSite=Strict` 与显式 `Secure` option 的规范 Cookie 序列化。

Credential helper 校验 session identity 与 signature shape。密码学签名与验证仍由调用者负责。
本包不拥有 token store、login UI、HTTP listener、WebSocket admission、rate limiter 或 process
lifecycle。

## 正确性与边界

契约测试覆盖规范及畸形 token、credential format 与 tampering、注入的 signature verification、
重复 Cookie 及必需 Cookie attribute。它们仅证明库契约。已退役应用的 token rotation、login
routing、network admission 与 session-revocation workflow 不再属于当前集成证据。

## 验证

```sh
pnpm --filter @orchbee/remote-client-protocol build
pnpm --filter @orchbee/remote-client-protocol typecheck
pnpm --filter @orchbee/remote-client-protocol test
pnpm --filter @orchbee/remote-client-protocol test:exports
```
