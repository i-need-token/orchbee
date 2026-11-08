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
	'session.connection.unavailable.recoveryRequired':
		'The Session needs recovery before it can start. To inspect it, run <command>orchbee server diagnose</command> in a terminal.',
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {The running Session ({sessionVersion}) is too old for Orchbee Desktop {desktopVersion}. Run <command>orchbee server stop</command>, then try again.} other {The running Session was started by an Orchbee from before version checks. Run <command>server stop</command> with the Orchbee that started it, then try again.}}',
	'session.connection.unavailable.title': 'The Orchbee Session isn’t available',
	'session.connection.unavailable.tryAgain': 'Try Again',
} as const;

export type SessionConnectionMessageId = keyof typeof enUSSessionConnectionMessages;
