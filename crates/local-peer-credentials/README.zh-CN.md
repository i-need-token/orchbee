# 本机 Peer Credential

这是一个私有、实验性的 Rust adapter，用于证明本机 stream 两端由同一 operating-system
effective user 运行。Server、Worker 与本机 Client connection owner 使用本 crate，但不共享
各自的 framing、handshake、identity、admission 或 lifecycle contract。

简体中文维护译本：本文档。
英文规范：[README.md](README.md)。
冲突时以英文为准。

## Contract

`require_same_effective_user` 从 kernel 获取已连接 peer identity，并与当前 effective user
比较。Linux 使用 `SO_PEERCRED`；Apple 与受支持的 BSD target 使用 `getpeereid`。两种 platform
call 均由 safe `nix` API 承担。identity 不一致时返回 `PermissionDenied`；不支持的 Unix
target 返回 `Unsupported`。

调用方已经拥有 connected local stream，并继续负责 endpoint selection、permission、connection
order、timeout、framing、role authentication、admission 与 stream lifetime。本 crate 不在 stream
上执行 I/O，不启动 thread 或 process，也不保留 identity。

## 限制

原生执行证据覆盖 macOS 与 Linux 上的 same-user socket pair。它不执行 cross-user negative
case，不认证 remote peer，也不声称 hostile multi-tenant
isolation。same-user process separation 限制意外 authority 与 failure coupling，但它不是 sandbox。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
cargo clippy -p orchbee-local-peer-credentials --all-targets --all-features --locked -- -D warnings
pnpm verify
```

## Windows 对端证明

对于 Windows 的 `LocalStream`，`require_same_effective_user` 委托给 `orchbee-local-ipc`：管道提供对端 PID，内核进程句柄提供其令牌 SID，该 SID 必须等于当前令牌用户。不使用 PID 文件、用户名或环境变量作为身份证据。原生同用户测试覆盖此适配器；不宣称已执行跨用户进程反例测试。

<!-- documentation-pairing: english-sha256=751482f3bbcde8ec843bd67c4ed79b45154b89f259d63fff5dd202247d0a280b zh-cn-sha256=7c2be4fd87b2f68bac59bc54e236247499ef2475e5c7ea067b2f5e1a281f06ea -->
