# Windows CLI 通知

`notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]` 向已有 Site 提交。标题/正文规范化会合并空白、移除控制字符，并截断为 80/240 个 Unicode 标量；规范化后标题为空则在提交前失败。省略位置时使用各 Client 配置（默认右下），声音默认 none。线协议同时限制规范字符串的标量数和 UTF-8 字节数。

## 交付模型

现有 Server Workbench 服务拥有临时通知注册表。监听者属于认证连接的 generation，五秒未轮询则过期；每个监听者最多一个待取消息，消息也限时五秒。最多 32 个监听者、256 个请求回执，回执保留 60 秒。成功 Show 向所有当前可接收的在线主 TUI 监听者排入通知，并开始一秒限流。disabled、busy、rate-limited、no-foreground-client 不投递。保留期内精确重放返回原结果而不再次排入；相同身份的不同请求失败。传输不确定后不重试变更。成功回执表示入队，不保证实际显示或声音播放。监听者只取一次；断连和过期释放待处理状态。Site 重启不保留通知。

Client 自有的可取消连接独立于输入/绘制进行轮询，只有 UI 所有者安装通知。启用卡片投递时，单个五秒临时卡片出现在内容区的指定角落，不盖住状态行，不捕获键盘/鼠标，不移动焦点，不改变布局。卡片模式下，已有提示、对话框和覆盖层会暂停接收；竞态下丢弃到达的卡片，不替换用户拥有的界面。空闲到期也触发重绘。显示文字转义终端及方向格式控制字符，按终端显示宽度裁剪。成功绘制后可请求一次安全 Windows 声音：done 使用信息声，request 使用感叹声，none 静音。系统声音策略可能抑制播放。外部投递的声音请求独立于视觉成功；system 模式请求 Windows 通知区域气泡。任何投递均不激活原生窗口，不跳转到 Agent 目标。

## 边界与证据

Herdr 参照为 `e0507237ad8d41f60aece10d00ef29ce2339ca34`，尤其是 `server/headless/notifications.rs`：Client 交付策略与 Server 回退策略不同。原生自定义通知遵循 Client 投递策略及省略位置继承。Agent 延迟、剪贴板策略、声音偏好和目标跳转仍属独立对齐工作。私有 cohort 匹配升级/回退，不新增持久化格式、依赖版本或无界队列。

定向证据覆盖文本规范化边界、独立协议校验、注册表过期/重放/扇出/背压、实际 Windows CLI 管道、关闭、Unicode 四角渲染、空闲过期及输入归属保持。不代表已认证 provider、实际 Windows 视觉/声音或最终全量回归验收。

## 自定义通知投递配置

Windows `[ui.toast] delivery` 选择 `off`（默认）、`orchbee`（`herdr` 为导入别名）、`terminal` 或 `system`。`[ui.toast.herdr] position` 用既有四个 CLI 位置名称选择默认角落。CLI 省略位置时使用各接收 Client 的配置，显式位置优先。启动使用 Client 配置；显式 Site 配置发布将有界规范通知配置与键位一起应用，无效发布同时保留旧策略。其他 Agent/剪贴板/声音偏好不属于本批自定义通知范围。

Off 关闭视觉提示，显式请求的自定义声音仍独立生效。没有 Client 时，Server 的 off 策略返回 `disabled`，否则返回 `no-foreground-client`。连接中的 Client 即使使用 off，也可以将传输接纳报告为 `shown`。策略变化丢弃未投递效果和卡片，不重放此前消息。

终端投递使用检测到的 Ghostty/iTerm/WezTerm OSC 9 或 Kitty OSC 99，必要时进行 tmux 转义。只有 TUI 生命周期所有者在完整帧之后写入，恢复开始后不再写入；未知宿主显示非致命本地化提示。系统投递拥有一个隐藏、不激活的 Windows 通知窗口/线程、有界收件箱及通知区域图标。既有 winsafe 安全 GUI 封装负责窗口创建和消息派发，不编写 unsafe 代码、不启动外部辅助进程。100 毫秒定时器检测关闭，十秒后释放图标，并在关闭窗口前删除图标；Drop 取消并 join。队列接纳不确认 OS 绘制，OS 策略可能抑制气泡。物理宿主/视觉/声音验收仍后置。
