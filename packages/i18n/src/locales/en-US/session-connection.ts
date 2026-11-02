export const enUSSessionConnectionMessages = {
	'session.connection.disconnected': 'Disconnected',
	'session.connection.reconnect': 'Reconnect',
	'session.connection.reconnectDescription':
		'Reconnects this window, starting the Orchbee session again if it stopped.',
	'session.connection.unavailable.description':
		'This window couldn’t connect to the Orchbee session.',
	'session.connection.unavailable.recoveryRequired':
		'The session needs recovery before it can start. To inspect it, run orchbee server diagnose in a terminal.',
	'session.connection.unavailable.title': 'The Orchbee session isn’t available',
	'session.connection.unavailable.tryAgain': 'Try Again',
} as const;

export type SessionConnectionMessageId = keyof typeof enUSSessionConnectionMessages;
