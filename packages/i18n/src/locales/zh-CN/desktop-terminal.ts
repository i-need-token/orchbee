import type { DesktopTerminalMessageId } from '../en-US';

export const zhCNDesktopTerminalMessages = {
	'desktop.terminal.connectionFailure': '终端连接失败。',
	'desktop.terminal.input': '终端输入',
	'desktop.terminal.screen': '终端屏幕',
} as const satisfies Readonly<Record<DesktopTerminalMessageId, string>>;
