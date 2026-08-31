# SSH Environment 协议

由 Desktop 与每个 Environment 独立的 SSH Runtime 共享的严格二进制 framing。版本 `0.4.0`
提供精确的 Hello handshake、Runtime instance 与 epoch identity、heartbeat liveness、channel
lifecycle record、二进制 channel data，以及 byte-credit flow control。

## Wire contract

每条 record 均以 16 字节 header 开始：

| Offset | 字节数 | 含义                                      |
| ------ | ------ | ----------------------------------------- |
| 0      | 4      | ASCII magic `OBSE`                        |
| 4      | 3      | 精确的 major、minor 与 patch version byte |
| 7      | 1      | Record kind                               |
| 8      | 4      | Big-endian unsigned payload length        |
| 12     | 4      | 保留的全零字节                            |

整条 record 上限为 1 MiB。Control payload 是严格且有界的 UTF-8 JSON。Channel data 使用四字节
big-endian channel identity，后接 opaque byte；credit record 包含 channel identity 与 byte count。
Decoder 会拒绝未知 version、kind、field、无效 UTF-8、无效 JSON、非零保留字节，以及不一致或越界
length。

Hello acceptance 标识一份不可变的 Runtime build、一个 release cohort、一个 Terminal Host
version/build、一个 daemon instance、一个 Runtime epoch，以及远端账户声明并验证过的绝对 login-Shell
path。重新连接可以附着到同一个 epoch；Runtime 重启后必须报告新 epoch。Runtime 精确 advertise 有序的
`[project-root-v2, terminal-host-v1]` pair，Desktop 要求该精确 pair。协议不提供 downgrade、dual-stack
negotiation 或 placeholder capability。

## Project-root service contract

一个 `project-root-v2` channel 精确承载一条 strict request。Canonical inspection 使用：

```json
{ "type": "inspect-project-root", "path": "/absolute/posix/path" }
```

已认证的 directory listing 使用：

```json
{ "type": "list-project-directory", "path": null }
```

`null` 选择 Runtime process 已验证的绝对 `HOME`；绝对 path 用于直接导航。每个 path 都必须是有效
UTF-8、absolute POSIX text、encoded bytes 不超过 4,096，并且不包含 NUL 或 control character。双方都不
进行 tilde、environment-variable、wildcard、command 或 shell expansion。完整 request 与 result 各自不
超过 128 KiB，每个方向精确获得 128 KiB channel credit。

Result 要么是 `{"kind":"resolved","canonicalPath":"/..."}`，要么是
`{"kind":"failed","reason":"..."}`。Failure reason 封闭为 `invalid-path`、`not-found`、
`not-directory`、`permission-denied`、`timed-out`、`busy` 与 `unavailable`。Channel identity 是唯一
correlation identity；service schema 不引入 Project、Checkout、Workspace 或 directory identity。Listing
result 包含 canonical path、可选 canonical parent、最多 256 个已排序 child entry；每个 entry 只含一个
最多 255 字节的 UTF-8 directory name，并带有 `truncated` flag。它不暴露 file entry 或 file byte；symlink
只有在其 target 可作为 directory 打开时才会出现。

在 Desktop 观察到 result 前，cancellation 以本地结算为准。如果 Desktop 结算 cancellation 时 Runtime 已经
发出唯一一条占用 credit 的 result，Runtime 会接受 cancelled close，Desktop 则在匹配的 close
acknowledgement 前消费并丢弃这一条 in-flight frame。交叉的 result 绝不会改变 caller settlement 或使
attachment 失效；额外或超出 credit 的 data 仍属于 transport corruption。

## Terminal Host relay contract

一个 `terminal-host-v1` channel 承载 opaque Terminal Host protocol byte。每个方向起始时精确获得
64 KiB credit；每个 data chunk 必须非空且不超过 64 KiB。只有 receiving boundary 消费 byte 后才返回
credit。一个 Runtime attachment 只允许一个 relay，同时 channel identifier 在 relay 与 project-root
service 之间保持全局单调。

Open failure 封闭为 `service-busy`、`service-unavailable` 与 `compatibility-unsupported`，并保持
channel-local。Attachment 丢失会关闭 relay 且不 replay；outer epoch、channel、framing、ordering 或
credit corruption 会 retire attachment。此 contract 既不定义也不解析 Terminal Host record，并且不授予
通用 command authority。

## 离线 release contract

冻结的 release schema `1` 定义 Runtime-only 双 platform contract。Release schema `2` 独立定义精确
有序的 `ssh-environment-runtime` 与 `terminal-host` component；每个 component 都包含精确有序的
`darwin-arm64` 与 `linux-x64` artifact，并保留自己的 semantic version。每个 artifact 都有非零且上限
为 128 MiB 的 size 与 SHA-256 digest。Trust schema `1` 和 `2` 绑定 canonical Ed25519 SPKI、其
`ed25519-spki-sha256:<digest>` identity，以及精确匹配 final-manifest bytes 的 SHA-256。

Signing payload 没有 trailing newline；final manifest 与 trust descriptor 各有一个。Schema 会拒绝
unknown field、component 或 platform reorder、缺失或不受支持 entry、noncanonical signature 与越界
value。固定 directory 与 file name 同 schema 一并导出，使仓库 release automation 与 Desktop
packaging 无法静默选择不同 layout 或混用 v1 与 v2。

## Authority 边界

本 package 只是一份 data contract。它不能 build 或 sign artifact、持有 private key、验证 package
trust、组合 release、创建连接、安装或启动 component、认证 peer、重试 attachment、授权 inspection
或执行 filesystem work。它拥有精确 `project-root-v2` schema/codec 与 `terminal-host-v1` opaque relay
envelope，但刻意不提供通用 execute-command record。任何新增 Runtime service 都必须由其所属 subsystem
显式 advertised、admitted、authorized 并实现。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/ssh-environment-protocol build
pnpm --filter @orchbee/ssh-environment-protocol typecheck
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-protocol test:exports
```
