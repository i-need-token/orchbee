import type { DesktopTrayMessageId } from '../en-US';

export const zhCNDesktopTrayMessages = {
	'desktop.tray.notice.body':
		'{terminals, plural, other {# 个终端继续工作。}}点击右下角的 Orchbee 图标重新打开，右键可全部退出。',
	'desktop.tray.notice.title': '{application} 仍在后台运行',
	'desktop.tray.quitAll': '全部退出…',
	'desktop.tray.quitAll.cancel': '取消',
	'desktop.tray.quitAll.confirm': '全部停止并退出',
	'desktop.tray.quitAll.detail':
		'{terminals, plural, other {# 个终端将停止}}{agents, plural, =0 {。} other {，其中 # 个智能体正在工作。}}布局和工作目录会保存；屏幕内容和智能体进行中的工作不会保存。连接到此会话的终端窗口中的 Orchbee 也会关闭。',
	'desktop.tray.quitAll.failure.busy': '此会话的另一次停止已在进行中。',
	'desktop.tray.quitAll.failure.close': '关闭',
	'desktop.tray.quitAll.failure.notSent': 'Orchbee 无法连接到会话，没有停止任何内容。',
	'desktop.tray.quitAll.failure.refused': '会话拒绝了停止请求，没有停止任何内容。',
	'desktop.tray.quitAll.failure.stopFailed':
		'会话已开始停止但未能完成，部分终端可能仍在运行。可以用 server stop --retry 和请求 ID {stopId} 重试。',
	'desktop.tray.quitAll.failure.title': 'Orchbee 未能停止全部终端',
	'desktop.tray.quitAll.failure.unconfirmed':
		'Orchbee 无法确认会话已停止，部分终端可能仍在运行。可以用 server stop-status 和请求 ID {stopId} 查看。',
	'desktop.tray.quitAll.failure.unmanaged': '此会话无法从 Orchbee 停止。',
	'desktop.tray.quitAll.stopping': '正在停止全部终端…',
	'desktop.tray.quitAll.title': '停止全部终端并退出 Orchbee？',
	'desktop.tray.quitKeepRunning': '退出 Orchbee，终端继续运行',
	'desktop.tray.reopen': '重新打开 Orchbee',
	'desktop.tray.status':
		'{terminals, plural, other {# 个终端正在运行}}{failed, plural, =0 {} other { · # 个失败}}{attention, plural, =0 {} other { · # 个需要处理}}{done, plural, =0 {} other { · # 个已完成}}',
	'desktop.tray.tooltip': '{application} · {status}',
} as const satisfies Readonly<Record<DesktopTrayMessageId, string>>;
