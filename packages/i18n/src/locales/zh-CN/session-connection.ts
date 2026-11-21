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
		'此开发构建无法加入正在运行的会话。请运行 <command>orchbee server stop</command> 停止它，然后重试。',
	'session.connection.unavailable.otherEnvironment':
		'此 Orchbee Desktop 属于另一个环境，无法加入正在运行的会话。',
	'session.connection.unavailable.recoveryBlocked':
		'之前启动的 {role, select, server {Server} worker {Worker} other {Terminal Host}} 仍在运行（{program}，进程 {processId}），Orchbee 无法启动此会话。请在任务管理器中结束该进程或重启电脑，下次启动会自动恢复会话。',
	'session.connection.unavailable.recoveryRequired':
		'该会话需要恢复后才能启动。如需检查，请在终端中运行 <command>orchbee doctor</command>。',
	'session.connection.unavailable.serverNotStarted':
		'{cause, select, missing {Orchbee 无法启动此会话：系统找不到它的程序文件。请重新安装 Orchbee，或检查该文件是否被安全软件移除。} denied {Orchbee 无法启动此会话：系统拒绝访问它的程序文件，可能是安全软件或系统策略阻止了它。} code {Orchbee 无法启动此会话：系统拒绝运行它的程序（错误 {code}）。} other {Orchbee 无法启动此会话：系统拒绝运行它的程序。}}',
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {正在运行的会话（{sessionVersion}）版本过旧，无法与 Orchbee Desktop {desktopVersion} 配合。请运行 <command>orchbee server stop</command>，然后重试。} other {正在运行的会话由早于版本检查的 Orchbee 启动。请用启动它的那个 Orchbee 运行 <command>server stop</command>，然后重试。}}',
	'session.connection.unavailable.title': 'Orchbee 会话不可用',
	'session.connection.unavailable.tryAgain': '重试',
	'session.connection.workbenchFailed.description':
		'终端仍在 Orchbee 会话中运行。重新加载窗口即可再次显示它们。',
	'session.connection.workbenchFailed.reload': '重新加载',
	'session.connection.workbenchFailed.title': '此窗口出了问题',
} as const satisfies Readonly<Record<SessionConnectionMessageId, string>>;
