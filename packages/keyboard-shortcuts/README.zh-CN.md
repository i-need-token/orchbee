# @orchbee/keyboard-shortcuts

私有包入口只导出仓库内有消费者的符号，Knip 的 `includeEntryExports` 检查该边界。内部契约、生成资产与独立一致性证据按其所有权保留；它们的存在不自动扩大公共入口。新增客户端应按已准入用途添加所需导出。

Orchbee 产品所有者共享的纯键盘组合键机制。

该软件包标准化单个物理 `KeyboardEvent.code` 加上精确的修饰符集，将工程修饰符 `Mod` 映射到 Apple 平台上的 Meta 和其他地方的 Control，捕获支持的事件，比较平台有效身份，匹配事件并派生平台显示文本。它不拥有产品 Action、默认绑定、上下文条件、冲突策略、持久性、UI、平台检测或事件侦听器生命周期。

产品功能在其所属的 `*-shortcuts.ts` 模块中继续定义其默认绑定。应用程序可以保留经过验证的覆盖，并使用 `keyboardShortcutChordKey(...)` 及其自己的范围重叠策略来诊断冲突。

仅支持至少包含一个非 Shift 修饰键的单步物理组合键。文本输入、无修饰键输入、仅修饰键事件、Escape、Tab、输入法组合和重复 keydown 事件均有意排除在捕获契约之外。

Apple 显示文本遵循平台菜单的 Control、Option、Shift、Command 顺序（`⌃⌥⇧⌘`）；非 Apple 显示文本使用以 `+` 分隔的修饰键名称。

```ts
import {
	captureKeyboardShortcutChord,
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
} from '@orchbee/keyboard-shortcuts';

const result = captureKeyboardShortcutChord(event, 'standard');
if (result.kind === 'captured') {
	console.log(formatKeyboardShortcutChord(result.chord, 'standard'));
}

if (matchesKeyboardShortcutChord(event, chord, 'standard')) {
	// Resolve the owning semantic Action and consume only when it is available.
}
```

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/keyboard-shortcuts build
pnpm --filter @orchbee/keyboard-shortcuts test:exports
```

<!-- documentation-pairing: english-sha256=81836d9b728a57471a52a94b436e5d9c654f1c101837bb53ac359e2501491bfc zh-cn-sha256=826ff6e2d1e3983ff2bc89046e272f158c7ec433ab31ae14276e3416101222a7 -->
