# Orchbee 原生入口

这是已接受 Phase 3A 原生命令底座的 experimental、尚未发布 Rust process entry。产出的
executable 名为 `orchbee`。它参与根 Cargo verification gate，但不被 Desktop、packaging、
installer 或 package manager 消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 命令面

确切 development surface 是：

```text
orchbee
orchbee --help
orchbee --version
```

裸调用选择未来 TUI route，目前会报告本地化的 development-unavailable result。它不会进入
terminal mode，也不会渲染占位 interface。`--help` 与 `--version` 不初始化 TUI 或 backend
就会完成。其他、额外或非 Unicode argument 都是 usage failure。不存在 short flag、alias、
abbreviation、业务命令、target discovery、current-directory inference 或 compatibility behavior。

Help 只写 stdout。Unavailable 与 usage result 只写 stderr。Version 向 stdout 写一行与 locale
无关的 `orchbee <version>`。Development exit status 中，help 与 version 为 `0`，不可用的裸 TUI
route 为 `1`，usage failure 为 `2`。这些值不是未来 stable public exit table。

## 本地化与权威

Authored help 与 diagnostic 同时提供 `en-US` 和 `zh-CN`。Locale selection 依次检查非空
`LC_ALL`、`LC_MESSAGES` 与 `LANG`；以 `zh` 开头的 language prefix 选择 `zh-CN`，其他 value
均 fallback 到 canonical `en-US`。Locale data 只影响 presentation，绝不会持久化或用作 authority。

三个已交付 invocation route 只使用 Rust standard-library argument、environment、stdout 与
stderr capability。它们不调用 rendezvous code、不打开 file 或 socket、不启动 process 或
thread、不初始化 terminal mode，也不读取 product state。在另行接受的 packaged cutover 前，
已交付 TypeScript [`apps/cli`](../cli/README.zh-CN.md) 继续作为当前 CLI 权威。

## POSIX 本机 Site rendezvous 底座

application-private 的 `local_site_rendezvous` module 为后续已接受 activation workflow 暂存了一个
经过 mode 检查的 macOS/Linux rendezvous primitive。未来 installation composition 必须提供稳定的
`SiteScope`；cohort version 与 digest 不属于该 scope，因此 update 会保留同一个 Site identity，
release、local 与 checkout-scoped development installation 则可以彼此隔离。`SiteScope` 接受
`[a-z][a-z0-9]*(?:-[a-z0-9]+)*` canonical form 的 1 到 63 个 ASCII byte，因此不能加入 path
separator、dot component 或依赖平台的 encoding。

对于一个可信、已存在且为 absolute 的 per-user runtime base，确切 derived path 是：

| 角色               | 稳定路径                                         |
| ------------------ | ------------------------------------------------ |
| Scope directory    | `<runtime-base>/<site-scope>`                    |
| Activation lock    | `<runtime-base>/<site-scope>/activation.lock`    |
| Site lifetime lock | `<runtime-base>/<site-scope>/site-lifetime.lock` |
| Client socket      | `<runtime-base>/<site-scope>/client.sock`        |
| Worker socket      | `<runtime-base>/<site-scope>/worker.sock`        |

`ModeCheckedLocalSiteRendezvous` 刻意在名称中标明尚未完整的 security boundary。Preparation 只会在
scope directory 不存在时创建它。Preparation 与 validation 使用 `O_NOFOLLOW`
打开选定 runtime base，通过其 descriptor 解析受控 scope，并保留 no-follow directory descriptor。
Scope 必须是 effective UID 拥有且 exact mode 为 `0700` 的 directory。新 scope 会被规范化为
`0700`；已有的错误 mode 会被拒绝且不会修复。

每项 lock operation 都通过保留的 directory descriptor，以
`O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` 只打开或创建固定 leaf。Leaf 必须是 effective UID 拥有且
exact mode 为 `0600` 且 hard link 数恰好为一的 regular file。已有且有效的 stale file 会被复用，
不会被 truncate 或 replace。`try_acquire_activation_lock` 与 `try_acquire_site_lifetime_lock` 各自只执行一次
nonblocking exclusive kernel lock attempt，并返回 owned guard 或 `Contended`；它们不会 wait、
retry 或推断 process health。Drop guard 只释放其 advisory lock。该 module 绝不移除 runtime
directory、durable lock file、socket path 或这些 path 上的任何 entry。

在 inherited 或 extended ACL 可以授予 mode bit 之外访问权限的 filesystem 上，这个底座尚未建立
production-private runtime boundary：它既不检查也不移除 ACL。在具备 ACL 能力的 boundary 补齐
这一缺口之前，public activation 必须保持 unwired。它也不选择 runtime base 或 scope，不验证
Unix-socket path length 或 socket metadata，不 bind、connect、认证 peer、启动 process、等待
readiness、unlink endpoint，也不声称 stale-process recovery。到达可信 runtime base 之前由
operating system 解析的 ancestor 不属于其 no-follow claim。

该 primitive 仍由 `apps/orchbee` 拥有，因为本切片中只有 public Client-side activation boundary
具备已接纳的需求；不存在第二个 source consumer。
`apps/orchbee-server` 不能导入另一个 application，而且尚未获取 `site-lifetime.lock`；后续 Server
lifecycle slice 必须先接纳一个真实 shared owner 或 explicit composition seam，两个 application
才能使用同一 contract。

