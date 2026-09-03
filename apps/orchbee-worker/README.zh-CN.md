# Rust Orchbee Worker

这是第一条 POSIX 本机终端 vertical 使用的实验性、尚未发布私有 Rust Worker application。
产出的 executable 是 `orchbee-worker`。它参与根 Cargo gate，但不会被打包、安装、由
`orchbee` 激活，也没有被当前产品路径消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 进程边界

在 macOS 与 Linux 上，确切私有开发调用为：

```text
orchbee-worker \
  --local-endpoint <absolute-server-socket> \
  --terminal-host-runtime-directory <absolute-directory> \
  --terminal-host-endpoint-id <32-lowercase-hex>
```

Worker 首先连接所提供的 Server Unix-domain socket，要求对端具有相同 effective
operating-system user，提交有界 cohort handshake，并验证 typed admission。在 Server 接受该
Worker connection 之前，它不会接触 Terminal Host。

Server admission 之后，Worker 只解析
`<runtime-directory>/<endpoint-id>.s`，连接现有 Terminal Host，独立要求相同 effective user，
并执行现有精确版本 Terminal Protocol `Hello`/`HelloAccepted` exchange。请求使用固定的
diagnostic Worker broker client identity，声明 protocol hard limit 且不声明 optional feature；
只有 registered request/response role、精确 correlation、精确 protocol version、匹配的 negotiated
limit、空 feature intersection 与非零 Host epoch 都成立时才接受。

只有两个对端均接受后，Worker 才向 stdout 写入 `ready\n`。随后它的主线程在两个自有 socket
上 poll，不创建 connection thread 或 queue。Server loss 会关闭 Host broker socket；Host loss
会关闭 Server attachment。两种结果都保持静默并以 status `1` 退出，使 Server 能观察 Worker
loss，也使没有 retained terminal 的 Host 能执行自己的 idle-shutdown rule。Invalid invocation、
connection、authentication、rejection、protocol、output 或 transport failure 也采用同样的静默
status-`1` 结果。

本切片在 handshake 之后不发送 Server message，发往 Terminal Host 的 message 也只有
`Hello`。它不执行 retry、discovery、activation、daemonization、child-process supervision、
terminal inventory、launch、attachment、input、resize、filesystem inspection、shell 或
working-directory resolution、PTY operation、Agent behavior 或 durable write。它不使用
current-directory、Project、Environment、Checkout、Worktree、launch-profile、Files 或 Git
fact。Endpoint selection、service supervision、Server-issued launch authorization、Host
activation 与 reconciliation 属于后续切片。

## 本机 Host broker 建模记录

