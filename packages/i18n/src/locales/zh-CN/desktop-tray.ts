import type { DesktopTrayMessageId } from '../en-US';

export const zhCNDesktopTrayMessages = {
	'desktop.tray.notice.body':
		'{terminals, plural, other {# 个终端继续工作。}}点击右下角的 Orchbee 图标重新打开，右键可停止会话。',
	'desktop.tray.notice.menuBarBody':
		'{terminals, plural, other {# 个终端继续工作。}}点按程序坞中的 Orchbee 重新打开，点按菜单栏中的 Orchbee 图标可停止会话。',
	'desktop.tray.notice.title': '{application} 仍在后台运行',
	'desktop.tray.open': '打开 Orchbee',
	'desktop.tray.quitAll': '停止会话…',
	'desktop.tray.quitAll.cancel': '取消',
	'desktop.tray.quitAll.confirm': '停止并退出',
	'desktop.tray.quitAll.detail':
		'{terminals, plural, other {# 个终端将停止}}{agents, plural, =0 {。} other {，其中 # 个智能体正在工作。}}布局和工作目录会保存；屏幕内容和智能体进行中的工作不会保存。连接到此会话的终端窗口中的 Orchbee 也会关闭。',
	'desktop.tray.quitAll.failure.busy': '此会话的另一次停止已在进行中。',
	'desktop.tray.quitAll.failure.close': '关闭',
	'desktop.tray.quitAll.failure.notSent': '没能连上会话，没有停止任何内容。',
	'desktop.tray.quitAll.failure.refused': '会话拒绝了停止请求，没有停止任何内容。',
	'desktop.tray.quitAll.failure.stopFailed': '会话已开始停止但没能完成，部分终端可能仍在运行。',
	'desktop.tray.quitAll.failure.title': '没能停止会话',
	'desktop.tray.quitAll.failure.unconfirmed': '不确定是否完成，请先看一下再重试。',
	'desktop.tray.quitAll.failure.unmanaged': '此会话无法从 Orchbee 停止。',
	'desktop.tray.quitAll.stopping': '正在停止会话…',
	'desktop.tray.quitAll.title': '停止会话并退出 Orchbee？',
	'desktop.tray.quitKeepRunning': '退出 Orchbee',
	'desktop.tray.status':
		'{terminals, plural, =0 {没有终端在运行} other {# 个终端正在运行}}{failed, plural, =0 {} other { · # 个智能体失败}}{attention, plural, =0 {} other { · # 个智能体需要处理}}{done, plural, =0 {} other { · # 个智能体已完成}}',
	'desktop.tray.tooltip': '{application} · {status}',
} as const satisfies Readonly<Record<DesktopTrayMessageId, string>>;
