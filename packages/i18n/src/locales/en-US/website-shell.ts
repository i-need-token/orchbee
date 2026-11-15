export const enUSWebsiteShellMessages = {
	'website.shell.notFound.metaTitle': 'Page not found — Orchbee',
	'website.shell.notFound.title': 'Page not found.',
	'website.shell.notFound.body':
		'There is nothing at this address. The page may have moved, or the address may be mistyped.',
	'website.shell.notFound.home': 'Back to the home page',
	'website.shell.skip': 'Skip to content',
	'website.shell.navigation': 'Clients',
	'website.shell.menu': 'Menu',
	'website.shell.footerNavigation': 'Project',
	'website.shell.nav.terminal': 'Terminal',
	'website.shell.nav.desktop': 'Desktop',
	'website.shell.nav.cli': 'Command line',
	'website.shell.nav.download': 'Download',
	'website.shell.nav.install': 'Install',
	'website.shell.otherLanguage': '中文',
	'website.shell.client.terminal': 'Terminal interface',
	'website.shell.client.desktop': 'Desktop application',
	'website.shell.client.cli': 'Command line',
	'website.shell.client.terminalSummary':
		'Runs inside the terminal you already use, with a sidebar, Tabs, and split Panels.',
	'website.shell.client.desktopSummary':
		'Presents the same Session in its own window. The terminal renders with WebGPU.',
	'website.shell.client.cliSummary':
		'Every operation available to a person is equally available to an agent through the <code>orchbee</code> command line.',
	'website.shell.otherClients': 'other clients',
	'website.shell.downloadForWindows': 'Download for Windows',
	'website.shell.getSource': 'Get the source',
	'website.shell.installerNote':
		'Installs for the current user only; no administrator rights needed. Adds the <code>orchbee</code> command to your <code>Path</code>.',
	'website.shell.copyCommand': 'Copy command',
	'website.shell.footer.name':
		'The name combines <b>orch</b>estrator with <b>bee</b>—independent workers coordinating as one.',
	'website.shell.footer.releases': 'Releases',
	'website.shell.footer.license': 'License',
	'website.shell.detail.highlights': 'in detail',
	'website.shell.detail.reference': 'reference',
	'website.shell.panelPlay.label': 'Panels you can split, focus, resize and close',
	'website.shell.panelPlay.help':
		'While the window has focus, keys act on its Panels. Tab or Escape leaves the window.',
	'website.shell.panelPlay.splitRight': 'Split right',
	'website.shell.panelPlay.splitDown': 'Split down',
	'website.shell.panelPlay.splitLeft': 'Split left',
	'website.shell.panelPlay.splitUp': 'Split up',
	'website.shell.panelPlay.maximize': 'Maximize / Restore',
	'website.shell.panelPlay.equalize': 'Equalize Panels',
	'website.shell.panelPlay.close': 'Close Panel',
	'website.shell.panelPlay.resize': 'Resize',
	'website.shell.panelPlay.idle': 'Click the window, then press a key.',
	'website.shell.panelPlay.prefix': '<b>Ctrl+b</b> The prefix. Now press one key.',
	'website.shell.panelPlay.full': 'Six Panels fill this window. Close one first.',
	'website.shell.panelPlay.small': 'This Panel is too small to split that way.',
	'website.shell.panelPlay.last': 'The last Panel stays.',
	'website.shell.panelPlay.one': 'Split first. A single Panel already fills the window.',
	'website.shell.panelPlay.none': 'There is no divider to move in that direction.',
} as const;

export type WebsiteShellMessageId = keyof typeof enUSWebsiteShellMessages;
