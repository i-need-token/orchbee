# Desktop Terminal Host 更新连续性

| 字段     | 内容                                                                                                      |
| -------- | --------------------------------------------------------------------------------------------------------- |
| 状态     | 已接受的未来设计；版本化 Host 安装与更新激活尚未实现                                                      |
| Owner    | `apps/desktop` 维护者                                                                                     |
| 当前权威 | [`apps/desktop/README.md`](../../apps/desktop/README.md) 以及 Terminal 各所属 package 和 application 指南 |
| 退出条件 | 实现已接受行为，把维护事实迁入 Desktop 所属指南和测试，然后删除本提案                                     |

英文 [desktop-terminal-host-update-continuity.md](desktop-terminal-host-update-continuity.md)
是 canonical 版本；本文用于简体中文评审。

## 决策

Desktop 更新不能仅仅因为 Electron 重启或捆绑的 Host 版本发生变化，就终止仍被保留的终端进程。
Desktop 拥有 Host 安装、发现、兼容性检查、激活、回滚和清理。Terminal Host 不拥有产品更新策略或
发布通道。

## 版本化安装

- 将已验证的 Host 二进制安装到应用数据运行时目录，并按平台、架构、版本和发布通道区分。
- 签名清单标识 Host 产物、摘要和精确协议契约。
- Endpoint 元数据标识活动 Host epoch、可执行文件版本、精确协议、进程和发布通道，不包含终端内容。
- 即使 Desktop bundle 包含更新的 Host，也要在 Desktop 和 renderer 重启后复用兼容的运行中 Host。
- Host 仍拥有保留终端时，绝不替换它。

## 激活

Desktop 启动时：

1. 发现同一用户、同一发布通道的活动 Host 并完成协商。
2. Desktop 与 Host 声明同一个精确协议时复用该 Host。
3. 通过 `CreationId` 将 Desktop recovery binding 与 Host inventory 对账。
4. 使用新的 attachment identity 和新 projection 重新连接保留终端。
5. 只有活动 Host 不再拥有保留终端时，才激活更新的 Host。

Updater 可以在终端被保留时下载更新。兼容的 Desktop 更新可以重启 Electron 并重新连接运行中的
Host。如果新 Desktop 无法与运行中的 Host 通信，则保持延迟激活，直到保留终端被关闭，或存在另一条
经过明确接受的恢复路径。

## 失败与清理

| 失败情况                      | 必需行为                                        |
| ----------------------------- | ----------------------------------------------- |
| 新 Host 在拥有终端前启动失败  | 停止它并回到上一个已验证的兼容 Host             |
| 存在保留终端时发生协议不匹配  | 保持兼容 Desktop 和 Host 可用，不替换终端 owner |
| Renderer 或 Electron 重启     | Detach 客户端 projection，并保留 Host terminal  |
| Host 崩溃、OS 重启或 PTY 丢失 | 报告丢失，绝不声称进程连续                      |

只有在没有活动进程使用旧 Host、没有受支持的已安装 Desktop 需要其协议契约，并且回滚策略不再保留它时，
才能删除旧 Host 二进制。

## 保留决策

实现切片仍需分别接受：

- 更新提供方和签名清单传输；
- 签名及平台信任集成；
- 发布通道策略，以及仅在已发布产品产生真实兼容需求后，对多个协议契约提供支持的策略和 wire 表达；
- 安装器与卸载行为；
- 操作系统关机行为；
- 某个版本有意移除与保留 Host 的兼容性时，面向用户的恢复方式。

在 Electron 旁放置一个开发 Host 不构成本设计已经实现的证据。网络共享、远程客户端、Host 或 OS
丢失后的 PTY 持久恢复，以及 Host 内的产品拓扑都不属于本提案。
