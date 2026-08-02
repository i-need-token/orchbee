export const enUSDesktopApplicationLifetimeMessages = {
	'desktop.applicationLifetime.actions.cancel': 'Cancel',
	'desktop.applicationLifetime.actions.preserveAndQuit': 'Keep Sessions and Quit',
	'desktop.applicationLifetime.actions.quitAnyway': 'Quit Anyway',
	'desktop.applicationLifetime.actions.retry': 'Retry',
	'desktop.applicationLifetime.actions.stopAndQuit': 'Stop Sessions and Quit',
	'desktop.applicationLifetime.exit.description':
		'{sessionCount, plural, one {# terminal session is running.} other {# terminal sessions are running.}}',
	'desktop.applicationLifetime.exit.title': 'Quit Orchbee?',
	'desktop.applicationLifetime.failed.description':
		'Some terminal sessions or processes may still be running. Retry, or quit anyway and leave unresolved processes running.',
	'desktop.applicationLifetime.failed.title': 'Couldn’t finish quitting',
	'desktop.applicationLifetime.progress.draining': 'Checking remaining terminal sessions…',
	'desktop.applicationLifetime.progress.exiting': 'Quitting Orchbee…',
	'desktop.applicationLifetime.progress.flushing': 'Saving your workspace…',
	'desktop.applicationLifetime.progress.stopping':
		'Stopped {completedCount, number} of {totalCount, number} terminal sessions.',
	'desktop.applicationLifetime.progress.title': 'Preparing to quit…',
} as const;

export type DesktopApplicationLifetimeMessageId =
	keyof typeof enUSDesktopApplicationLifetimeMessages;
