export const enUSWorkbenchMessages = {
	'workbench.emptySessions.description': 'Create an in-memory session to start working.',
	'workbench.emptySessions.title': 'No sessions are open',
	'workbench.navigation.label': 'Workbench navigation',
	'workbench.runtime.local.label': 'Local runtime',
	'workbench.runtime.sessionCount': '{sessionCount, plural, one {# session} other {# sessions}}',
	'workbench.session.status.disconnected': 'Disconnected',
	'workbench.session.status.exited': 'Exited',
	'workbench.session.status.running': 'Running',
	'workbench.session.status.starting': 'Starting',
	'workbench.sessions.heading': 'Sessions',
	'workbench.terminal.actions.closeSplit': 'Close split',
	'workbench.terminal.actions.closeTerminal': 'Close terminal',
	'workbench.terminal.actions.label': 'Terminal actions',
	'workbench.terminal.actions.newTerminal': 'New terminal',
	'workbench.terminal.actions.splitDown': 'Split down',
	'workbench.terminal.actions.splitRight': 'Split right',
	'workbench.terminal.pane.label': '{sessionTitle} terminal',
	'workbench.terminal.start.failed': 'The terminal could not start.',
	'workbench.terminal.start.pending': 'Starting terminal…',
} as const;

export type WorkbenchMessageId = keyof typeof enUSWorkbenchMessages;
