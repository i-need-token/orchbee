# Terminal protocol Rust 绑定

语言中立的[终端权威](../../schemas/terminal/README.zh-CN.md)拥有线协议参考、版本政策、
常量注册表及夹具。本独立 Rust 绑定直接读取这些夹具；私有注册表测试比较生产常量，
不为测试增加公共导出。

## 共享 SGR 样式投影

`terminal_sgr_style` 无分配地格式化一份有效 cell 样式。原生 CLI 终端导出与 Server 事件匹配
共用其规范 reset、标志、下划线和 RGB 序列。这些序列描述已捕获样式，不宣称与历史 PTY
输入字节相同。

本 crate 解码并校验原生运行时所有者使用的有界终端帧与行窗口契约，
不执行进程、viewer、文件或终端 I/O。

## 有界帧元数据

文本帧解码器公开每行已有的提示分类及现有扩展 4 完成事实：
精确 25 字节，序号与完成时间戳必须为正，时长以毫秒计，退出码为单字节。
重复完成或图形扩展拒绝整帧。扩展 2 图形保持不透明：
只有精确有效的空 OBKG 头能证明无图形；其他负载保守标记为有图形。
增量帧省略图形扩展表示不变，不表示为空，由投影所有者继承该状态。
尾部展示消费这些事实，不猜测终端输出。
完成资格、去重和通知投递归客户端；帧扩展测试负责畸形及边界情况。

## 通过版本准入的文本选区

`extract_terminal_text_selection` 从已经捕获的 `TerminalTextWindowRows` 提取包含端点的选区。
它检查尺寸、总行数、连续行身份、端点成员关系、4096 行跨度和调用者指定的 UTF-8 字节上限。
宽字符及组合字符尾只选择一次；只有已证明的软换行续行才不插入换行。
硬行结束时移除尾部空格。无效或不完整窗口返回协议错误，不返回部分文本。

调用者负责物理来源成员关系、内容版本准入及 viewer 回收。
原生 CLI 选区与 Server 配置式插件命令共用这一纯提取器；各自不同的输出／上下文字节预算
在相应边界保持明确。

## 传输注册表查找

传输注册表的数值 kind 唯一且严格递增，在二分查找前由编译期检查保证。
79 沿用已有相关联 `ReadProcessInfo` 请求／响应的 control JSON 契约；80/81 保留
graphics-image 契约。进程信息 payload 往返也经过携带 correlation 和 Host epoch 的
传输编码／解码，避免已注册 payload 被悄然视为未知必需传输消息。
Event role 仍被拒绝，已有 payload 上限不变。

## 只读 PNG 解码

80/81 消息使用 graphics-image 编码（5）以及借用视图 `GraphicsDecodeRequest`／`GraphicsDecodeResult`。OBPI 请求有 40 字节头，包含附件、声明尺寸和精确 PNG 长度；OBPX 结果有 56 字节头，包含附件、终端、尺寸和精确 RGBA 长度。绑定检查非零身份、PNG 头一致性、1–4096 尺寸、16 MiB 输入和 32 MiB 解码像素。PNG 内容解码和物理读取授权归 Host 所有；Worker 和 Server 在消费像素前检查信封关联、epoch 和目标。此编码有独立上限，不扩大原始键盘或粘贴限制。

<!-- documentation-pairing: english-sha256=d2cfbf0c6e09e49dc2fb672879cf2545d320adbf79a9c32d6944246a7efeaf59 zh-cn-sha256=55a2ffe76ac7483c3bc8d28d566651806744042ce7e855d687d2eb50d11bf210 -->
