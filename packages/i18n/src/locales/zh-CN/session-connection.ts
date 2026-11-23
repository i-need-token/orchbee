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
	'session.connection.unavailable.pathRefused.accessControlList':
		'{system, select, windows {Orchbee 无法使用 {path}：其权限中有 Orchbee 无法核对的条目。请在“属性 > 安全 > 高级”中移除该条目，或将 ORCHBEE_HOME 设为只有你能打开的文件夹。} other {Orchbee 无法使用 {path}：它的访问控制列表可能允许其他用户访问。请运行 <command>chmod -RN {path}</command>，然后重新启动 Orchbee。}}',
	'session.connection.unavailable.pathRefused.locksUnsupported':
		'Orchbee 无法把数据保存在 {path}：此文件系统不支持文件锁。请将 ORCHBEE_HOME 设为本机磁盘上的文件夹。',
	'session.connection.unavailable.pathRefused.otherOwner':
		'{system, select, windows {Orchbee 无法使用 {path}：其所有者不是你、SYSTEM 或 Administrators。请在“属性 > 安全 > 高级”中将你的账户设为其所有者，或将 ORCHBEE_HOME 设为你拥有的文件夹。} other {Orchbee 无法使用 {path}：它属于其他用户。如果你用 sudo 启动了 Orchbee，请以自己的身份运行，或将 ORCHBEE_HOME 设为你拥有的文件夹。}}',
	'session.connection.unavailable.pathRefused.othersCanChange':
		'{system, select, windows {Orchbee 无法使用 {path}：它没有权限设置，任何用户都可以修改它。请将 ORCHBEE_HOME 设为本机磁盘上的文件夹。} other {Orchbee 无法使用 {path}：其他用户可以修改它。请运行 <command>chmod go-w {path}</command>，然后重新启动 Orchbee。}}',
	'session.connection.unavailable.pathRefused.othersCanRead':
		'{system, select, windows {Orchbee 无法使用 {path}：其权限允许你、SYSTEM 和 Administrators 以外的用户访问。请在“属性 > 安全 > 高级”中移除该访问权限，或将 ORCHBEE_HOME 设为只有你能打开的文件夹。} other {Orchbee 无法使用 {path}：其他用户可以读取它。请运行 <command>chmod -R go-rwx {path}</command>，然后重新启动 Orchbee。}}',
	'session.connection.unavailable.pathRefused.permissionsNotKept':
		'Orchbee 无法把数据保存在 {path}：此文件系统不保留文件权限。请将 ORCHBEE_HOME 设为本机磁盘上的文件夹。',
	'session.connection.unavailable.pathRefused.symbolicLink':
		'{system, select, windows {Orchbee 无法使用 {path}：它是链接或联接点。Orchbee 只在真实的文件夹中保存数据；请将 ORCHBEE_HOME 设为这样的文件夹。} other {Orchbee 无法使用 {path}：它是符号链接。只有数据文件夹本身和 config.toml 可以是链接。}}',
	'session.connection.unavailable.pathRefused.unsafeSocketFolder':
		'Orchbee 无法用 {path} 存放套接字：它属于其他用户或对其他用户开放。请删除它，然后重新启动 Orchbee。',
	'session.connection.unavailable.recoveryBlocked':
		'之前启动的 {role, select, server {Server} worker {Worker} other {Terminal Host}} 仍在运行（{program}，进程 {processId}），Orchbee 无法启动此会话。请在{system, select, macos {活动监视器} other {任务管理器}}中结束该进程或重启电脑，下次启动会自动恢复会话。',
	'session.connection.unavailable.recoveryRequired':
		'该会话需要恢复后才能启动。如需检查，请在终端中运行 <command>orchbee doctor</command>。',
	'session.connection.unavailable.serverNotStarted':
		'{cause, select, missing {Orchbee 无法启动此会话：系统找不到它的程序文件。请重新安装 Orchbee，或检查该文件是否被安全软件移除。} denied {Orchbee 无法启动此会话：系统拒绝访问它的程序文件，可能是安全软件或系统策略阻止了它。} code {Orchbee 无法启动此会话：系统拒绝运行它的程序（错误 {code}）。} other {Orchbee 无法启动此会话：系统拒绝运行它的程序。}}',
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {正在运行的会话（{sessionVersion}）版本过旧，无法与 Orchbee Desktop {desktopVersion} 配合。{restartable, select, true {请运行 <command>orchbee server restart</command>，然后重试。终端会继续运行。} other {请运行 <command>orchbee server stop</command>，然后重试。}}} other {正在运行的会话由早于版本检查的 Orchbee 启动。请用启动它的那个 Orchbee 运行 <command>server stop</command>，然后重试。}}',
	'session.connection.unavailable.title': 'Orchbee 会话不可用',
	'session.connection.unavailable.tryAgain': '重试',
	'session.connection.workbenchFailed.description':
		'终端仍在 Orchbee 会话中运行。重新加载窗口即可再次显示它们。',
	'session.connection.workbenchFailed.reload': '重新加载',
	'session.connection.workbenchFailed.title': '此窗口出了问题',
} as const satisfies Readonly<Record<SessionConnectionMessageId, string>>;
