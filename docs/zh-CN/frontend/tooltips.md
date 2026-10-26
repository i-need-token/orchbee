# 前端 Tooltip

## 归属与 Provider

每个可运行前端都要在产品界面上方安装一个来自 `@orchbee/ui` 的 `TooltipProvider`。共享 Provider 负责 hover 时序和相邻 Tooltip 行为；功能组件组合 `Tooltip`、`TooltipTrigger` 与 `TooltipContent`，但不再安装另一个 Provider。

Provider 的默认打开延迟为 `300ms`。不要在各个 Trigger 上重复这个值。不同延迟必须有已记录的交互要求和聚焦测试；单纯的视觉偏好不构成例外。未继承应用 Decorator 的独立 Story 和测试要安装自己的 Provider。

## 内容契约

- Tooltip 用于补充控件的简短动作名称或说明。必要指令、状态、校验或错误恢复信息即使没有 hover 也必须可见。
- 每个纯图标控件都要独立拥有可访问名称。Tooltip 内容不能替代 `aria-label`、可见文本或其他可访问命名关系。
- 产品编写的 Tooltip 和可访问性文本通过 `@orchbee/i18n` 本地化。通用 UI Primitive 接收消费者提供的文本，绝不自行选择产品文案。
- 当动作存在键盘快捷键时，使用 `Kbd` 或 `KbdGroup`，在本地化动作名称后展示按平台格式化的快捷键。不要把平台专属符号放进翻译消息。
- Tooltip 文本要简短并以动作为导向。不要在 Tooltip 中放置交互控件、段落或产品工作流。
- 共享 Tooltip 表面不使用方向箭头。通过位置和间距建立 Tooltip 与 Trigger 的关联；使用方不要局部添加箭头。

## 交互契约

- 指针 hover 使用共享的 `300ms` 延迟，避免无意的光标移动制造视觉噪音。
- 键盘聚焦必须展示等价信息，并且不能要求移动指针。Escape 和焦点离开按照 UI Primitive 的可访问性行为关闭 Tooltip。
- 触摸用户不能依赖 hover。Tooltip 绝不承载发现或完成动作所必需的信息。
- Trigger 的命中区域、禁用行为和焦点提示仍由底层控件负责。不要添加会改变其交互几何的、仅用于 Tooltip 的隐形覆盖层。
- Tooltip 的位置不能遮挡 Trigger 或动作的直接目标。优先使用 Primitive 的碰撞处理，不要使用视口专属坐标。

## 验证

- 单元覆盖将 `TooltipProvider` 默认值固定为 `300ms`。
- 代表性的浏览器交互验证确认：延迟结束前 Tooltip 保持关闭，结束后打开。
- 键盘审查能够到达 Trigger、展示等价内容，并且在不困住焦点的情况下关闭。
- 本地化审查覆盖每个已发布 Locale，并让快捷键符号格式化留在消息目录之外。
- 浏览器审查检查 Tooltip 在消费界面中的位置、裁剪和对比度。

<!-- documentation-pairing: english-sha256=9c5476284a01630405264a8cc7afef9c6e0bd5af31cfb10924a13f2a2ff4a7f3 zh-cn-sha256=d426d23584dbd2ff67b59eeb908861d7ce09a89e64a845874484a9d327a43c4b -->
