# 键盘快捷键

`@orchbee/web` 拥有一组精选常见语义快捷键的客户端本地覆盖。Workbench 和投影终端功能继续拥有各自的 Action 输入与平台默认值。Web 组合层汇总这些声明，验证完整的有效绑定表，并把同一个来源注入已挂载的 Workbench 与投影终端。菜单、工具提示、Action 搜索界面和快捷键设置都从该有效绑定表投影标签；这些界面都不拥有另一份绑定副本。

## 初始目录

| 快捷方式 ID                                  | 适用范围               | 苹果默认          | 标准默认值         |
| -------------------------------------------- | ---------------------- | ----------------- | ------------------ |
| `workbench.search.actions.open`              | Workbench              | `Cmd+Shift+P`     | `Ctrl+Shift+P`     |
| `workbench.terminal.create`                  | Workbench              | `Cmd+T`           | `Ctrl+Shift+T`     |
| `workbench.terminal.active.close`            | 活动终端               | `Cmd+W`           | `Ctrl+Shift+W`     |
| `workbench.panel.active.maximization.toggle` | 活动面板               | `Cmd+Shift+Enter` | `Ctrl+Shift+Enter` |
| `workbench.panel.active.split-right`         | 活动面板               | `Cmd+D`           | `Ctrl+Shift+D`     |
| `workbench.panel.active.split-below`         | 活动面板               | `Cmd+Shift+D`     | `Ctrl+Shift+E`     |
| `workbench.search.all.open`                  | Workbench              | `Cmd+K`           | `Ctrl+Shift+K`     |
| `workbench.settings.open`                    | Workbench              | `Cmd+,`           | `Ctrl+,`           |
| `workbench.sidebar.toggle`                   | Workbench              | `Cmd+B`           | `Ctrl+Shift+B`     |
| `terminal.search.open`                       | 聚焦终端或查找         | `Cmd+F`           | `Ctrl+Shift+F`     |
| `terminal.selection.copy`                    | 具有选择功能的聚焦终端 | `Cmd+C`           | `Ctrl+Shift+C`     |
| `terminal.clipboard.paste`                   | 聚焦终端               | `Cmd+V`           | `Ctrl+Shift+V`     |
| `terminal.selection.select-all`              | 聚焦终端               | `Cmd+A`           | `Ctrl+Shift+A`     |

