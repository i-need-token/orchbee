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

对于一个可信、已存在且为 exactly normalized absolute 的 per-user runtime base，确切 derived
path 是：

| 角色               | 稳定路径                                         |
| ------------------ | ------------------------------------------------ |
| Scope directory    | `<runtime-base>/<site-scope>`                    |
| Activation lock    | `<runtime-base>/<site-scope>/activation.lock`    |
| Site lifetime lock | `<runtime-base>/<site-scope>/site-lifetime.lock` |
| Client socket      | `<runtime-base>/<site-scope>/client.sock`        |
| Worker socket      | `<runtime-base>/<site-scope>/worker.sock`        |

`ModeCheckedLocalSiteRendezvous` 刻意在名称中标明尚未完整的 security boundary。Preparation 只会在
scope directory 不存在时创建它。Preparation 与 validation 把 `/` 作为 trusted root 打开，然后用
`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC` 逐个 descriptor-relative 地打开 runtime-base component 与
受控 scope。它们既不 canonicalize，也不让 operating system 对传入 base 执行 normalization。除 `/`
本身外，可接受 base 只含一个 leading slash、由单个 slash 分隔的一个或多个 nonempty normal
component，不含 trailing slash、`.` 或 `..` component，也不含 NUL byte。Malformed base 在 trusted
root 之外的 filesystem access 发生前被拒绝；symbolic-link 或 non-directory component 会在该
component 处被拒绝。保留的 scope 必须是 effective UID 拥有且 exact mode 为 `0700` 的 directory。
新 scope 会在不 follow leaf 的情况下接受检查，验证其 directory type 与 effective-UID ownership，在
依赖 permission 的 open 之前以 descriptor-relative 方式改为 `0700`，然后仅在打开 descriptor 的
device 与 inode 和 chmod 前检查的 entry 一致时才 admission。即使 process umask 移除了全部请求的
permission，这也能在不改变 process umask 的情况下达到 exact `0700`。已有的错误 mode 会被拒绝且
不会修复。

每项 lock operation 都通过保留的 directory descriptor，以
`O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` 只打开或创建固定 leaf。Leaf 必须是 effective UID 拥有且
exact mode 为 `0600` 且 hard link 数恰好为一的 regular file。已有且有效的 stale file 会被复用，
不会被 truncate 或 replace。`try_acquire_activation_lock` 与 `try_acquire_site_lifetime_lock` 各自只执行一次
nonblocking exclusive kernel lock attempt，并返回 owned guard 或 `Contended`；它们不会 wait、
retry 或推断 process health。File creation 与 advisory-lock acquisition 是分开的：返回 `Contended`
的 caller 可能先创建并规范化了此前缺失的 durable lock inode，之后另一个 descriptor 才赢得 lock。
Contended attempt 绝不会 truncate、replace、unlink 或改变 existing inode 的 ownership，也不会修复
pre-existing invalid entry。Drop guard 只释放其 advisory lock。该 module 绝不移除 runtime
directory、durable lock file、socket path 或这些 path 上的任何 entry。

在 inherited 或 extended ACL 可以授予 mode bit 之外访问权限的 filesystem 上，这个底座尚未建立
production-private runtime boundary：它既不检查也不移除 ACL。在具备 ACL 能力的 boundary 补齐
这一缺口之前，public activation 必须保持 unwired。它也不选择 runtime base 或 scope，不验证
Unix-socket path length 或 socket metadata，不 bind、connect、认证 peer、启动 process、等待
readiness、unlink endpoint，也不声称 stale-process recovery。

该 primitive 仍由 `apps/orchbee` 拥有，因为本切片中只有 public Client-side activation boundary
具备已接纳的需求；不存在第二个 source consumer。
`apps/orchbee-server` 不能导入另一个 application，而且尚未获取 `site-lifetime.lock`；后续 Server
lifecycle slice 必须先接纳一个真实 shared owner 或 explicit composition seam，两个 application
才能使用同一 contract。

### 建模与证据

