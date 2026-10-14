# Windows Console Host

这是一个私有、实验性的 Rust boundary，用于在不重新打开 process-global standard stream 的
情况下使用调用方准入的 Windows console handle。Orchbee native application 是当前 consumer。

简体中文维护译本：本文档。
英文规范：[README.md](README.md)。冲突时以英文为准。

## 保留句柄合同

`ConsoleInput::admit` 与 `ConsoleOutput::admit` 接管调用方克隆的准确 `OwnedHandle`。所有 mode、
readiness、record、geometry 与 output 操作都使用这些 retained handle。本 crate 从不调用
`GetStdHandle`，不打开 `CONIN$` 或 `CONOUT$`，不使用 `std::io::stdout`，也不公开 raw Windows
handle 或 Win32 representation type。

调用方继续拥有 original handle、mode restoration journal、terminal lifecycle 与 output
ordering。endpoint 仅在最后一个 retained adapter 释放后关闭准入的 duplicate。input endpoint
不实现 `Clone`；application 在 startup 与 TUI phase 之间保留一个 canonical reader。

## 输入记录

`read_ready_record` 先调用 `GetNumberOfConsoleInputEvents`。count 为零时直接返回 `None`，且不
调用 `ReadConsoleInputW`；count 为正时只请求一个 record，并且只在 Win32 准确报告一个结果时
准入。readiness observation 与同步 read 不是针对外部 process 或另一个独立别名 OS reader 的
原子 reservation。调用方必须保证 single-reader ownership；竞争 reader 可能消费已观察到的
record，并导致随后的 Win32 read 阻塞。

safe record type 保留 raw Unicode UTF-16 unit、repeat count、virtual key 与 scan code、control
flag、绝对 screen-buffer mouse coordinate、button state、event flag、focus、resize、menu 与
unknown event tag。产品 key decoding、surrogate 与 repeat state、AltGr handling、mouse-button
history、window-origin subtraction 及 terminal-byte encoding 继续由 application 负责。

## Unicode 输出

每个 `ConsoleWriter` 都是在 retained output handle 上独立、有序的 UTF-8 stream。它最多保留
三个不完整 UTF-8 byte 与一个 pending low surrogate。每次 `WriteConsoleW` 最多接收 4096 个
UTF-16 unit；writer 必须把每个返回 count 通过 Unicode scalar boundary 映射后，才能报告已消费
的 UTF-8 byte。short write 停在 high surrogate 后只保留 low surrogate，因此不会重放已经接受
的 high surrogate。

zero count、invalid count、无效或不完整 UTF-8 及 native failure 都会锁定该 writer。其后调用
不再执行 native output。`ConsoleWriteError` 记录可信的 complete-scalar 或 split-scalar progress；
当 Win32 无法证明失败调用的 effect 时标记为 unknown。fresh writer 以空 state 开始，可以尝试
有界 lifecycle cleanup，且不会继续 failed writer 的 payload。调用方为每个 semantic stream
保留一个 writer，并在正常 drop 前 flush。

## 限制

本 crate 不解释 terminal input，不渲染 TUI，不选择 mode，不记录 restoration，不序列化独立
writer，不托管 pseudoconsole，不启动 process，也不适配 redirected byte stream。window geometry
以 cell 与有符号 screen-buffer origin 返回；pixel dimension 不属于此 boundary。

## 验证

从 repository root 运行原生 Windows check：

```sh
node scripts/run-rust-workspace.mjs test --package orchbee-windows-console-host
pnpm lint:rust
pnpm test:rust-runner
pnpm verify
```
