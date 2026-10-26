# Desktop 窗口生命周期

## 问题和范围

本地 Desktop 客户端拥有一个 Electron 窗口；此前它以固定尺寸启动，并依赖附带的 OS 控件。现在 Desktop 负责用户期望的最小持久窗口生命周期：最大化/恢复、原生全屏切换、安全恢复普通窗口边界，以及菜单选择与菜单快捷键共用的单一语义 Action 路径。

本合约仅限当前单一本地Desktop窗口。多窗口身份、在窗口之间移动选项卡、快速终端、始终位于顶部、本机标题栏复制以及 renderer 或 Host 对窗口状态的权威（authority）仍超出范围。

## 所有权和边界

Electron Main 是本机窗口状态和效果的唯一所有者。渲染器、Workbench 共享文档、Terminal Host、终端绑定和投影附件既不持久也不决定窗口几何形状。窗口状态文件是 Electron 的产品 `userData` 目录下的可恢复设备本地首选项；它不是终端恢复或共享产品状态。

严格的版本 1 记录是：

```ts
interface DesktopWindowStateV1 {
	readonly version: 1;
	readonly bounds: {
		readonly x: number;
		readonly y: number;
		readonly width: number;
		readonly height: number;
	};
	readonly maximized: boolean;
}
```

坐标与尺寸均为有界整数。输入缺失、畸形、不可读或版本过时时，启动尺寸回退为 `1024 × 720`。有效记录会投影到当前 Electron 显示拓扑：选择交集最大的工作区；若无交集则使用主显示器工作区；缩小过大尺寸，并限制两个坐标轴，确保窗口完整可见。该投影也能处理负显示坐标与已移除显示器。

仅 `BrowserWindow.getNormalBounds()` 进入记录。最大化记录可恢复的正常范围加上`maximized: true`。全屏是暂时的，并且在进程启动时永远不会恢复。几何和最大化事件按观察顺序将原子文件替换排入队列。窗口关闭、同进程重新打开和应用程序关闭在同一队列后面等待；写入失败是可恢复的首选项失败，并且永远不会阻止终端或应用程序生命周期行为。

## Actions 和平台投影

Main 拥有的 Action 运行时注册了两个稳定的、不可自动化的意图：

| Action ID                           | 效果                                       | 菜单加速器                        |
| ----------------------------------- | ------------------------------------------ | --------------------------------- |
| `desktop.window.toggle-maximized`   | 最大化正常窗口或恢复最大化窗口；全屏时禁用 | `CmdOrCtrl+Shift+M`               |
| `desktop.window.toggle-full-screen` | 请求进入或退出 Electron 原生全屏           | macOS `Ctrl+Cmd+F`;其他平台 `F11` |

应用菜单及其快捷键调用同一条已准备 Action 路径。原生“交通灯”按钮与窗口管理器控件会把 OS 输入映射到同一 BrowserWindow 状态；它们发出普通 Electron maximize、unmaximize 与 fullscreen 事件供持久化逻辑消费。

聚焦现有的最小化窗口首先会恢复并显示它，然后聚焦它。现有的单实例、关闭确认、渲染器文档停用、共享 Workbench 和 Terminal Host 生命周期保持不变。

## 状态模型和不变量

持久状态为 `normal bounds + maximized`。运行时观察还区分普通、最大化、全屏与不可用窗口状态。输入包括 load、几何完成、maximize、unmaximize、进入/退出全屏、close、reopen 与应用关闭。

所有者保留这些不变量：

- 没有包含全屏状态的持久记录；
- 每个持久的矩形都源自正常边界，而不是最大化或全屏框架；
- 每个恢复的矩形完全包含在一个当前显示工作区域中；
- 排队写入保留捕获顺序，完成的刷新会观察最后一次成功的捕获；
- 不可用或与全屏不兼容的 Action 不会产生隐藏效果；
- 窗口状态和 Action 永远不会改变 Workbench 文档、终端绑定、Host 会话或投影附件。

纯状态测试覆盖有序边界捕获、排除临时全屏状态、显示器选择与钳制边界。文件测试覆盖缺失/有效/无效/不可读状态、原子替换、序列化写入及失败。组合测试覆盖聚焦前恢复、退出刷写和原有关闭门控。原生最大化/全屏/重启/屏外修正及附件身份由明确授权后才运行的 manual-only 原生窗口系统测试覆盖；常规验证排除该测试。不声明单独的 TLA 流程证明。

## 证据限制

Electron 和操作系统在显示枚举、`getNormalBounds()`、最大化事件和本机全屏转换方面仍然受信任。 macOS 本机行为在当前开发运行程序上执行。 Linux 和 Windows 被确定性模型和适配器测试覆盖，直到它们的本机运行程序执行相同的系统跟踪；即使 Orchbee 始终提供安全请求的矩形，Wayland 合成器也可能会限制绝对定位。

<!-- documentation-pairing: english-sha256=14882568810444148b55dba19af03bacd7e629e3f56044cfa8bd2e23164b1611 zh-cn-sha256=dbd21314357aad7c0360b3c78087a131636f84e589d03251f1f7f976fc20005b -->
