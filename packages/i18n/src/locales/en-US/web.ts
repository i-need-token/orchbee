export const enUSWebMessages = {
	'web.inMemory.commands.help.output': 'Available commands: help, status, clear',
	'web.inMemory.commands.simulated.output': 'Simulated command: {command}',
	'web.inMemory.commands.status.output':
		'In-memory session · {columnCount, plural, one {# column} other {# columns}} × {rowCount, plural, one {# row} other {# rows}}',
	'web.inMemory.sessions.agentOutput.introduction':
		'Codex agent · ready\nWaiting for a task in the first Workbench vertical slice.',
	'web.inMemory.sessions.agentOutput.title': 'Agent output',
	'web.inMemory.sessions.localShell.introduction':
		'Orchbee Workbench\nThis terminal is backed by the in-memory session adapter. Type help to begin.',
	'web.inMemory.sessions.localShell.title': 'Local shell',
	'web.inMemory.sessions.new.introduction': 'New in-memory terminal session.',
	'web.inMemory.sessions.new.title': 'Session {sessionNumber, number}',
} as const;

export type WebMessageId = keyof typeof enUSWebMessages;
