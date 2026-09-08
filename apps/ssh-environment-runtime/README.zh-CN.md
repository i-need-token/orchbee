# SSH Environment Runtime

`orchbee-ssh-environment-runtime` 是通过可替换 SSH attachment 到达的、每个 Environment 独立的
detached daemon。它是 native transport endpoint，不是 remote shell。它拥有有界只读 `project-root-v2`
observation 与一个 opaque `terminal-host-v1` relay，但自身不创建 terminal、Project、Workspace、Git 或
Agent effect。

## 命令

```text
orchbee-ssh-environment-runtime --version
orchbee-ssh-environment-runtime serve \
  --runtime-directory <absolute-path> --instance-id <32-hex> \
  --build-digest <64-hex> --release-cohort-digest <64-hex> \
  --terminal-host-version <semver> --terminal-host-build-digest <64-hex> \
  --terminal-host-executable <absolute-path> \
  --terminal-host-runtime-directory <absolute-path> \
  --terminal-host-endpoint-id <32-hex>
orchbee-ssh-environment-runtime attach --runtime-directory <absolute-path> --instance-id <32-hex>
```

`serve` 拥有一个 daemon epoch、lifetime lock、private Unix-domain socket，以及 Desktop bootstrap
准入的精确 signed cohort identity。`attach` 会认证这个 same-user endpoint，并且只桥接 stdin/stdout
protocol byte。在认证连接建立前，`attach` 最多重试 private endpoint 五秒，因此 daemon 被突然停止后留下的
stale socket 不会与其替代进程竞速；连接建立后，bridge loss 会立即终止 attachment。其余 internal worker
command 是精确 private ABI，不授予 caller-selected command、shell 或 path execution。

Runtime directory、lock 与 socket 必须属于 effective user，具有精确的 owner-only permission，不包含
extended ACL grant，并且在准入时绝不跟随 symlink。Server 会先在同一目录绑定并保护一个未公开的 socket，
然后再通过原子操作发布准入 endpoint name，因此公开 path 永远不会以宽松的创建期 permission 被观察到。
Server 还会检查每个本地 peer 的 effective user identity。同一时间只允许一个 attachment；竞争 attachment
会收到 `runtime-attachment-conflict` 并被关闭。

## 协议与生命周期

Runtime 实现 `@orchbee/ssh-environment-protocol` 的精确 `0.4.0` 版本。它接受一个精确 Hello，返回 Runtime
build identity、release cohort identity、Terminal Host version/build、platform、远端账户声明并验证过的绝对
login-Shell path、有序 capability 与一个新的 daemon epoch，随后确认 heartbeat。Partial record 与未完成
handshake 同时受时间和大小限制。断开 attachment 不会停止 Runtime 或改变 epoch；重启 Runtime 始终会产生
新 epoch。

Runtime 精确 advertise `[project-root-v2, terminal-host-v1]`；Desktop 会拒绝任何其他 capability set。
一个 attachment 内的 channel identifier 全局单调。协议刻意没有 execute-command message、generic service
dispatch、forwarding 或 network listener。

## Project-root service

一个由 Desktop 发起的 channel 要么检查一个显式 absolute POSIX UTF-8 path，要么列出一个 directory。Null
listing path 会解析为 Runtime process 已验证的绝对 `HOME`。Runtime 跟随远端 operating-system symlink，
要求最终 target 是可访问 directory，并返回 canonical absolute path。非法 path、不存在、非 directory、
permission、deadline、capacity 与内部 availability outcome 都映射为有界 protocol reason；原始
operating-system error 不会跨越 channel，也不会进入 log。

