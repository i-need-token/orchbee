export const enUSWebsiteHomeMessages = {
	'website.home.metaTitle':
		'Orchbee — A modern, multiplexed terminal for running and orchestrating AI agents',
	'website.home.metaDescription':
		'Sessions persist in the background and remain accessible from the terminal, the desktop, and the command line simultaneously, giving people and agents a shared environment to work in.',
	'website.home.title':
		'A modern, multiplexed terminal for running and orchestrating AI\u00A0agents.',
	'website.home.lead':
		'Sessions persist in the background and remain accessible from the terminal, the desktop, and the command line simultaneously, giving people and agents a shared environment to work in.',
	'website.home.beta':
		'Beta. Runs on Windows 11 (x86_64) today; macOS and Linux builds are in progress.',
	'website.home.stage.view': 'Viewing this Session in the',
	'website.home.stage.closeHint': 'Closing the window does not end the Session.',
	'website.home.stage.close': 'Close window',
	'website.home.stage.awayTitle': 'The Session is running in the background.',
	'website.home.stage.awayBody': 'Its shells and agents continue while no client is attached.',
	'website.home.stage.closedFor': 'Window closed for',
	'website.home.stage.reattach': 'Reattach',
	'website.home.stage.caption':
		'<b>Demo</b>One Session with a shell and two agents, drawn from the real interface and played from a script.',
	'website.home.clients.kicker': 'clients',
	'website.home.clients.title': 'Each client in detail',
	'website.home.clients.lead':
		'The terminal interface, the desktop application, and the command line present and control the same Session. Each has a page of its own.',
	'website.home.install.kicker': 'beta',
	'website.home.install.title': 'Install',
	'website.home.install.lead':
		'Orchbee is in beta. It runs on Windows 11 (x86_64) today; macOS and Linux builds are in progress.',
	'website.home.install.releaseNotes': '{version} · Release notes',
	'website.home.install.app': 'Desktop app, terminal and CLI',
	'website.home.install.npm': 'Terminal and CLI only',
	'website.home.install.npmNote':
		'Requires Node.js 22 or later. Installs the terminal interface and the CLI, without the desktop app.',
	'website.home.install.start': 'Start',
	'website.home.install.startNote':
		'Opens the terminal interface. Or open Orchbee from the Start menu.',
	'website.home.install.note':
		'The installer is not code-signed yet, so Windows SmartScreen may ask you to confirm.',
	'website.home.install.source': 'Get the source and install its dependencies',
	'website.home.install.sourceRequirements':
		'Windows 11. Needs Git, Node.js 22+, Corepack, Rust 1.97.0 and Zig 0.16.0.',
	'website.home.install.sourceStart': 'Start the desktop app',
	'website.home.install.sourceNote':
		'Pre-built apps aren’t available yet; this builds from the source.',
} as const;

export type WebsiteHomeMessageId = keyof typeof enUSWebsiteHomeMessages;
