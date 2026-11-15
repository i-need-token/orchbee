import type { WebsiteTerminalMessageId } from '../en-US';

export const zhCNWebsiteTerminalMessages = {
	'website.terminal.metaTitle': '终端界面 — Orchbee',
	'website.terminal.metaDescription':
		'运行在你已有的终端里，带侧边栏、标签页与分屏面板。它与桌面应用呈现的是同一个会话。',
	'website.terminal.lead':
		'运行在你已有的终端里，带侧边栏、标签页与分屏面板。它与桌面应用呈现的是同一个会话。',
	'website.terminal.note': '然后运行 <code>orchbee</code>。',
} as const satisfies Readonly<Record<WebsiteTerminalMessageId, string>>;