### 建模与证据

| 字段         | 当前暂存 contract                                                                                                                                                                                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | 为后续 local Client activation workflow 提供一个 path-component-safe、same-effective-UID、经过 mode 检查的 rendezvous identity，同时不提前实现 process activation、readiness、cleanup 或 recovery。                                                                                                                     |
| Inputs       | 一个可信、已存在且为 absolute 的 runtime base；在 63-byte ASCII boundary 解析的 installation-owned scope string；effective UID；以及 adversarial directory 和 lock entry。Scope 缺失、link、错误 type/owner/mode、stale file 与已持有 lock 都是显式 case。                                                            |
| Data         | Canonical `SiteScope`、确切 derived path、一个保留的 scope-directory descriptor，以及每个 acquired advisory lock 的一个 owned file descriptor。不包含 PID、readiness marker、process identity、durable record payload 或 socket state。                                                                                |
| Outputs      | Preparation 或 validation 返回保留的 rendezvous 或 typed failure。每次 lock attempt 返回 `Acquired`、`Contended` 或 typed filesystem failure。Client 与 Worker socket method 返回确切 stable path且不执行 I/O。                                                                                                      |
| Invariants   | 每个被打开的受控 directory 或 lock entry 都使用 no-follow；已接受 entry 在 admission 时具有 exact type、effective-UID ownership、mode 与 lock link count；已接受的 activation 与 lifetime leaf 在 admission 时无法 alias 同一个 inode；contention 不改变 file；drop resource 不移除任何内容；scope identity 独立于 executable cohort。 |
| Decisions    | Scope syntax 与 length、absolute base、create-versus-validate policy、descriptor metadata、lock kind、create-versus-existing lock file，以及一次 kernel lock outcome 完全决定结果。                                                                                                                                |
| Model        | 一个纯粹且有界的 scope/path resolver 向一个 descriptor-relative POSIX filesystem adapter 提供输入。该 adapter 不包含 retry loop、clock、process observer 或 recovery branch。                                                                                                                                       |
| Transitions  | 缺失 scope 可以变成一个 validated directory；缺失 lock leaf 可以变成一个 durable validated regular file；unlocked leaf 可以变成 guard-held，并在 guard drop 时回到 unlocked。已有 invalid 或 contended state 保持不变。                                                                                            |
| Dependencies | Rust path/value operation、用于 effective UID 与 descriptor-relative POSIX call 的 safe `nix` wrapper，以及用于一次 advisory exclusive attempt 的 `File::try_lock`。不存在 network、socket、child-process、thread、time、randomness、persistence payload 或 Server dependency。                                       |
| Ownership    | Typed scope 来自未来 installation composition；本 application 拥有 derivation、directory validation、lock-file validation 与 guard lifetime。未来 activation workflow 将拥有 deadline 与 process sequencing；Server 必须另外获得已接受的 lifetime-lock seam。                                                      |
| Verification | Owner-local deterministic test 覆盖 valid/invalid scope name、确切 path、create/validate behavior、runtime-base 与 scope link、non-directory、错误 owner/mode、lock symbolic/hard link、directory、FIFO、错误 metadata、独立 contention、stale inode/content preservation、reacquisition 与 cleanup non-ownership。              |
| Limits       | Test 在一个 effective user 下运行，并信任 kernel metadata 与 advisory-lock semantics。它们注入不同的 expected UID，而不是执行 cross-user。Durable 表示该 owner 绝不 unlink 或 truncate lock file，并不表示对其执行 fsync；kernel filesystem call 不具有 wall-clock guarantee。此 atomic primitive 不选择 TLA+ model；已接受的未来 activation workflow 仍须提供其 singleton/process-order model 与 total deadline。 |

## 测量

从仓库根目录收集 reference release baseline：

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

该脚本构建确切 locked release package，warm 后重复调用 `orchbee --version`，报告 artifact byte
与 warm latency percentile，并在 `/usr/bin/time` 可以提供时报告 peak resident memory。结果只描述
输出中注明的 host、toolchain、profile 与 invocation。它不是 acceptance budget、cold-cache
guarantee，也不是未来 TUI performance 的证据。

Phase 3A reference run 于 2026-09-02 在 Apple M1 arm64、Darwin 23.6.0、Rust 1.97.0、默认 Cargo
release profile 上完成，包含 5 次 warmup 与 50 次 measured invocation：

| 测量项               | Reference observation |
| -------------------- | --------------------- |
| Executable size      | 432,144 bytes         |
| Warm startup minimum | 1.179 ms              |
| Warm startup p50     | 1.466 ms              |
| Warm startup p95     | 2.921 ms              |
| Warm startup maximum | 4.046 ms              |
| Peak resident memory | 1,343,488 bytes       |

后续已接受切片只要改变 executable 的 reachable dependency graph 或 invoked startup path，就重新运行
并替换这份 reference。这个 staged module 无法从任何 invocation route 到达。受支持平台的 acceptance budget
仍为保留决策。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
pnpm verify
```
