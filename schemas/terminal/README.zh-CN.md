# 终端契约

## 权威

本目录拥有语言中立的 Terminal 线协议契约。[当前参考](references/protocol.zh-CN.md)定义布局和语义；
[版本政策](docs/version-history.zh-CN.md)管理五个独立的 OBTP 版本域。
唯一的 `v1/` 目录保存当前契约，其五个版本仍为 `0.1.0`，不引入第六个线协议版本。

[`v1/constants.json`](v1/constants.json) 是手工维护的注册表，登记 magic、格式版本与头部大小、
消息种类及其角色／编码／成功响应契约、信封枚举编码和协议限额。修改时同步参考文档和所有受影响绑定。
Rust、Zig 和 TypeScript 保持独立实现；绑定和夹具生成器都不会生成注册表，也不从中生成源码。

## 消费者与证据

- `crates/terminal-protocol` 在私有 `wire_constants` 测试模块读取注册表，比较解码器实际使用的常量、
  编码后的角色标志、载荷枚举编码、完整消息注册表及其已实现的传输限额。
- `apps/terminal-host` 通过一致性构建选项读取有界的注册表与夹具字节。其协议套件比较五种格式、
  每项消息契约、信封枚举编码和全部共享限额。Host 构建不依赖 TypeScript 包。
- `packages/terminal-protocol` 实现 TypeScript codec 和 schema。其注册表测试比较五种格式、全部消息契约、
  实际编码的信封枚举值及限额。私有格式常量通过真实编码器输出观测，规范投影仅作为输入；
  注册表始终提供独立期望值。包只导出代码，仓库消费者直接从本目录读取夹具。
- 本目录变更由既有受影响验证解析器选择全部三种绑定及其传递消费者。TLA 登记消费本权威及原生绑定。

注册表检查替代针对 TypeScript 生成的常量快照的对拍。既有的畸形输入、严格边界、原生生产和消费者语义
证据仍然必须保留；常量一致本身不能证明 codec 或运行时行为。

## 夹具与维护

[`v1/fixtures/`](v1/fixtures/) 拥有规范记录及一致性语料。文件名不带版本，内容携带当前格式版本。
TypeScript 绑定既有的 `fixtures:generate` 和 `test:fixtures` 命令在本目录生成和检查 codec 夹具。
`canonical.json` 是生成器输入；`control-conformance.json` 保存载荷用例，不再保存第二份常量注册表。

Terminal Host 既有的 `surface-fixtures:generate` 和 `test:surface-fixtures` 命令生成并逐字节比较三个
`ghostty-*.bin` 记录。终端渲染器消费相同记录进行语义检查。原生生产、夹具权威和消费者断言各自保持职责。
