# SSH Environment Runtime

`orchbee-ssh-environment-runtime` 是通过可替换 SSH attachment 到达的、每个 Environment 独立的
detached daemon。它是 native transport endpoint，不是 remote shell，也尚不是远程 terminal、Project、
Git 或 Agent service。它唯一准入的 filesystem 行为是下文所述的有界只读 `project-root-v1` observation。

## 命令

```text
orchbee-ssh-environment-runtime --version
orchbee-ssh-environment-runtime serve --runtime-directory <absolute-path> --instance-id <32-hex> --build-digest <64-hex>
orchbee-ssh-environment-runtime attach --runtime-directory <absolute-path> --instance-id <32-hex>
```

`serve` 拥有一个 daemon epoch、lifetime lock、private Unix-domain socket，以及 Desktop bootstrap
准入的精确 signed artifact digest。`attach` 会认证这个 same-user endpoint，并且只桥接 stdin/stdout
protocol byte。Desktop bootstrap 使用已脱离的 inherited stream 启动 `serve`，随后用 `attach` 替换
remote shell；这种拆分使 SSH child 消失后 daemon 仍可存活。

Runtime directory、lock 与 socket 必须属于 effective user，具有精确的 owner-only permission，不包含
extended ACL grant，并且在准入时绝不跟随 symlink。Server 还会检查每个本地 peer 的 effective user
identity。同一时间只允许一个 attachment；竞争 attachment 会收到 `runtime-attachment-conflict` 并被关闭。

## 协议与生命周期

Runtime 实现 `@orchbee/ssh-environment-protocol` 的 `0.1.0` 版本。它接受一个精确 Hello，返回不可变
artifact version 与 SHA-256 build digest 以及一个新的 daemon epoch，并确认 heartbeat。Partial record
与未完成 handshake 同时受时间和大小限制。断开 attachment 不会停止 daemon 或改变 epoch；重启 daemon
始终会产生新 epoch。

Runtime 只 advertise `project-root-v1`；Desktop 会拒绝任何其他 capability set。协议刻意没有
execute-command message。后续 terminal 或其他 service 需要各自已接受的 owner contract 与 private
Runtime-side adapter。

## Project-root inspection

一个由 Desktop 发起的 channel 检查一个显式 absolute POSIX UTF-8 path。Runtime 跟随远端 operating-system
symlink，要求最终 target 是可访问 directory，并且只返回 canonical absolute path。非法 path、不存在、非
directory、permission、deadline、capacity 与内部 availability outcome 都映射到协议中的有界 reason；原始
operating-system error 不会跨越 channel，也不会进入 log。

每个 Environment 最多运行四个 inspection。每次 filesystem observation 都在同一 immutable Runtime binary
的受限 child 中执行；path 只通过 stdin 到达 child，stderr 被丢弃，stdout 只承载一条私有有界 result。Daemon
把这些 child 与 attachment 一起 poll，因此 filesystem latency 无法阻塞 heartbeat。Cancellation 或
attachment loss 会 kill 匹配 child，30 秒 deadline 会在发布 `timed-out` 前强制终止它。Reconnect 永不继承
或 replay 旧 channel。

该 service 不会列出 directory、读取 file byte、保留 handle、watch 或 mutate path、运行 shell，也不会分配
Project、Checkout、Workspace 或 directory identity。Requested path 与 canonical path 始终只存在于进程内。

当前 daemon retention 覆盖 user session 与操作系统继续保留进程时的 attachment loss。跨 total logout
或 reboot 存活需要后续评审过的 service 或 linger integration。任何命令都不会移除旧安装版本或远程工作。

## 兼容性与 release identity

当前只准入以下远端 build target：

- `aarch64-macos`，wire 上报告为 `darwin-arm64`；以及
- `x86_64-linux-gnu`，wire 上报告为 `linux-x64`。

其他 target 会在 build selection 阶段失败，不会产生未经声明的 binary。Production build 必须由 release
pipeline 固定为 Zig `0.16.0`、`ReleaseSafe`、stripping，并传入显式
`-Druntime-version=<semver>`。Version identity 绝不从 Git state 推断。无下载 bootstrap 验证 signed
manifest 与 artifact byte 后，把该 artifact 的 SHA-256 传给 `serve`；daemon 在其 epoch 内不可变地保留
该值，并通过 Hello 返回。这避免了要求 binary 内嵌其自身最终 digest 的不可能循环。

## 离线 release 工作流

`release:prepare` 把精确 matrix cross-build 到私有 candidate，并且只原子发布两个 artifact 与 canonical
`manifest-signing-payload.json`。外部 release authority 或 HSM 对这些精确 payload bytes 签名，返回一个
raw 64-byte Ed25519 signature。Repository 代码绝不接受或读取 production private key。

`release:finalize` 独立接收 prepared directory、signature 与 public key。它拒绝 expanded layout、
symlink、非普通文件、noncanonical payload、错误 signature 或 artifact size/digest drift，然后只原子发布
artifact 与 canonical `manifest.json`。请求的 output path 必须是绝对路径且不存在：

```sh
pnpm --filter @orchbee/ssh-environment-runtime release:prepare --output /absolute/prepared-release --runtime-version 1.2.3
pnpm --filter @orchbee/ssh-environment-runtime release:finalize --prepared /absolute/prepared-release --signature /absolute/signature.bin --public-key /absolute/release-public-key.pem --output /absolute/final-release
```

Desktop package composition 与完整 trust/rollback contract 位于
[`../desktop/docs/ssh-runtime-release.md`](../desktop/docs/ssh-runtime-release.zh-CN.md)。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/ssh-environment-runtime build
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/ssh-environment-runtime format:check
```

真实进程 conformance test 在 private temporary directory 中启动 daemon，执行跨语言 Hello 与 heartbeat，
验证 canonical symlink resolution 及有界 failure 与 cancel path，拒绝竞争 attachment，重新连接到同一
epoch，并证明 daemon 重启会改变 epoch。Release test 只使用临时 key，并覆盖固定 build argument、signing
round trip、mutation 与精确 layout。没有测试会连接 SSH server 或读取 production signing key。
