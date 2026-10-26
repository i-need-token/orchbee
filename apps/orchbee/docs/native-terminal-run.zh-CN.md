# 原生终端命令提交

## 命令与所有权

`orchbee terminal run <target> <command>` 向既有 Terminal 提交一个带引号的非空 UTF-8 命令。操作数类似选项时使用 `--`。命令字节、空白、引号和换行保持原样；拒绝分开的多个命令操作数。参数包上限为 64 KiB，paste 上限为 256 KiB。目标终端中的当前程序解释命令。

未加 `--wait` 时，一次快照解析目标，一次输入请求发送 paste 和 Enter。该路径遵循五秒发送预算和 `orchbee.terminal.send` 回执契约。回执只确认输入进度。部分或未知投递绝不重放。不获取 viewer、controller、resize 或焦点权限。

## 关联完成

Windows 的 `--wait [--timeout <ms>]` 使用只读 viewer 和受管 PowerShell 集成。总期限默认为 30000 毫秒，包含发现、就绪观察、提交、轮询和清理。集成不受支持或尚未就绪时，在输入前拒绝。调用者须拥有空的可编辑命令行；viewer 不排除并发用户输入。

CLI 生成随机调用 ID，并发送带注释前缀的一次 paste/Enter 请求。经过校验的 PSReadLine 包装器仅剥离精确前缀，将原命令返回 PowerShell，不在新作用域求值。私有 APC 标记在命令开始和完成时回传 ID。无关的 OSC 133 完成不能结束本次等待。Host 只保留最近一次关联调用；缺失或被覆盖的证据可能导致超时，绝不授权重放。这是协作式 Shell 证据，不是针对终端进程的身份认证边界。

JSON 使用 `orchbee.terminal.run` v1。`submission` 保留完整输入回执；`completion` 单独包含 `exitCode`、`statusKind: powershell-success` 和物理输出坐标。PowerShell 的 `$? = true` 对应 0，否则为 1；不猜测原生进程退出码。generation 或行原点变化后，输出范围的 `retained` 为 false。`failure` 保留超时、取消和传输事实，不抹去已接受输入或已观察完成；`cleanup` 记录 viewer 清理义务。取消不会停止 Shell 命令。JSON 是恢复格式，脚本工作应在提交前选择它。

## 新输出边界

Host 能识别主屏行原点时，`terminal read --format json` 提供不透明游标。将其传给 `terminal wait-output <target> --after <cursor> --contains <text>`，或使用 `--after now` 从第一个观察帧开始。只匹配后续物理行；排除占用的边界行以及提示符／输入行。已有输出不会被算作新输出。

Host 重启、generation 变化、历史淘汰或超出所选窗口的间隙会拒绝游标。选择足够的 `--lines` 窗口和显式 `--timeout`。新游标代表新的观察边界，不会恢复丢失输出。参见[终端输出](native-terminal-output.zh-CN.md)。

## 证据与兼容

解析、游标、线格式解码和 Host 读取批次用例覆盖精确关联、无关完成、无效记录及不重放。Windows 命名管道用例覆盖既有单次 paste/Enter 回执路径。证据来自同作者实现测试，不声称独立审查或穷尽调度证明。帮助、补全适配器、离线 Agent 指南与 schema 同步演进。旧二进制拒绝新选项；既有会话须使用新初始化的受管 PowerShell 才支持关联等待。

<!-- documentation-pairing: english-sha256=4ff6c2d5d95038b9f2a591c951262b982f20946e0800eca2f6fe072ecc681de5 zh-cn-sha256=fb98aa495843959542998dd15b0824a85130bb376af71a5049af6b64d4119162 -->
