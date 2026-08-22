# 键盘快捷键

`@orchbee/web` 拥有八个常见语义快捷方式的客户端本地覆盖。 Workbench 和投影终端功能继续拥有其 Action 输入和平台默认值。 Web 组合物组合了这些声明，验证一个完整的有效表，并将相同的源注入到安装的 Workbench 和投影终端中。

## 初始目录

| 快捷方式 ID                     | 适用范围               | 苹果默认      | 标准默认值     |
| ------------------------------- | ---------------------- | ------------- | -------------- |
| `workbench.search.actions.open` | Workbench              | `Cmd+Shift+P` | `Ctrl+Shift+P` |
| `workbench.terminal.create`     | Workbench              | `Cmd+N`       | `Ctrl+Shift+N` |
| `workbench.search.all.open`     | Workbench              | `Cmd+K`       | `Ctrl+Shift+K` |
| `workbench.settings.open`       | Workbench              | `Cmd+,`       | `Ctrl+,`       |
| `workbench.sidebar.toggle`      | Workbench              | `Cmd+B`       | `Ctrl+Shift+B` |
| `terminal.search.open`          | 聚焦终端或查找         | `Cmd+F`       | `Ctrl+Shift+F` |
| `terminal.selection.copy`       | 具有选择功能的聚焦终端 | `Cmd+C`       | `Ctrl+Shift+C` |
| `terminal.selection.select-all` | 聚焦终端               | `Cmd+A`       | `Ctrl+Shift+A` |

标准默认值故意保留未移位的 `Ctrl+B/N/K/F/A` 和 `Ctrl+C` 可用于 shell、终端应用程序和多路复用器。用户可以故意录制另一个支持的自定义和弦。即使 Action 暂时不可用，产品冲突仍然被禁止。

## 和弦与冲突模型

`@orchbee/keyboard-shortcuts` 只提供纯物理和弦机制。和弦包含一个受支持的 `KeyboardEvent.code` 和标准化的精确修饰符集。 `Mod` 映射到 Apple 上的 Command 和标准平台上的 Control。匹配拒绝额外的修饰符、组合（包括旧键代码 229）、重复、不支持的代码和仅修饰符事件。格式和冲突身份来自相同的标准化和弦。

所有八个初始声明都可以重叠：Workbench 快捷方式及其焦点终端的快捷方式都可以适用。因此，他们有效的和弦必须是全球独一无二的。冲突编辑既不更改内存也不更改存储，并标识冲突的 Action、范围、绑定源和可用性条件。在调度期间，拥有的 Action 适配器在执行或 `preventDefault()` 之前解析当前可用性；不可用的绑定会传递到终端或浏览器。

## 持久化与恢复

覆盖使用 `orchbee:web:keyboard-shortcuts:v1` 并严格记录：

```json
{
	"version": 1,
	"overrides": [
		{
			"shortcutId": "workbench.search.all.open",
			"chord": { "code": "KeyG", "modifiers": ["mod", "shift"] }
		}
	]
}
```

仅存储自定义值，按快捷方式 ID 排序。未知字段或 IDs、重复项、未排序的条目、非规范修饰符、不支持的代码、存储的默认值、过多的条目以及有效冲突会使完整记录无效并恢复平台默认值。写入失败会使不可变会话绑定保持活动状态，报告它未保存，并重试下一个合法更改。恢复一项会删除其覆盖。

Browser Preview、Gateway Browser 和 Electron 各自针对其自己的原始本地存储创建和处置一个控制器。 Gateway和Electron通过`ProjectedTerminalSurfaceCapability`传递控制器的窄结合源。事件处理程序在调度时读取当前表，因此更改不会重新安装 Workbench、替换附件或重新打开 Host 或 Gateway 连接。

## 维护检查

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
