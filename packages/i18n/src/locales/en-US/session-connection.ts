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
	'session.connection.unavailable.recoveryBlocked':
		"Orchbee can't start this Session while the {role, select, server {Server} worker {Worker} other {Terminal Host}} from an earlier start is still running ({program}, process {processId}). End that process in Task Manager or restart the computer; the next start then recovers the Session by itself.",
	'session.connection.unavailable.recoveryRequired':
		'The Session needs recovery before it can start. To inspect it, run <command>orchbee doctor</command> in a terminal.',
	'session.connection.unavailable.serverNotStarted':
		"{cause, select, missing {Orchbee couldn't start this Session: the system couldn't find its program file. Reinstall Orchbee, or check whether security software removed the file.} denied {Orchbee couldn't start this Session: the system denied access to its program file. Security software or a system policy may be blocking it.} code {Orchbee couldn't start this Session: the system refused to run its program (error {code}).} other {Orchbee couldn't start this Session: the system refused to run its program.}}",
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {The running Session ({sessionVersion}) is too old for Orchbee Desktop {desktopVersion}. Run <command>orchbee server stop</command>, then try again.} other {The running Session was started by an Orchbee from before version checks. Run <command>server stop</command> with the Orchbee that started it, then try again.}}',
	'session.connection.unavailable.title': 'The Orchbee Session isn’t available',
	'session.connection.unavailable.tryAgain': 'Try Again',
	'session.connection.workbenchFailed.description':
		'Your terminals keep running in the Orchbee Session. Reload the window to show them again.',
	'session.connection.workbenchFailed.reload': 'Reload',
	'session.connection.workbenchFailed.title': 'Something went wrong in this window',
} as const;

export type SessionConnectionMessageId = keyof typeof enUSSessionConnectionMessages;
