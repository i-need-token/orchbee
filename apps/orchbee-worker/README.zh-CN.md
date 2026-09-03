# Rust Orchbee Worker

这是第一条 POSIX 本机 Server-Worker process spine 使用的实验性、尚未发布私有 Rust Worker
application。产出的 executable 是 `orchbee-worker`。它参与根 Cargo gate，但不会被打包、安装、
由 `orchbee` 激活或连接到 Terminal Host。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 进程边界

在 macOS 与 Linux 上，确切私有开发调用
`orchbee-worker --local-endpoint <absolute-path>` 会连接所提供的 Unix-domain socket，要求 Server
具有相同 effective operating-system user，提交一条带 executable cohort 与固定 development-local
Worker identity 的有界 Worker handshake，并验证 typed admission response。接受后，它向 stdout
写入 `ready\n`，保持 attachment 且不发送 work。Server disconnect 是 bounded unavailable outcome，
并以 status `1` 退出。Invalid invocation、connection、authentication、rejection、protocol、output
或 transport failure 保持静默，也以 status `1` 退出。

Worker 不执行 retry、discovery、activation、daemonization、child-process supervision、filesystem
inspection、shell resolution、Terminal Host connection、PTY operation、Agent behavior 或 durable
write。它不使用 current-directory、Project、Environment、Checkout、Worktree 或 launch profile
fact。Endpoint selection 与 lifecycle supervision 属于后续切片。

## Ownership 与 model mapping

[`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.zh-CN.md) crate 拥有 Worker
framing；[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md)
拥有 kernel same-user proof。Worker 只拥有 connect、authenticate、register、acknowledge 与观察
Server loss 的顺序。Server 拥有 admission、active identity 与 generation。
[`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model 把本进程映射到
candidate、active Worker、disconnect 与 Server-stop transition；Server guide 拥有完整 modeling
record 与 evidence limit。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-site-process-spine
pnpm verify
```
