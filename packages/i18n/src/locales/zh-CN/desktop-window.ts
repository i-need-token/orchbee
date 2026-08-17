import type { DesktopWindowMessageId } from '../en-US';

export const zhCNDesktopWindowMessages = {
	'desktop.window.action.toggleFullScreen': '进入 / 退出全屏',
	'desktop.window.action.toggleMaximized': '最大化 / 还原窗口',
	'desktop.window.actionUnavailable': '当前没有可用的桌面窗口。',
	'desktop.window.category': '窗口',
	'desktop.window.maximizedUnavailableInFullScreen': '请先退出全屏，再更改窗口的最大化状态。',
} as const satisfies Readonly<Record<DesktopWindowMessageId, string>>;
