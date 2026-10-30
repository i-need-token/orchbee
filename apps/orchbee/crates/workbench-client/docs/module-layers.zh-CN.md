# Workbench Client 模块分层

状态：此 crate 已采纳的分层与横向依赖契约。当前代码尚未符合；`pnpm lint:rust-modules` 会报告
剩余的环和指向高层的引用，但不会因此失败。

本文是[模块边界](../../../../../docs/zh-CN/MODULE-BOUNDARIES.md#原生源码树)要求此 crate 具备的
owner 本地契约。[应用二进制](../../../docs/module-layers.zh-CN.md)对自身模块使用同一套分层。
[同级传输层](../../local-client/README.zh-CN.md#模块方向)保留一份单独的横向依赖清单。

## 分层

各层按从低到高排列。

| 层             | 拥有                                                                                                                                                                              | 禁止                                                                         |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `values`       | 身份、令牌、类型化事实、几何、组件控件状态类型、动作定义及其发现元数据、本地化消息目录、主题角色，以及基于它们的确定性函数，例如事件解码和文本投影                                | 拥有跨越单次调用的状态、执行 I/O，或读取时钟与环境                           |
| `state`        | 某一份 Client 状态的唯一写入者及其同步转换，按[状态所有者](#状态所有者)分类                                                                                                       | 执行传输、终端或进程 I/O，绘制，启动工作流，或铸造规范 Workbench 事实        |
| `workflow`     | 多步骤和有副作用的操作：连接任务与 lane、观测与轮询作业、激活、Agent 启动、终端模式与退出生命周期，以及它们借以触达传输、宿主终端、信号、剪贴板、文件和进程的、以平台命名的适配器 | 绘制、路由输入，或写入其他 owner 的状态；把自身结果结算进该 owner 的情况除外 |
| `presentation` | 从状态和工作流状态派生的只读视图模型、布局几何、渲染、渲染记录的帧命中图与交互投影，以及共享 TUI 组件；布局从不引用渲染                                                           | 修改功能状态、启动副作用或解析输入                                           |
| `input`        | 把解码后的宿主输入投递给当前捕获 owner、依据最近发布的帧解析指针手势、解析绑定，以及转换为功能意图或动作                                                                          | 拥有功能状态，或通过其发出的意图和动作之外的途径执行副作用                   |
| `composition`  | 生命周期与接线：Workbench Client 连接任务与 UI 循环的组合、功能动作的执行、端口实现，以及 crate 的导出门面                                                                        | 持有可由更低层拥有的业务规则                                                 |

## 依赖规则

- 模块可以引用自身所在层和更低层。低层从不引用高层，无论通过路径、`use`、重新导出还是类型
  别名。
- 后代模块继承父模块的层，除非[模块归属](#模块归属)单独列出它。父模块从不重新导出或使用被归入
  更高层的后代模块。
- 同一层内的引用保持无环。两个模块互相需要时，把共享部分移到两者中较低的那个，或移到更低的层。
- 外部协议、运行时和渲染 crate 位于 `values` 之下。同级 Client Connection 传输层的句柄只能经
  `workflow` 模块 `local_client` 触达；状态所有者结算其交换结果值，但不持有句柄。
- 操作系统 `cfg` 选择属于路径命名了平台或能力的 `workflow` 适配器，以及 `composition`；
  `host_capabilities` 准入表是唯一的 `values` 例外。其他模块保持平台中立。
- 应用只通过 `application` 消费此 crate；crate 内没有任何模块引用它。`cfg(test)` 或
  `test-support` feature 下的代码位于分层图之外，也从不扩大生产可见性。

## 状态所有者

每一份 Client 状态都只有一个写入模块。其他模块通过共享借用读取它，或接收派生值；渲染只读取
派生的视图模型。

| 来源            | 写入者契约                                                                                                         | 所有者                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| --------------- | ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Server 投影     | 只应用经过验证的 Server 或 Worker 结果与事件；从不铸造规范成员关系，也从不因投影缺失推断物理清理                   | `workbench_client::workbench_projection`、`workbench_client::state`、`workbench_client::agent_board`、`terminal_projection`                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Client 本地视图 | 拥有只存在于此 Client 中的界面框架、选择、焦点、草稿、菜单、提示、复制模式、偏好和位置；只通过自己唯一的存储持久化 | `client_view`、`action_menu`、`machine_picker`、`machines`、`native_settings`、`notifications`、`onboarding`、`terminal_copy_mode`、`transient_notice`、`workbench_client::command_notice`、`workbench_client::completion_reminders`、`workbench_client::copy_episode`、`workbench_client::normal_exit_hold`、`workbench_client::startup_directory`、`workbench_client::startup_preference`、`workbench_client::terminal_selection`、`workbench_client::terminal_viewport`、`workbench_client::ui::history_positions`、`workbench_client::ui::retained_frames` |
| 连接生命周期    | 拥有一个 Client Connection 的队列、关联、代次、保留证据和唤醒；由 `workflow` 连接任务驱动                          | `workbench_client::mailbox`、`workbench_client::wakeup`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

`ClientView` 和 `WorkbenchClientUi` 共享四个值。每个值只有一个写入者，另一方经组合层接收所有者
的值：

- 主机能力是编译期准入。两方各在构造时接收一次，此后没有写入者。
- `ClientView::apply_user_preferences` 写入已确认的用户偏好。`WorkbenchClientUi` 在同一次结算中
  接收所有者的值。
- 配置观察者把通知策略写入 `ClientView` 的通知状态。`WorkbenchClientUi` 连同准入它的修订号一起
  接收该策略。
- `workbench_client::copy_episode` 拥有复制模式活动。`ClientView` 保留一份镜像，只由
  `settle_copy_mode` 按所有者当前的回答写入。

`ClientView` 中的 Client 本地提示和 `workbench_client::command_notice` 中的 Workbench 命令提示
是不同的事实，各有各的写入者。

## 组件

[TUI 组件](../../../../../docs/zh-CN/tui/components.md#所有权与准入)中的层级按如下方式映射到
各层。

| 层级     | 位置                                                                                                                                                                                                 |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 基础控件 | `presentation` 中的共享组件模块，位于 `tui_render::controls` 下，清单见 [TUI 组件参考](tui-components.zh-CN.md)；它们的控件状态类型属于 `values`，绘制出的区域记录到帧命中图。功能模块从不分叉它们。 |
| 重复组合 | 编排基础控件的共享 `presentation` 模块，例如列表、选择器、模态窗口和快捷键栏；浮层框架和模态组合位于 `tui_render::modal` 下。它们不拥有功能副作用。                                                  |
| 业务表面 | 所属功能模块。每个子模块只属于一层：状态在 `state`，视图模型和绘制器在 `presentation`，手势到意图的映射在 `input`，副作用在 `workflow`。归属表列出每个与父模块不同层的子模块。                       |

## 模块归属

每行给出目标层。模块目前混合多个层时，说明会指出需要移走的部分。

| 模块                                          | 层             | 职责与理由                                                                                          |
| --------------------------------------------- | -------------- | --------------------------------------------------------------------------------------------------- |
| `action_menu`                                 | `state`        | Client 本地的菜单捕获与选择，以及菜单目标展示哪个前缀绑定。                                         |
| `action_projection`                           | `values`       | 从 `client_actions` 投影出的动作身份、类别、色调和发现元数据。                                      |
| `agent_provider`                              | `values`       | 可识别的 Agent 身份、规范命令名，以及从 `orchbee-agent-integration` 重新导出的受管集成身份。        |
| `agent_runtime`                               | `workflow`     | Agent 执行端口，以及 Agent 启动消费的观测事实。                                                     |
| `agent_start`                                 | `workflow`     | CLI 与 Client 界面共用的类型化 Agent 启动。                                                         |
| `application`                                 | `composition`  | 面向应用二进制的导出门面。                                                                          |
| `background_thread`                           | `workflow`     | 共享退出契约下的 Client 后台线程所有权与进程退出预算。                                              |
| `client_actions`                              | `values`       | 绑定、菜单、能力策略和发现共用的 Client、菜单与前缀动作定义。                                       |
| `client_context`                              | `values`       | 只含身份的 Client 导航位置。                                                                        |
| `client_view`                                 | `state`        | Client 本地的界面框架、前缀模式、浮层和呈现偏好。绑定解析移到 `input`。                             |
| `exchange_values`                             | `values`       | 状态所有者构造和结算的 Client Connection 交换请求值与结果值；不含传输句柄或时钟。                   |
| `host_appearance`                             | `values`       | 默认颜色探测观测到的外层终端外观事实。                                                              |
| `host_capabilities`                           | `values`       | 可选宿主副作用的名称及其编译期平台准入，包括登录 Shell 模式和本地启动恢复检查点。                   |
| `host_graphics`                               | `values`       | 宿主窗口几何以及据此测得的图形事实。                                                                |
| `local_activation`                            | `workflow`     | 本地 Session 激活，包括其锁、标记、进程派生和截止时间。                                             |
| `local_client`                                | `workflow`     | 通往同级传输层句柄的唯一入口。状态所有者通过 `exchange_values` 读取交换值。                         |
| `localization`                                | `values`       | 类型化消息身份以及 `en-US` 和 `zh-CN` 目录。从环境检测 locale 的部分移到 `composition`。            |
| `machine_agents`                              | `workflow`     | 跨机器 Agent 观测作业及其有界检查点。                                                               |
| `machine_discovery`                           | `workflow`     | 已启用的已保存机器的只读观测者。                                                                    |
| `machine_observation`                         | `workflow`     | 机器作业共用的有界、可取消观测生命周期。                                                            |
| `machine_picker`                              | `state`        | Client 本地的机器选择。                                                                             |
| `machine_picker::paint`                       | `presentation` | 捕获输入的机器选择器、其 Agent 控件和查询行的绘制。                                                 |
| `machine_picker::pointer`                     | `input`        | 应用到已绘制机器选择器目标上的释放和滚轮移动。                                                      |
| `machine_spaces`                              | `workflow`     | 通过机器观测读取的 Workspace 元数据和本地 Git 检查点。                                              |
| `machine_ssh`                                 | `workflow`     | 为已保存机器构建并运行 SSH 命令的适配器。                                                           |
| `machine_tunnel`                              | `workflow`     | 已保存机器的 carrier 适配器。                                                                       |
| `machines`                                    | `state`        | 已保存机器目录的值，以及该目录唯一的存储写入者。                                                    |
| `native_settings`                             | `state`        | Client 本地的 Settings 草稿及其值行。                                                               |
| `notifications`                               | `state`        | 主 TUI 的通知状态；绘制、传输和宿主副作用由下列子模块承担。                                         |
| `notifications::paint`                        | `presentation` | 可见通知卡片的非捕获绘制。                                                                          |
| `notifications::subscription`                 | `workflow`     | Client Connection 上的通知订阅。                                                                    |
| `notifications::system`                       | `workflow`     | 平台系统通知与提示音适配器。                                                                        |
| `onboarding`                                  | `state`        | Client 本地的引导状态；只有显式关闭才写入其存储。                                                   |
| `startup_status`                              | `values`       | 本地启动结论。                                                                                      |
| `terminal_buffer_text`                        | `values`       | 遍历经过验证的保留行中的字形。                                                                      |
| `terminal_content`                            | `workflow`     | 在独立 viewer 连接上的有界内容检查。                                                                |
| `terminal_content::source`                    | `state`        | 复制模式和阅读位置据以设防的内容来源身份与屏幕见证。                                                |
| `terminal_copy`                               | `values`       | 缓冲区坐标复制策略。                                                                                |
| `terminal_copy_mode`                          | `state`        | 针对一个已准入内容表面的 Client 本地键盘复制状态。                                                  |
| `terminal_copy_query`                         | `state`        | 复制模式片段据以匹配的复制查询身份、请求和结果。                                                    |
| `terminal_copy_query::lane`                   | `workflow`     | 带关联且同一时刻只有一个在途请求的复制移动与搜索读取。                                              |
| `terminal_device`                             | `workflow`     | 继承终端设备适配器。准入失败移到 `values`；窗口几何位于 `host_graphics`。                           |
| `terminal_events`                             | `values`       | 宿主事件类型和确定性字节解码器。                                                                    |
| `terminal_graphics`                           | `workflow`     | 宿主图形探测。测得的事实位于 `host_graphics`。                                                      |
| `terminal_host_appearance`                    | `workflow`     | 外层终端默认颜色探测。观测到的事实位于 `host_appearance`。                                          |
| `terminal_input`                              | `input`        | 把解码后的宿主事件投递给当前捕获 owner 或终端转发。                                                 |
| `terminal_lifecycle`                          | `workflow`     | 继承终端的模式租约、回滚和恢复。                                                                    |
| `terminal_link_activation`                    | `workflow`     | TUI 与 CLI 共用的单次副作用链接激活。                                                               |
| `terminal_links`                              | `values`       | 基于经过验证的字形的纯文本链接策略。                                                                |
| `terminal_live_view`                          | `presentation` | 为显示而对实时网格做的 Client 本地裁剪。                                                            |
| `terminal_pointer_admission`                  | `workflow`     | any-motion 指针报告的一次性准入探测。                                                               |
| `terminal_projection`                         | `state`        | 从 Server 与 Worker 流派生的单个附着的已验证文本投影。                                              |
| `terminal_scrollbar`                          | `values`       | 经过验证的滚动条观测和整数单元格几何。                                                              |
| `terminal_text`                               | `values`       | 不可信文本的终端安全投影，以及共享的散文换行规则。                                                  |
| `terminal_title`                              | `values`       | 标题活动前缀处理，以及界面为控制台默认标题和启动程序显示的程序名称。                                |
| `termination_signal`                          | `workflow`     | 供退出生命周期使用的宿主终止观测。                                                                  |
| `transient_notice`                            | `state`        | 一个有界且会合并的提示生命周期。                                                                    |
| `tui_capabilities`                            | `values`       | 把宿主能力映射到可用动作的策略。                                                                    |
| `tui_composition`                             | `values`       | 无状态地选出最上层捕获表面，供绘制结算和输入分发共用。                                              |
| `tui_graphics`                                | `workflow`     | 与 backend 共用帧和恢复租约的 Windows 图形表面。                                                    |
| `tui_layout`                                  | `presentation` | shell 与窗格的布局几何。                                                                            |
| `tui_pointer`                                 | `input`        | 基于已发布命中图的指针归属、手势生命周期和滚轮规范化。                                              |
| `tui_pointer::hit_map`                        | `presentation` | 帧命中图和交互投影。区域携带所有者绘制时的见证，所以命中图位于 `state` 之上。                       |
| `tui_presentation`                            | `values`       | 表面实例的呈现生命周期分类。                                                                        |
| `tui_render`                                  | `presentation` | 帧组合与绘制。基础控件和重复组合作为共享组件层保留；业务表面移到各自功能。                          |
| `tui_render::controls::control_state`         | `values`       | 共享组件的控件状态词汇。                                                                            |
| `tui_shortcut_help`                           | `presentation` | 从绑定和活动模式派生的有效快捷键帮助。                                                              |
| `tui_theme`                                   | `values`       | 语义主题角色和内置主题。                                                                            |
| `windows_console_input`                       | `workflow`     | Windows 控制台记录输入适配器。                                                                      |
| `windows_signal`                              | `workflow`     | Windows 控制台中断分发器。                                                                          |
| `workbench_client`                            | `composition`  | 连接任务生命周期与 UI 组合。其子模块在下方各自标明层。                                              |
| `workbench_client::agent_attention`           | `workflow`     | Agent 注意力观测 lane 与绘制确认。                                                                  |
| `workbench_client::agent_board`               | `state`        | 来自最近一次完成读取的 Agent 看板行，以及看板的选择和排队意图。                                     |
| `workbench_client::agent_board::start::jobs`  | `workflow`     | Client PATH 目录查询和 Agent 启动作业，以及它们的平台 PATH 适配器；作业把结果结算回看板的启动状态。 |
| `workbench_client::agent_notifications`       | `values`       | 逻辑时间通知策略。                                                                                  |
| `workbench_client::agent_read_lane`           | `workflow`     | 共享 carrier 上的有界 Agent 与 Panel 读取作业。                                                     |
| `workbench_client::client_control`            | `workflow`     | 由 UI owner 推进步骤的远程导航 lane。                                                               |
| `workbench_client::command_notice`            | `state`        | 有界的 Workbench 命令提示，以及指明其对象的上下文。                                                 |
| `workbench_client::completion_reminders`      | `state`        | 每个命令事实只消费一次的提醒状态。                                                                  |
| `workbench_client::copy_episode`              | `state`        | 复制模式片段：所在窗格、内容围栏和待决查询。查询驱动由 UI 组合层持有。                              |
| `workbench_client::configuration`             | `workflow`     | 配置发布监视器。                                                                                    |
| `workbench_client::connection`                | `workflow`     | Client 连接任务。                                                                                   |
| `workbench_client::independent_popup`         | `workflow`     | 独立的投影与控制 lane。                                                                             |
| `workbench_client::input`                     | `input`        | 分发器消费的输入端口契约。                                                                          |
| `workbench_client::mailbox`                   | `state`        | 连接生命周期的队列、关联，以及与 Client 状态一起进行的原子准入。                                    |
| `workbench_client::normal_exit_hold`          | `state`        | Shell 以退出码 0 结束的窗格的关闭呈现，保持到窗格离开成员关系或超过上限。                           |
| `workbench_client::pane_graphics`             | `workflow`     | 窗格画布读取 owner。                                                                                |
| `workbench_client::panel_reports`             | `workflow`     | 已选 Presentation 的观测 lane。                                                                     |
| `workbench_client::plugin_overlays`           | `workflow`     | 插件浮层读取与关闭作业。                                                                            |
| `workbench_client::primary_directory`         | `values`       | 已准入的主根进程目录样本。                                                                          |
| `workbench_client::settings_lane`             | `workflow`     | Settings 快照观测与写入提交。                                                                       |
| `workbench_client::startup_directory`         | `state`        | 空 Session 启动意图和保留的调用证据。                                                               |
| `workbench_client::startup_preference`        | `state`        | Client 本地的最近显示位置。                                                                         |
| `workbench_client::startup_preference::store` | `workflow`     | 平台位置缓存的存储标识、加载和唯一的后台写入者。                                                    |
| `workbench_client::state`                     | `state`        | 针对经过验证的规范投影的附着、成员关系、选择和待处理操作。                                          |
| `workbench_client::status_recovery`           | `workflow`     | 连接任务退出后的保留关闭观测任务。                                                                  |
| `workbench_client::terminal_clipboard`        | `workflow`     | 剪贴板副作用适配器。                                                                                |
| `workbench_client::terminal_effects`          | `workflow`     | 有界外部副作用队列。                                                                                |
| `workbench_client::terminal_selection`        | `state`        | 附着本地的选择命令和已准入文本。                                                                    |
| `workbench_client::terminal_tail`             | `presentation` | 基于保留行的实时尾部显示决策。                                                                      |
| `workbench_client::terminal_viewport`         | `state`        | 每个窗格的保留历史视口状态。                                                                        |
| `workbench_client::ui`                        | `composition`  | `WorkbenchClientUi`：功能组合、结算和输入端口实现。其功能状态移到状态所有者。                       |
| `workbench_client::ui::history_positions`     | `state`        | 跨 viewer 与上下文替换保留、经内容校验的阅读选择。                                                  |
| `workbench_client::ui::retained_frames`       | `state`        | 所选面板最近绘制的帧，跨上下文切换有界保留。                                                        |
| `workbench_client::wakeup`                    | `state`        | 连接生命周期的唤醒合并。                                                                            |
| `workbench_client::window_title`              | `workflow`     | 窗口标题订阅消费者。                                                                                |
| `workbench_client::workbench_projection`      | `state`        | 把经过验证的规范快照投影为 Client 上下文、Tab 和 Panel。                                            |
| `workbench_client::worktree_lane`             | `workflow`     | 有界 Worktree 命令 lane。                                                                           |
| `worktree_destination`                        | `values`       | 已配置的 checkout 目标位置策略。                                                                    |

## 维护

添加、删除、重命名或移动模块时，在同一变更中更新它的行。检查会报告未归属的顶层模块和指向不存在
模块的行。运行 `pnpm lint:rust-modules -- --details --member apps/orchbee/crates/workbench-client`
可列出每个环和每个指向高层的引用及其源码位置样本；[Lint](../../../../../docs/zh-CN/LINTING.md)
拥有该命令。

<!-- documentation-pairing: english-sha256=d7d9fe053ebeb738597613b8785020cee039fe397bf5c962af559c8dc51166e1 zh-cn-sha256=caeee90c54c658fcbb1a4b1734bc8e7e82cc0784875c795fe6ea1e964a654dc5 -->
