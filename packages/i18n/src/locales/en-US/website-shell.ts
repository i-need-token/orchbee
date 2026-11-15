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
} as const;

export type WebsiteShellMessageId = keyof typeof enUSWebsiteShellMessages;
