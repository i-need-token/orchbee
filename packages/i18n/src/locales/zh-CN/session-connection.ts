import type { SessionConnectionMessageId } from '../en-US';

export const zhCNSessionConnectionMessages = {
	'session.connection.disconnected': '已断开',
	'session.connection.reconnect': '重新连接',
	'session.connection.reconnectDescription':
		'重新连接此窗口；如 Orchbee 会话已停止，会先重新启动它。',
	'session.connection.unavailable.description': '此窗口无法连接到 Orchbee 会话。',
	'session.connection.unavailable.desktopTooOld':
		'Orchbee Desktop {desktopVersion} 版本过旧，无法加入{hasSessionVersion, select, true {正在运行的会话（{sessionVersion}）} other {正在运行的会话}}。请更新 Desktop 后重试。',
	'session.connection.unavailable.developmentBuildMismatch':
		'此开发构建无法加入正在运行的会话。请运行 orchbee server stop 停止它，然后重试。',
	'session.connection.unavailable.otherEnvironment':
		'此 Orchbee Desktop 属于另一个环境，无法加入正在运行的会话。',
	'session.connection.unavailable.recoveryRequired':
		'该会话需要恢复后才能启动。如需检查，请在终端中运行 orchbee server diagnose。',
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {正在运行的会话（{sessionVersion}）} other {正在运行的会话}}版本过旧，无法与 Orchbee Desktop {desktopVersion} 配合。请运行 orchbee server stop 停止它（其中的终端会随之结束），然后重试。',
	'session.connection.unavailable.title': 'Orchbee 会话不可用',
	'session.connection.unavailable.tryAgain': '重试',
} as const satisfies Readonly<Record<SessionConnectionMessageId, string>>;
