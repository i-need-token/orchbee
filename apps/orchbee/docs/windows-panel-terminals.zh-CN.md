# Windows Panel 终端创建

`orchbee panel create-terminal [<panel>|--panel <target>|--current] [--client <id>]
[--child|--presentation <id>] [--label <label>] [--exec <path|name> --arg <value>...|--profile <selector>]
[--cwd follow|home|current|workspace|<absolute-path>] [--env NAME=VALUE] [--timeout <ms>]`
在既有 Panel 内创建终端，不分割其几何区域，仅支持 Windows。
省略成员选择时绑定未绑定的主项；`--presentation` 选择该 Panel 内精确的未绑定主项或子项；
`--child` 追加新子项，最多 16 个。已绑定目标会拒绝，不会选择其他 Panel 的 Presentation。
`--label` 仅用于子项，默认使用下一个数字成员标签。绑定既有成员保留其标签及输入策略。

显式 Panel 无需 TUI。省略／当前 Panel 先使用经过验证的调用者，再选择唯一 Ready Client；
`--client` 指定该 Client，与显式 Panel 冲突。目标使用既有 Panel ID／句柄。
命令不改变 Client 焦点。精确成员焦点和单独成员移除仍是独立的生命周期义务。

launch／profile 互斥。几何尺寸使用既有列／行／单元格像素选项。
参数和环境变量值保持字面值。沿用创建配置及 cwd 策略：follow 仅读取所属 Panel 主项的进程 cwd，
未绑定主项不会借用子项；运行时 cwd 不可用则使用启动默认值。运行时发现前完成输入校验。
总超时默认 30000 ms。

CLI 解析一份规范快照，只提交一次保留的 `panel.create-terminal` 请求。
Server 负责授权、修订／Close 约束、身份分配、启动及结算。
`orchbee.panel.terminal-create` v1 保留 Session／请求／基准修订、精确目标、目录来源及完整的等待或最终结果，
不导出 argv 或环境变量值。Pending 代表已接受，尚未确认启动。
已知物理失败／不可用和结果不确定均返回非零，并保留已提交身份。
写入后回复丢失、无效或关联不符会退役连接并报告不确定，不重试、不补偿关闭。
信号清理保留已记录结果，并在需要时返回取消退出码。

共享创建适配器也服务布局结构创建／绑定。匹配的原生 CLI／Server 二进制一起上线及回滚，
不增加持久化迁移。解析器／目标／回执检查及作用域内 Windows CLI 对端验证边界；
真实 provider 和完整 TUI 验收继续纳入最终 Windows 回归。

程序名使用最终 CLI 的 PATH，回执保留解析出的绝对路径；参见[可执行程序名](native-terminal-creation.zh-CN.md#可执行程序名)。

<!-- documentation-pairing: english-sha256=b0463a35e94eb089c8ac99e2254209b5cf7f260b1f6cc3359201e01e485d6e88 zh-cn-sha256=c0852de75ab7bca5dfca97d8fd38b5bb471c847a1e888dc9f10f973a03ba8efd -->