每个 Environment 最多运行四个 operation。每次 filesystem observation 都在同一 immutable Runtime binary
的受限 child 中执行；path 只通过 stdin 到达 child，stderr 被丢弃，stdout 只承载一条私有有界 result。Daemon
把这些 child 与 attachment 一起 poll，因此 filesystem latency 无法阻塞 heartbeat。Cancellation 或
attachment loss 会 kill 匹配 child，30 秒 deadline 会在发布 `timed-out` 前强制终止它。Reconnect 永不继承
或 replay 旧 channel。如果唯一 result 已发出时与 Desktop cancellation 交叉，Runtime 会接受 cancelled
close；有序交付使 Desktop 在 close acknowledgement 前丢弃该 result。

Listing 在 128 KiB 内返回最多 256 个按字典序排列、可访问的 UTF-8 child-directory name，以及 canonical
path、可选 parent 和 truncation metadata。它会省略 file、非法或不可访问 entry，以及 target 无法作为
directory 打开的 symlink。该 service 永不读取 file byte、保留 handle、watch 或 mutate path、运行 shell，
也不分配 Project、Checkout、Workspace 或 directory identity；path 与 listing metadata 保持瞬时，绝不进入
log 或 durable state。

## Terminal Host relay

一个 `terminal-host-v1` channel 连接 configured same-UID private Host endpoint。Endpoint 不存在时，隔离 child
只会用固定 `serve` argument 启动 configured signed executable 精确路径，并重试 endpoint。它永不删除
endpoint、kill 或 replace 现有 Host、选择 `PATH`，也不解析 Terminal Host protocol bytes。

Relay 只准入一个 channel，双向各有 64 KiB credit，非空 chunk 最大为 64 KiB。Endpoint connection、launch
与 blocking Host I/O 留在 child 中；Runtime event loop 会把其 nonblocking pipe 与 heartbeat 及四个
project-root worker 一起 poll。Busy、unavailable 与 compatibility outcome 为 channel-local。Attachment
corruption 会 retire attachment；attachment loss 只 kill relay worker，绝不 replay outcome-unknown Host
effect。Terminal Host 拥有自己的 epoch、retained terminal、inventory、PTY 与 process tree。

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

这些 application-owned command 保留为冻结的 schema-version-1 Runtime-only workflow。
Schema-version-2 SSH Environment release 使用仓库级 `release:ssh-environment:prepare` 与
`release:ssh-environment:finalize`，把本 workspace 的 parameter-fixed artifact 与单独构建的
Terminal Host artifact 组装在一起。本 workspace 不导入 Terminal Host source，也不拥有组合
cohort。

Desktop package composition 与完整 trust/rollback contract 位于
[`../desktop/docs/ssh-runtime-release.md`](../desktop/docs/ssh-runtime-release.zh-CN.md)。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/ssh-environment-runtime... build
pnpm --filter @orchbee/terminal-host build
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/ssh-environment-runtime format:check
```

运行 focused test 前，先构建已声明 protocol dependency 与 Terminal Host conformance artifact。Runtime
test lifecycle 只构建自身 native artifact；recursive repository test gate 可能正在消费 sibling package
output 时，它绝不会重建或临时删除该 output。Root `pnpm verify` gate 已强制这一 build-before-test order。

真实进程 conformance test 在 private temporary directory 中启动 Runtime 与实际 Terminal Host，执行两个
跨语言 Hello 与 heartbeat，检查 relay credit，验证 canonical symlink resolution、有界 failure/cancel path
及 result-cancellation crossover，拒绝竞争 attachment，重新连接到同一 Runtime epoch，强制制造带 stale
endpoint 的 daemon crash，证明一个已经等待的 attachment 能到达替代 Runtime 的新 epoch，并证明 Runtime
重启会改变 epoch。Endpoint 存在只是初步等待条件：实际 Hello 证明已就绪，返回的 epoch 标识替代 daemon，
即使文件系统复用了旧 socket inode 也不受影响。每个 daemon 的 Hello 成功后才检查 socket 权限，因此重启检查
覆盖新发布的 endpoint。Release test 只使用临时 key，并覆盖固定 build argument、signing round trip、mutation 与精确 layout。没有测试会连接
SSH server 或读取 production signing key。
