import type { DesktopTrayMessageId } from '../en-US';

export const zhCNDesktopTrayMessages = {
	'desktop.tray.notice.body':
		'{terminals, plural, other {# 个终端继续工作。}}点击右下角的 Orchbee 图标重新打开，右键可退出。',
	'desktop.tray.notice.title': '{application} 仍在后台运行',
	'desktop.tray.quitKeepRunning': '退出 Orchbee，终端继续运行',
	'desktop.tray.reopen': '重新打开 Orchbee',
	'desktop.tray.status':
		'{terminals, plural, other {# 个终端正在运行}}{failed, plural, =0 {} other { · # 个失败}}{attention, plural, =0 {} other { · # 个需要处理}}{done, plural, =0 {} other { · # 个已完成}}',
	'desktop.tray.tooltip': '{application} · {status}',
} as const satisfies Readonly<Record<DesktopTrayMessageId, string>>;
