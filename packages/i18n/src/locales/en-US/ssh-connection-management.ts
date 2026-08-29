export const enUSSshConnectionManagementMessages = {
	'ssh.connections.actions.add': 'Add connection',
	'ssh.connections.actions.addHost': 'Add',
	'ssh.connections.actions.adding': 'Adding…',
	'ssh.connections.actions.connect': 'Connect',
	'ssh.connections.actions.disconnect': 'Disconnect',
	'ssh.connections.actions.disconnecting': 'Disconnecting…',
	'ssh.connections.actions.dismissFailure': 'Dismiss error',
	'ssh.connections.actions.refresh': 'Refresh',
	'ssh.connections.actions.retry': 'Try again',
	'ssh.connections.category': 'SSH Connections',
	'ssh.connections.description':
		'Manage the reusable SSH hosts available when you open remote projects and folders.',
	'ssh.connections.discovery.invalidConfig':
		'Some SSH configuration could not be read. Valid Host entries are still shown.',
	'ssh.connections.discovery.readFailed':
		'Orchbee could not read your SSH configuration. You can still add a connection manually.',
	'ssh.connections.discovery.truncated':
		'Your SSH configuration is large, so only the first available Host entries are shown.',
	'ssh.connections.failure.busy': 'Finish the current SSH operation before trying again.',
	'ssh.connections.failure.connection':
		'Orchbee could not change this connection. Check SSH access and try again.',
	'ssh.connections.failure.control':
		'The Desktop control connection was interrupted. Reload the window before trying again.',
	'ssh.connections.failure.storage':
		'Orchbee could not save this connection safely. Restart Orchbee before trying again.',
	'ssh.connections.failure.target':
		'That SSH address could not be resolved. Check the address or SSH Host name.',
	'ssh.connections.fields.destination': 'SSH address',
	'ssh.connections.fields.destinationDescription':
		'Use a Host name from ~/.ssh/config, user@hostname, or paste ssh user@hostname.',
	'ssh.connections.fields.destinationPlaceholder': 'my-server or user@example.com',
	'ssh.connections.managed.description':
		'These connections are shared by every remote project on this device.',
	'ssh.connections.managed.emptyDescription':
		'Add an address above or choose a host found in your SSH config.',
	'ssh.connections.managed.emptyTitle': 'No SSH connections added yet',
	'ssh.connections.managed.projectCount':
		'{count, plural, =0 {Not used by a project yet} one {Used by # project} other {Used by # projects}}',
	'ssh.connections.managed.section': 'Your connections',
	'ssh.connections.privacy.description':
		'Orchbee saves connection details only. Passwords and passphrases go directly to SSH when you connect and are never stored.',
	'ssh.connections.privacy.title': 'Your credentials stay private',
	'ssh.connections.registryUnavailable':
		'Saved SSH connections are temporarily unavailable. Discovery remains visible, but adding and connecting are disabled.',
	'ssh.connections.source.orchbee': 'Added to Orchbee',
	'ssh.connections.source.system': 'System SSH config',
	'ssh.connections.status.blocked': 'Needs attention',
	'ssh.connections.status.connected': 'Connected',
	'ssh.connections.status.connecting': 'Connecting…',
	'ssh.connections.status.disconnected': 'Disconnected',
	'ssh.connections.status.interactionRequired': 'Sign-in required',
	'ssh.connections.status.retrying': 'Reconnecting…',
	'ssh.connections.status.unavailable': 'Unavailable',
	'ssh.connections.suggestions.description':
		'Orchbee automatically found these concrete Host entries on this device. Add one to make it available to remote projects.',
	'ssh.connections.suggestions.emptyDescription':
		'Add another concrete Host entry to ~/.ssh/config, then refresh this list.',
	'ssh.connections.suggestions.emptyTitle': 'No additional SSH hosts found',
	'ssh.connections.suggestions.section': 'From your SSH config',
	'ssh.connections.title': 'SSH Connections',
} as const;

export type SshConnectionManagementMessageId = keyof typeof enUSSshConnectionManagementMessages;
