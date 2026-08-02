import type { DesktopApplicationLifetimeMessageId } from '../en-US';

export const zhCNDesktopApplicationLifetimeMessages = {
	'desktop.applicationLifetime.actions.cancel': '取消',
	'desktop.applicationLifetime.actions.preserveAndQuit': '保留会话并退出',
	'desktop.applicationLifetime.actions.quitAnyway': '仍然退出',
	'desktop.applicationLifetime.actions.retry': '重试',
	'desktop.applicationLifetime.actions.stopAndQuit': '停止会话并退出',
	'desktop.applicationLifetime.exit.description':
		'当前有 {sessionCount, number} 个终端会话正在运行。',
	'desktop.applicationLifetime.exit.title': '退出 Orchbee？',
	'desktop.applicationLifetime.failed.description':
		'部分终端会话或进程可能仍在运行。你可以重试，或仍然退出并让未解决的进程继续运行。',
	'desktop.applicationLifetime.failed.title': '无法完成退出',
	'desktop.applicationLifetime.progress.draining': '正在检查剩余的终端会话…',
	'desktop.applicationLifetime.progress.exiting': '正在退出 Orchbee…',
	'desktop.applicationLifetime.progress.flushing': '正在保存工作区…',
	'desktop.applicationLifetime.progress.stopping':
		'已停止 {completedCount, number} 个终端会话，共 {totalCount, number} 个。',
	'desktop.applicationLifetime.progress.title': '正在准备退出…',
} as const satisfies Readonly<Record<DesktopApplicationLifetimeMessageId, string>>;