新建标签页、关闭活动终端、活动面板最大化、向右分屏、向下分屏和粘贴的默认键采用 2026-08-22 观察到的 Warp [桌面端文档约定](https://docs.warp.dev/getting-started/keyboard-shortcuts)；这里只固定选择默认按键，不主张更广泛的行为或视觉一致性。标准平台默认值特意保留未加 Shift 的 `Ctrl+B/T/W/K/F/D/E/V/A` 与 `Ctrl+C`，供 shell、终端应用和多路复用器使用。用户可以主动录入其他受支持的自定义组合键。即使某个 Action 暂时不可用，产品内冲突仍然不允许出现。

没有有效绑定的命令不渲染空的快捷键提示。“清除”“重置”“滚动到底部”“新建子终端”“向左分屏”和“向上分屏”在本目录中仍然有意保持未绑定。允许用户为初始未绑定命令创建可空绑定不在本模型范围内。

## 活动目标命令模型

四个 `workbench.*.active.*` Action 是上下文命令。菜单发起的 Action 调用可以携带显式面板目标；否则使用活动标签页中聚焦的面板。执行开始时会重新解析目标，因此陈旧或缺失的目标不可用，也不会吞掉键盘事件。

`workbench.terminal.active.close` 精确解析为一个下层关闭操作：

| 活动目标状态                                     | 准备的操作                     |
| ------------------------------------------------ | ------------------------------ |
| 当前显示的是子终端                               | 关闭该子终端                   |
| 当前显示的是主终端，且活动标签页有两个或更多面板 | 关闭该面板                     |
| 当前显示的是主终端，且活动标签页只有一个面板     | 关闭活动标签页                 |
| 不存在有效活动标签页或目标面板                   | 不可用；保留状态与事件默认行为 |

上下文 Action 会把已经注册的面板或标签页关闭 Action 准备成调用并执行。下层所有者继续负责可用性、运行中进程确认、取消、串行化与权威提交。上下文 Action 只报告下层执行结果，绝不直接修改面板或标签页状态。取消确认后，拓扑与终端会话保持不变。取消上下文执行会取消其准备好的下层执行；如果下层操作已经提交，则其完成结果仍是权威结果。下层注册缺失或陈旧时，上下文 Action 会被阻止，且不会通过直接修改来兜底。

最大化只在活动标签页至少有两个面板时可解析。向右和向下分屏 Action 在存在有效目标面板时可解析。每个命令都准备并执行现有面板 Action，因此菜单、键盘分发和 Action 搜索界面共享同一个状态转换与可用性结果。不可用命令既不会执行，也不会触发 `preventDefault()`。

## 和弦与冲突模型

`@orchbee/keyboard-shortcuts` 只提供纯物理和弦机制。和弦包含一个受支持的 `KeyboardEvent.code` 和标准化的精确修饰符集。 `Mod` 映射到 Apple 上的 Command 和标准平台上的 Control。匹配拒绝额外的修饰符、组合（包括旧键代码 229）、重复、不支持的代码和仅修饰符事件。格式和冲突身份来自相同的标准化和弦。

目录中的所有声明都可能重叠：Workbench 快捷键及其聚焦终端的快捷键可能同时适用。因此，其有效组合键必须在产品内全局唯一。发生冲突的编辑既不改变内存也不改变存储，并会标识冲突的 Action、范围、绑定来源和可用性条件。分发期间，所属 Action 适配器会在执行或调用 `preventDefault()` 前解析当前可用性；不可用绑定会继续传递给终端或浏览器。

Workbench 负责格式化自身绑定。投影终端负责其绑定默认值与分发，而 Web 只通过只读 Workbench 端口投影当前的 Action 到标签映射。这样可保持依赖方向：Workbench 永远不导入应用拥有的快捷键目录。命令菜单项保存可选的有效标签，两个共享菜单渲染器都使用 UI primitive 的尾部快捷键槽位。Action 搜索接收同一个投影器，其中也包括从后代作用域发现的终端 Action。

## 持久化与恢复

新写入使用 `orchbee:web:keyboard-shortcuts:v2` 严格记录：

```json
{
	"version": 2,
	"overrides": [
		{
			"shortcutId": "workbench.search.all.open",
			"chord": { "code": "KeyG", "modifiers": ["mod", "shift"] }
		}
	]
}
```

只存储自定义值，并按快捷键 ID 排序。未知字段或 ID、重复项、未排序条目、非规范修饰符、不支持的代码、已存储默认值、过多条目以及有效冲突都会使完整 v2 记录无效，并恢复平台默认值。写入失败时，不可变的会话绑定仍保持活动，同时报告尚未保存，并在下一次合法修改时重试。恢复单项会移除其覆盖。

当 v2 不存在时，Web 使用旧目录和旧默认值读取 `orchbee:web:keyboard-shortcuts:v1` 记录，把其中显式覆盖带入扩展后的目录，移除与新默认值等价的覆盖，拒绝迁移后产生的冲突，并写入规范 v2 记录。v1 值原样保留，作为回滚快照。一旦存在有效 v2 记录，它就是权威来源；无效 v2 绝不回退到 v1，因为回退会掩盖当前数据损坏。回滚到只支持 v1 的版本时，会恢复升级前的快捷键快照；只在 v2 中做出的编辑有意不对旧版本可用。

Browser Preview、Gateway Browser 和 Electron 各自针对自己的源本地存储创建并销毁一个控制器。Gateway 与 Electron 通过 `ProjectedTerminalSurfaceCapability` 传递控制器的窄绑定来源。事件处理器在分发时读取当前表，因此修改不会重新挂载 Workbench、替换附件，也不会重新打开 Host 或 Gateway 连接。

## 声明与证据

| 声明                                                   | 确定性证据                                               |
| ------------------------------------------------------ | -------------------------------------------------------- |
| 一个有效目标只产生一个子终端、面板或标签页关闭操作     | 活动目标解析器决策表测试                                 |
| 确认、取消与提交继续由准备好的下层 Action 所有         | 同时注册面板与标签页 Action 的 Workbench Action 集成测试 |
| Apple 与标准平台默认值符合目录，且自定义编辑会影响分发 | 快捷键定义、解析器与控制器测试                           |
| 菜单、Action 搜索和设置显示同一个当前绑定              | 投影与渲染界面测试                                       |
| 有效 v1 记录只迁移一次且不会丢失显式旧覆盖             | 存储迁移测试，包括与新默认值等价的覆盖                   |
| 无效 v2 数据恢复默认值且不读取 v1                      | 存储损坏与优先级测试                                     |

测试使用合成的平台标志、Action 作用域、存储与键盘事件。它们不主张操作系统全局快捷键行为或完整 Warp 兼容性。手动 Desktop 原生窗口焦点测试不包含在自动验证中。

## 维护检查

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