| 字段   | 当前契约                                                                                                                                                                                                                                                                                                                                                                                          |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 本机 Server 已能接纳一个 Worker，Rust Terminal Protocol binding 也能编码 Host connection message，但尚无真实 Worker process 持有正常的 node-local Host broker connection。本切片必须建立该连接，同时不虚构 terminal authority，也不绕过 Server launch authorization。                                                                                                                             |
| 输入   | 不可信私有 argument vector；一个绝对 Server socket；一个绝对 Host runtime directory；一个规范的 32 位小写十六进制 Host endpoint ID；两个 socket 的 kernel peer credential；一次有界 local-Site handshake；以及一次有界 Terminal Protocol handshake。缺失、malformed、incompatible、wrong-role、rejected、timed-out 与 disconnected input 都是显式 failure。                                       |
| 数据   | 一个 Worker 自有 Server socket 及其已接受的非零 Worker generation，以及仅在 Server admission 后出现的一个 Worker 自有 Host socket、已接受非零 Host epoch 与 negotiated protocol limit。这些 process-local value 会一起释放，绝不成为 Server、PTY、VT、attachment 或 terminal state。                                                                                                              |
| 输出   | 双边 admission 成功后恰好发出一个 `ready\n` marker。其余所有结果都是静默 status-`1` process outcome。Host 与 Server 在另一侧失败时分别观察到 connection close。不产生 terminal request、semantic event 或 durable mutation。                                                                                                                                                                      |
| 不变量 | Host contact 必须在 Server admission 之后；跨越 Host boundary 的只有现有 Host `Hello` request；readiness 要求两个 connection 都已接受；一个 Worker 对每个对端最多持有一个 socket；停止会释放两者；任一 peer loss 都不能遗留另一条 connection 或 connection thread；terminal-command count 始终为零。                                                                                              |
| 决策   | Invocation validity、Server admission、Host endpoint syntax、peer-user equality、envelope length、registered message role、correlation identity、精确 protocol version、negotiated limit、optional-feature intersection、Host response kind、timeout 与 peer liveness 完整决定结果。                                                                                                              |
| 模型   | 顺序 admission workflow，随后是单一 two-descriptor liveness poll。[`LocalHostBroker`](formal/tla/LocalHostBroker.tla) 对独立排序的 Server rejection/loss、Host peer rejection、wrong role、incompatibility、malformed response、timeout、Host loss、accepted readiness 与 coupled link cleanup 建模。                                                                                             |
| 转换   | `absent -> connecting-server -> connecting-host -> host-handshake -> host-admitted -> ready`；任何 rejection 或 loss 都直接进入 `stopped` 并关闭两个 link。没有 transition 会 create、attach、mutate 或 dispose terminal。                                                                                                                                                                        |
| 依赖   | Rust 标准库 argument、path、Unix-socket、有界 I/O 与 process API；安全 `nix` polling；独立 local peer-credential adapter；私有 local-Site protocol；以及现有 Terminal Protocol 的独立 Rust binding。没有 async runtime、network listener、child process、persistent storage、shell、PTY、VT、filesystem-content、Git 或 Agent capability。                                                        |
| 验证   | 注册的 TLA+ safety 与 progress checking；真实 Worker process 配合受控 fake Server/Host peer 的 ordering、malformed/rejected handshake、精确 readiness、Host-loss 与 Server-loss cleanup 测试；以及真实 Worker 到 Zig Host 的 native process witness，它执行 production handshake 并在 Worker disconnect 后观察 Host idle shutdown。                                                               |
| 限制   | Runtime evidence 覆盖同一 effective user 下的 macOS 与 Linux。Same-user process separation 并非 hostile-tenant isolation。Witness 只建立 connection、handshake、liveness 与 cleanup；不建立 Host activation、terminal launch、retained-terminal continuity、data relay、reconnect、Windows named pipe、remote enrollment、production service lifecycle、packaging、latency 或 throughput budget。 |

检查过的 TLA+ model 不包含 terminal command transition。它的 `serverLink` 与 `hostLink` 映射到两个
自有 Unix stream，`hostRequest = "hello"` 映射到唯一发出的 Terminal Protocol record，
`readinessEmitted` 映射到 stdout marker。Process test 持有 implementation refinement 与 native
boundary evidence；model 只证明其有限 state space 与声明的 fairness。Fairness 只适用于 admission、
Host handshake 或 readiness publication 尚未完成时。进入 ready 后，peer loss 仍是普通的非确定性
外部 action，因此完好的 Worker 可以无限期 poll。

## 所有权

[`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.zh-CN.md) crate 拥有
Server-Worker framing。
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md) crate 拥有两条
本机 connection 使用的 kernel same-user proof。
[`orchbee-terminal-protocol`](../../packages/terminal-protocol/references/protocol.zh-CN.md) package
拥有 Host wire；其独立 Rust binding 提供本次消费的 envelope 与 connection codec。Worker 拥有
admission order、Host broker connection 与 coupled liveness。Server 仍是唯一 placement 与
launch-authorization owner。Terminal Host 仍是唯一 PTY、process-tree、VT、history 与 attachment
authority。

[`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model 继续拥有
Server admission 与 Worker-generation behavior。`LocalHostBroker` 只从该 admission boundary
之后开始，不增加第二个 Server 或 Host model。

## 验证

从仓库根目录运行。真实 Host process witness 使用正常 Terminal Host build artifact，所以执行聚焦
Cargo test 前要先构建它；根 merge gate 本身已经先 build 再 test。

```sh
pnpm --filter @orchbee/terminal-host build
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-site-process-spine,local-host-broker
pnpm verify
```
