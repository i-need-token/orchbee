export const enUSSessionConnectionMessages = {
	'session.connection.disconnected': 'Disconnected',
	'session.connection.reconnect': 'Reconnect',
	'session.connection.reconnectDescription':
		'Reconnects this window, starting the Orchbee session again if it stopped.',
	'session.connection.unavailable.description':
		'This window couldn’t connect to the Orchbee Session.',
	'session.connection.unavailable.desktopTooOld':
		'Orchbee Desktop {desktopVersion} is too old for {hasSessionVersion, select, true {the running Session ({sessionVersion})} other {the running Session}}. Update Desktop, then try again.',
	'session.connection.unavailable.developmentBuildMismatch':
		'This development build cannot join the running Session. Stop it with <command>orchbee server stop</command>, then try again.',
	'session.connection.unavailable.otherEnvironment':
		'This copy of Orchbee Desktop belongs to another environment and cannot join the running Session.',
	'session.connection.unavailable.pathRefused.accessControlList':
		"{system, select, windows {Orchbee can't use {path}: its permissions hold an entry Orchbee can't check. Remove that entry under Properties > Security > Advanced, or set ORCHBEE_HOME to a folder only you can open.} other {Orchbee can't use {path}: it has an access control list that can let other users in. Run <command>chmod -RN {path}</command>, then start Orchbee again.}}",
	'session.connection.unavailable.pathRefused.locksUnsupported':
		"Orchbee can't keep its data in {path}: this file system doesn't support file locks. Set ORCHBEE_HOME to a folder on this computer's own disk.",
	'session.connection.unavailable.pathRefused.otherOwner':
		"{system, select, windows {Orchbee can't use {path}: its owner is an account other than you, SYSTEM or Administrators. Make your account its owner under Properties > Security > Advanced, or set ORCHBEE_HOME to a folder you own.} other {Orchbee can't use {path}: it belongs to another user. If you started Orchbee with sudo, run it as yourself, or set ORCHBEE_HOME to a folder you own.}}",
	'session.connection.unavailable.pathRefused.othersCanChange':
		"{system, select, windows {Orchbee can't use {path}: it has no permissions, so every account can change it. Set ORCHBEE_HOME to a folder on this computer's own disk.} other {Orchbee can't use {path}: other users can change it. Run <command>chmod go-w {path}</command>, then start Orchbee again.}}",
	'session.connection.unavailable.pathRefused.othersCanRead':
		"{system, select, windows {Orchbee can't use {path}: its permissions let in an account other than you, SYSTEM or Administrators. Remove that access under Properties > Security > Advanced, or set ORCHBEE_HOME to a folder only you can open.} other {Orchbee can't use {path}: other users can read it. Run <command>chmod -R go-rwx {path}</command>, then start Orchbee again.}}",
	'session.connection.unavailable.pathRefused.permissionsNotKept':
		"Orchbee can't keep its data in {path}: this file system doesn't keep file permissions. Set ORCHBEE_HOME to a folder on this computer's own disk.",
	'session.connection.unavailable.pathRefused.symbolicLink':
		"{system, select, windows {Orchbee can't use {path}: it is a link or junction. Orchbee keeps its data only in real folders; set ORCHBEE_HOME to one.} other {Orchbee can't use {path}: it is a symbolic link. Only the data folder itself and config.toml may be links.}}",
	'session.connection.unavailable.pathRefused.unsafeSocketFolder':
		"Orchbee can't use {path} for its sockets: it belongs to another user or is open to other users. Remove it, then start Orchbee again.",
	'session.connection.unavailable.recoveryBlocked':
		"Orchbee can't start this Session while the {role, select, server {Server} worker {Worker} other {Terminal Host}} from an earlier start is still running ({program}, process {processId}). End that process in {system, select, macos {Activity Monitor} other {Task Manager}} or restart the computer; the next start then recovers the Session by itself.",
	'session.connection.unavailable.recoveryRequired':
		'The Session needs recovery before it can start. To inspect it, run <command>orchbee doctor</command> in a terminal.',
	'session.connection.unavailable.serverNotStarted':
		"{cause, select, missing {Orchbee couldn't start this Session: the system couldn't find its program file. Reinstall Orchbee, or check whether security software removed the file.} denied {Orchbee couldn't start this Session: the system denied access to its program file. Security software or a system policy may be blocking it.} code {Orchbee couldn't start this Session: the system refused to run its program (error {code}).} other {Orchbee couldn't start this Session: the system refused to run its program.}}",
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {The running Session ({sessionVersion}) is too old for Orchbee Desktop {desktopVersion}. {restartable, select, true {Run <command>orchbee server restart</command>, then try again. Terminals keep running.} other {Run <command>orchbee server stop</command>, then try again.}}} other {The running Session was started by an Orchbee from before version checks. Run <command>server stop</command> with the Orchbee that started it, then try again.}}',
	'session.connection.unavailable.title': 'The Orchbee Session isn’t available',
	'session.connection.unavailable.tryAgain': 'Try Again',
	'session.connection.workbenchFailed.description':
		'Your terminals keep running in the Orchbee Session. Reload the window to show them again.',
	'session.connection.workbenchFailed.reload': 'Reload',
	'session.connection.workbenchFailed.title': 'Something went wrong in this window',
} as const;

export type SessionConnectionMessageId = keyof typeof enUSSessionConnectionMessages;
