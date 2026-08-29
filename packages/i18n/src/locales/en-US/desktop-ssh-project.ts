export const enUSDesktopSshProjectMessages = {
	'desktop.sshProject.actions.cancel': 'Cancel',
	'desktop.sshProject.actions.close': 'Close',
	'desktop.sshProject.actions.connect': 'Connect',
	'desktop.sshProject.actions.continue': 'Continue',
	'desktop.sshProject.actions.retry': 'Retry',
	'desktop.sshProject.connection.description':
		'Orchbee will reconnect this Project without changing its saved structure or remote files.',
	'desktop.sshProject.connection.title': 'Connect Remote Project',
	'desktop.sshProject.description':
		'Connect to a trusted SSH host, validate a remote Project folder, and add it to Orchbee.',
	'desktop.sshProject.failure.busy': 'Another SSH Project operation is already in progress.',
	'desktop.sshProject.failure.connection':
		'Orchbee could not establish the remote runtime connection. Check SSH access and try again.',
	'desktop.sshProject.failure.control':
		'The Desktop control connection was interrupted. Reload the window before trying again.',
	'desktop.sshProject.failure.projectPath':
		'The remote Project path could not be used. Check that it is an accessible absolute folder path.',
	'desktop.sshProject.failure.storage':
		'Orchbee could not safely record this SSH Environment. Restart Orchbee before trying again.',
	'desktop.sshProject.failure.target':
		'The SSH destination or configuration could not be resolved safely.',
	'desktop.sshProject.failure.title': 'Remote Project Could Not Be Connected',
	'desktop.sshProject.failure.unsupported':
		'SSH Project connections are not supported on this Desktop platform.',
	'desktop.sshProject.failure.workbench':
		'Orchbee could not safely add the Project to the shared Workbench. Reload and try again.',
	'desktop.sshProject.fields.configurationFilePath': 'SSH configuration file (optional)',
	'desktop.sshProject.fields.configurationFilePathDescription':
		'Use an absolute path to an OpenSSH configuration file.',
	'desktop.sshProject.fields.configurationFilePathPlaceholder': '/Users/you/.ssh/config',
	'desktop.sshProject.fields.destination': 'SSH destination',
	'desktop.sshProject.fields.destinationPlaceholder': 'build-host or user@example.com',
	'desktop.sshProject.fields.name': 'Environment name',
	'desktop.sshProject.fields.namePlaceholder': 'Build machine',
	'desktop.sshProject.fields.projectPath': 'Remote Project path',
	'desktop.sshProject.fields.projectPathDescription':
		'Use an absolute POSIX folder path on the remote host.',
	'desktop.sshProject.fields.projectPathPlaceholder': '/srv/project',
	'desktop.sshProject.progress.committingProject': 'Adding the Project to the Workbench…',
	'desktop.sshProject.progress.connecting': 'Connecting and preparing the remote runtime…',
	'desktop.sshProject.progress.inspectingProject': 'Validating the remote Project folder…',
	'desktop.sshProject.progress.recordingEnvironment': 'Recording the SSH Environment…',
	'desktop.sshProject.progress.resolvingTarget': 'Resolving the SSH configuration…',
	'desktop.sshProject.prompt.authenticationDescription':
		'This response is sent directly to the active SSH process and is not saved.',
	'desktop.sshProject.prompt.authenticationTitle': 'SSH Authentication Required',
	'desktop.sshProject.prompt.hostDescription':
		'Review the SSH host verification prompt before continuing.',
	'desktop.sshProject.prompt.hostTitle': 'Verify SSH Host',
	'desktop.sshProject.prompt.response': 'Response',
	'desktop.sshProject.title': 'Connect via SSH',
} as const;

export type DesktopSshProjectMessageId = keyof typeof enUSDesktopSshProjectMessages;
