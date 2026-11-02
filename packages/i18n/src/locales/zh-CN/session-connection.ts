import type { SessionConnectionMessageId } from '../en-US';

export const zhCNSessionConnectionMessages = {
	'session.connection.disconnected': '已断开',
	'session.connection.reconnect': '重新连接',
	'session.connection.reconnectDescription':
		'重新连接此窗口；如 Orchbee 会话已停止，会先重新启动它。',
	'session.connection.unavailable.description': '此窗口无法连接到 Orchbee 会话。',
	'session.connection.unavailable.recoveryRequired':
		'该会话需要恢复后才能启动。如需检查，请在终端中运行 orchbee server diagnose。',
	'session.connection.unavailable.title': 'Orchbee 会话不可用',
	'session.connection.unavailable.tryAgain': '重试',
} as const satisfies Readonly<Record<SessionConnectionMessageId, string>>;