| 字段         | 当前暂存 contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | 为后续 local Client activation workflow 提供一个 path-component-safe、same-effective-UID、经过 mode 检查的 rendezvous identity，同时不提前实现 process activation、readiness、cleanup 或 recovery。                                                                                                                                                                                                                                                                                                                                                                                       |
| Inputs       | 一个可信、已存在且为 exactly normalized absolute 的 runtime base；在 63-byte ASCII boundary 解析的 installation-owned scope string；effective UID；以及 adversarial directory 和 lock entry。Relative 与 non-normal base、scope 缺失、link、错误 type/owner/mode、stale file 与已持有 lock 都是显式 case。                                                                                                                                                                                                                                                                                |
| Data         | Canonical `SiteScope`、确切 derived path、一个保留的 scope-directory descriptor，以及每个 acquired advisory lock 的一个 owned file descriptor。不包含 PID、readiness marker、process identity、durable record payload 或 socket state。                                                                                                                                                                                                                                                                                                                                                   |
| Outputs      | Preparation 或 validation 返回保留的 rendezvous 或 typed failure。每次 lock attempt 返回 `Acquired`、`Contended` 或 typed filesystem failure。Client 与 Worker socket method 返回确切 stable path且不执行 I/O。                                                                                                                                                                                                                                                                                                                                                                           |
| Invariants   | 每个 runtime-base component 与被打开的受控 directory 或 lock entry 都使用 no-follow；新初始化 scope 的 opened identity 与 chmod 前检查的 entry 一致；已接受 entry 在 admission 时具有 exact type、effective-UID ownership、mode 与 lock link count；已接受的 activation 与 lifetime leaf 在 admission 时无法 alias 同一个 inode；contended lock attempt 绝不 truncate、replace、unlink、改变 existing inode 的 ownership 或修复 pre-existing invalid entry，但可以创建并 mode-normalize 缺失 lock inode；drop resource 不移除任何内容；scope identity 独立于 executable cohort。          |
| Decisions    | Exact normalized-base syntax、每个 descriptor-relative base-component result、scope syntax 与 length、create-versus-validate policy、descriptor metadata、lock kind、create-versus-existing lock file，以及一次 kernel lock outcome 完全决定结果。                                                                                                                                                                                                                                                                                                                                        |
| Model        | 一个纯粹且有界的 scope/path resolver 向一个从 trusted root descriptor 开始且不执行 canonicalization 的 POSIX adapter 提供输入，随后保留 validated scope descriptor。该 adapter 不包含 retry loop、clock、process observer 或 recovery branch。                                                                                                                                                                                                                                                                                                                                            |
| Transitions  | 即使 umask 完全 restrictive，缺失 scope 也可以变成一个 identity-matched validated directory。缺失 lock leaf 可以变成一个 durable validated regular file，包括其 creator 最终返回 `Contended` 的情况；creation winner 与 advisory-lock winner 不必是同一个 caller。Unlocked leaf 可以变成 guard-held，并在 guard drop 时回到 unlocked。已有 invalid entry 保持不变。                                                                                                                                                                                                                       |
| Dependencies | Rust path/value operation、用于 effective UID 与 descriptor-relative POSIX call 的 safe `nix` wrapper，以及用于一次 advisory exclusive attempt 的 `File::try_lock`。不存在 network、socket、child-process、thread、time、randomness、persistence payload 或 Server dependency。                                                                                                                                                                                                                                                                                                           |
| Ownership    | Typed scope 来自未来 installation composition；本 application 拥有 derivation、directory validation、lock-file validation 与 guard lifetime。未来 activation workflow 将拥有 deadline 与 process sequencing；Server 必须另外获得已接受的 lifetime-lock seam。                                                                                                                                                                                                                                                                                                                             |
| Verification | Owner-local deterministic test 覆盖 valid/invalid scope name、确切 path、relative 与 malformed lexical base、trailing slash 与 dot/parent component、intermediate/final base link、scope link、restrictive-umask initialization、concurrent permissive-umask creation、non-directory、错误 owner/mode、lock symbolic/hard link、directory、FIFO、错误 metadata、独立 contention、stale inode/content preservation、reacquisition 与 cleanup non-ownership。                                                                                                                               |
| Limits       | Test 在一个 effective user 下运行，并信任 kernel metadata 与 advisory-lock semantics。它们注入不同的 expected UID，而不是执行 cross-user。Concurrent creation 在 permissive umask 下执行；如果另一个 process 的 umask 限制 user bit，本 primitive 不会在 separate caller 观察到其短暂 pre-chmod window 时 retry。Durable 表示该 owner 绝不 unlink 或 truncate lock file，并不表示对其执行 fsync；kernel filesystem call 不具有 wall-clock guarantee。此 atomic primitive 不选择 TLA+ model；已接受的未来 activation workflow 仍须提供其 singleton/process-order model 与 total deadline。 |

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
