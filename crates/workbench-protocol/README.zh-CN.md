# Workbench 协议

本 crate 管理原生 Workbench 客户端与 Server 共用的请求、响应、校验和帧传输契约。它不打开套接字或执行操作；运行时所有者负责处理已受理请求。

## 原生插件运行时契约

封闭的插件调用/log/get/cancel 请求及结果使用同批 cohort 的 OBWP 45/46，分别限制为 32 KiB 和 8 MiB。上下文只包含展示数据；受理 Server 解析执行意图，Worker 拥有进程。日志游标固定 Worker 身份/generation 和递减序号；修改投递结果未知时显式返回。Rust、TypeScript 和 JSON 共用插件一致性语料。[原生插件行为](../../apps/orchbee/docs/windows-plugins.zh-CN.md) 管理运行时生命周期和剩余集成。

`tab.create` 和 `panel.split` 的启动选项可包含 `pluginPane: { pluginId, entrypointId }`。两者均为有界 ASCII 目录名称（120 字节，入口名不含点）。选择器与程序／profile 互斥，Workspace 或子终端创建拒绝使用。缺省和 null 可选选择器规范化为省略，旧请求序列化保持不变。Server 解析与受保护的执行环境归终端受理所有，不由此不含展示数据的选择器决定。

插件 PaneGet 为只读操作，PaneFound 返回非零规范修订号、封闭选择器、精确 context／Tab／Panel／Presentation 及绑定。响应关联必须匹配请求的 Panel；结果不是 action Job 回执或调用者展示上下文。
