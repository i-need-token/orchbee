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

Production code 只使用 Rust standard-library argument、environment、stdout 与 stderr capability。
它不打开 file 或 socket，不启动 process 或 thread，不初始化 terminal mode，也不读取 product
state。在另行接受的 packaged cutover 前，已交付 TypeScript [`apps/cli`](../cli/README.zh-CN.md)
继续作为当前 CLI 权威。

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

后续已接受切片只要改变 executable dependency graph 或 startup path，就重新运行并替换这份
reference。受支持平台的 acceptance budget 仍为保留决策。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
pnpm verify
```
