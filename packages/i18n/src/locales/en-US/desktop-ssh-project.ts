export const enUSDesktopSshProjectMessages = {
	'desktop.sshProject.actions.authenticate': 'Authenticate',
	'desktop.sshProject.actions.cancel': 'Cancel',
	'desktop.sshProject.actions.close': 'Close',
	'desktop.sshProject.actions.connect': 'Connect',
	'desktop.sshProject.actions.retry': 'Retry',
	'desktop.sshProject.actions.trustHost': 'Trust and Continue',
	'desktop.sshProject.attachment.description':
		'Enter an SSH address. Orchbee will connect, then let you choose a remote folder for this Project.',
	'desktop.sshProject.attachment.directoryDescription':
		'Choose the remote folder to add to this Project.',
	'desktop.sshProject.attachment.directoryTitle': 'Choose a Remote Folder',
	'desktop.sshProject.attachment.title': 'Add SSH Folder',
	'desktop.sshProject.connection.description':
		'Orchbee will reconnect this Project without changing its saved structure or remote files.',
	'desktop.sshProject.connection.title': 'Connect Remote Project',
	'desktop.sshProject.description':
		'Connect to an SSH host, then choose the remote folder for your Project.',
	'desktop.sshProject.directory.description':
		'Choose the remote folder you want to add as a Project.',
	'desktop.sshProject.directory.title': 'Choose a Project Folder',
	'desktop.sshProject.environmentConnection.description':
		'Authenticate this reusable connection. Passwords and passphrases are sent only to the active SSH process.',
	'desktop.sshProject.environmentConnection.failureTitle': 'SSH Connection Could Not Be Opened',
	'desktop.sshProject.environmentConnection.title': 'Connect SSH Connection',
	'desktop.sshProject.failure.busy': 'Another SSH Project operation is already in progress.',
	'desktop.sshProject.failure.connection':
		'Orchbee could not establish the remote runtime connection. Check SSH access and try again.',
	'desktop.sshProject.failure.control':
		'The Desktop control connection was interrupted. Reload the window before trying again.',
	'desktop.sshProject.failure.projectAttachment':
		'This Project already uses another SSH host, or this remote folder belongs to another Project.',
	'desktop.sshProject.failure.projectPath':
		'The remote Project path could not be used. Check that it is an accessible absolute folder path.',
	'desktop.sshProject.failure.storage':
		'Orchbee could not safely record this SSH Environment. Restart Orchbee before trying again.',
	'desktop.sshProject.failure.target': 'The SSH address could not be resolved safely.',
	'desktop.sshProject.failure.title': 'Remote Project Could Not Be Connected',
	'desktop.sshProject.failure.unsupported':
		'SSH Project connections are not supported on this Desktop platform.',
	'desktop.sshProject.failure.workbench':
		'Orchbee could not safely add the Project to the shared Workbench. Reload and try again.',
	'desktop.sshProject.fields.destination': 'SSH address',
	'desktop.sshProject.fields.destinationDescription':
		'Use a host from your SSH config or enter user@hostname.',
	'desktop.sshProject.fields.destinationError': 'Enter a valid SSH host, such as user@hostname.',
	'desktop.sshProject.fields.destinationPlaceholder': 'user@hostname',
	'desktop.sshProject.progress.committingProject': 'Adding the Project to the Workbench…',
	'desktop.sshProject.progress.committingProjectFolder': 'Adding the remote folder to the Project…',
	'desktop.sshProject.progress.connecting': 'Connecting and preparing the remote runtime…',
	'desktop.sshProject.progress.inspectingProject': 'Validating the remote Project folder…',
	'desktop.sshProject.progress.recordingEnvironment': 'Recording the SSH Environment…',
	'desktop.sshProject.progress.resolvingTarget': 'Resolving the SSH configuration…',
	'desktop.sshProject.progress.selectingDirectory': 'Loading your remote home folder…',
	'desktop.sshProject.prompt.authenticationDescription':
		'Your password or passphrase is sent only to this SSH process and is never saved.',
	'desktop.sshProject.prompt.authenticationTitle': 'SSH Authentication Required',
	'desktop.sshProject.prompt.hostDescription':
		'Confirm the host fingerprint before trusting this connection.',
	'desktop.sshProject.prompt.hostTitle': 'Verify SSH Host',
	'desktop.sshProject.prompt.secret': 'Password or passphrase',
	'desktop.sshProject.title': 'Connect via SSH',
} as const;

export type DesktopSshProjectMessageId = keyof typeof enUSDesktopSshProjectMessages;
