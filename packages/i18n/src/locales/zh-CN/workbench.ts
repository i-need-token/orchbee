import type { WorkbenchMessageId } from '../en-US';

export const zhCNWorkbenchMessages = {
	'workbench.emptySessions.description': '创建一个内存会话以开始使用。',
	'workbench.emptySessions.title': '暂无打开的会话',
	'workbench.navigation.label': 'Workbench 导航',
	'workbench.runtime.local.label': '本地运行时',
	'workbench.runtime.sessionCount': '{sessionCount, number} 个会话',
	'workbench.session.status.disconnected': '已断开连接',
	'workbench.session.status.exited': '已退出',
	'workbench.session.status.running': '运行中',
	'workbench.session.status.starting': '启动中',
	'workbench.sessions.heading': '会话',
	'workbench.terminal.actions.closeSplit': '关闭分屏',
	'workbench.terminal.actions.closeTerminal': '关闭终端',
	'workbench.terminal.actions.label': '终端操作',
	'workbench.terminal.actions.newTerminal': '新建终端',
	'workbench.terminal.actions.splitDown': '向下分屏',
	'workbench.terminal.actions.splitRight': '向右分屏',
	'workbench.terminal.pane.label': '{sessionTitle} 终端',
	'workbench.terminal.start.failed': '终端无法启动。',
	'workbench.terminal.start.pending': '正在启动终端…',
} as const satisfies Readonly<Record<WorkbenchMessageId, string>>;
