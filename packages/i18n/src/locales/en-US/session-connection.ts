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
		'This development build cannot join the running Session. Stop it with orchbee server stop, then try again.',
	'session.connection.unavailable.otherEnvironment':
		'This copy of Orchbee Desktop belongs to another environment and cannot join the running Session.',
	'session.connection.unavailable.recoveryRequired':
		'The Session needs recovery before it can start. To inspect it, run orchbee server diagnose in a terminal.',
	'session.connection.unavailable.sessionTooOld':
		'{hasSessionVersion, select, true {The running Session ({sessionVersion})} other {The running Session}} is too old for Orchbee Desktop {desktopVersion}. Stop it with orchbee server stop, which ends its terminals, then try again.',
	'session.connection.unavailable.title': 'The Orchbee Session isn’t available',
	'session.connection.unavailable.tryAgain': 'Try Again',
} as const;

export type SessionConnectionMessageId = keyof typeof enUSSessionConnectionMessages;
