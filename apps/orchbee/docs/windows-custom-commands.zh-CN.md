# Windows 自定义命令

## 配置与目录模型

共享 Windows 设置所有者接受最多64项 `[[keys.command]]`。每项包含最多4096 UTF-8字节、非空且无NUL的 `command`，`type`（`shell`、`pane`、`popup`、`plugin_action`，默认 `shell`），可选 `key`（一个组合键或最多16个组合键）及最多512字节的可选 `description`。仅 popup 可以设置 `width` 和 `height`，接受不超过4096的正单元格数或1%-100%的百分比。省略按键产生未绑定目录项。任一配置命令包含未知字段、重复／不安全／保留组合键、无效命令或不适用于该动作的尺寸时，整个配置命令集合都会禁用并发出有界警告。不会执行部分或未经验证的命令；其他有效设置和内置动作仍会接纳。完整设置文件／profile仍限制64 KiB。

自定义和内建按键共用规范化、有效 prefix 及冲突规则。运行时通过既有规范 keys profile 发布；配置重载将完整已接纳的命令／绑定集合及其诊断一起安装，后续修正后的发布会恢复配置命令。致命文件/TOML 失败保留此前发布。目录 ID 绑定完整规范化 profile 和有序项索引，属于不透明值；任意命令／绑定定义变化时改变，不能接收可执行文本或授予进程权限。当前命令 ID 只解析一个声明动作，过时或不存在的 ID 在副作用前失败。

## 调用所有权模型

原生命令发现、CLI调用和TUI绑定消费同一已安装配置。调用核对请求的 Workspace／Tab／Panel 父子关系及当前来源成员；指定目标时通过既有已观察聚焦路径切换。Plugin选区按照准确终端内容版本读取，不相关或过时选区拒绝。CLI选区坐标与Server来源观察器共用terminal protocol绑定中的纯文本行提取器，处理宽字符、自动换行及plugin上下文字节上限。调用继续之前，临时观察器负责完成detach结算。Shell与plugin动作可以没有Pane上下文；Pane／popup要求当前来源已准入。

Shell通过Worker拥有的进程执行配置的Windows命令解释器，不向终端输入。Pane使用常规规范终端创建及聚焦；popup采用独立明确类型的终端lane及其输入／缩放／关闭所有者。Plugin action沿用既有action解析及日志。各类动作不伪装成已安装plugin。来源上下文属于显示元数据而非启动权限；托管Session路由和实际Worker放置保持权威。超时、取消、答复丢失或畸形后不重放已提交副作用；取消等待不撤销已接受副作用。

Worker在carrier二十秒答复预算内，最多等待五秒确认shell物理启动。窗口内较慢的进程创建仍可产生确认回执。确认缺失或不确定时，保留进程所有者并报告物理结果不确定；迟到结果不允许重放，也不会悄悄清除不确定状态。定向测试覆盖延迟确认、重复准入、超时、迟到完成及join后的关闭。

TUI命令交换使用独立可取消连接和负责join的线程，等待命令时不暂停Client心跳或popup交付。至多一个命令处于排队或等待回执状态。空Workspace可以调用全局shell／plugin动作；Pane／popup仍要求准确已绑定来源。快捷键发现净化展示文本，配置中保留字面命令。嵌套回执必须对应声明动作，并匹配请求、Session及popup选择器；即使其他协议结果本身有效，也拒绝与当前动作无关的结果。

## 参考与证据

参考为Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `app/custom_commands.rs` 与 `api/schema/commands.rs`：四类配置动作、endpoint发布ID、可选目标聚焦及带内容版本的plugin选区。原生配置profile提供命令身份边界，既有运行时所有者提供进程与物理终端权限。解析／profile检查仅证明共享配置基础；完成还需要每种动作的实际目录／调用／按键消费者、相关联的拒绝／unknown及生命周期证据，以及最终Windows流程验收。本模型不增加Desktop或其他平台命令实现。

定向证据包括真实Windows shell环境／退出处理、Worker后台所有权及重复／冲突／关闭检查、CLI可执行进程的目录／调用／拒绝／unknown用例、Server已安装目录／来源／权限检查、TUI绑定／发现／来源失效及actor取消，以及Rust／TypeScript／JSON共享的[一致性语料](../../../schemas/workbench/v1/workbench-configured-command-conformance-v1.json)。原生命令输出为 `orchbee.command` v1；过期ID使用封闭的 `orchbee.cli.failure.command` v1文档。配置popup控制使用v11，在v1-v10下仍无效。这些定向检查不声明最终全cohort或交互式Windows验收完成。

配置shell见证在原有三秒预算内等待完整的command-ID回执；文件创建本身不能证明写入完成。夹具在成功路径显式检查清理。panic展开期间，清理失败报告准确保留目录，不替换原始panic。反例覆盖初始空回执，以及暂不允许删除目录的消费者：原始panic保持可观察，消费者释放句柄后再回收目录。
