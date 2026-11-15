import { defineMessages } from 'react-intl';

import { enUSWebsiteShellMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteShellMessages);

export const websiteShellMessages = defineMessages({
	notFoundMetaTitle: message('website.shell.notFound.metaTitle', 'Document title of the 404 page.'),
	notFoundTitle: message('website.shell.notFound.title', 'Headline of the 404 page.'),
	notFoundBody: message('website.shell.notFound.body', 'Explanation under the 404 headline.'),
	notFoundHome: message(
		'website.shell.notFound.home',
		'Button on the 404 page that leads to the home page.',
	),
	skip: message('website.shell.skip', 'Keyboard link that jumps past the navigation bar.'),
	navigation: message(
		'website.shell.navigation',
		'Accessible name of the navigation that links the three client pages.',
	),
	menu: message(
		'website.shell.menu',
		'Accessible name of the button that opens the navigation menu on narrow screens.',
	),
	footerNavigation: message(
		'website.shell.footerNavigation',
		'Accessible name of the footer links.',
	),
	navTerminal: message(
		'website.shell.nav.terminal',
		'Navigation link to the terminal interface page; shown in lowercase.',
	),
	navDesktop: message(
		'website.shell.nav.desktop',
		'Navigation link to the desktop application page; shown in lowercase.',
	),
	navCli: message(
		'website.shell.nav.cli',
		'Navigation link to the command line page; shown in lowercase.',
	),
	navDownload: message(
		'website.shell.nav.download',
		'Navigation button to the install section once a release is published.',
	),
	navInstall: message(
		'website.shell.nav.install',
		'Navigation button to the install section while no release is published.',
	),
	otherLanguage: message(
		'website.shell.otherLanguage',
		'Link to the same page in the other language, written in that language.',
	),
	clientTerminal: message(
		'website.shell.client.terminal',
		'Name of the terminal interface client.',
	),
	clientDesktop: message('website.shell.client.desktop', 'Name of the desktop application client.'),
	clientCli: message('website.shell.client.cli', 'Name of the command line client.'),
	clientTerminalSummary: message(
		'website.shell.client.terminalSummary',
		'One-sentence summary of the terminal interface in the client index.',
	),
	clientDesktopSummary: message(
		'website.shell.client.desktopSummary',
		'One-sentence summary of the desktop application in the client index.',
	),
	clientCliSummary: message(
		'website.shell.client.cliSummary',
		'One-sentence summary of the command line in the client index; <code> marks the command name.',
	),
	otherClients: message(
		'website.shell.otherClients',
		'Lowercase label above the index of the other two clients on a client page.',
	),
	downloadForWindows: message(
		'website.shell.downloadForWindows',
		'Button that downloads the Windows installer.',
	),
	getSource: message(
		'website.shell.getSource',
		'Button to the build instructions while no release is published.',
	),
	installerNote: message(
		'website.shell.installerNote',
		'What the Windows installer does; <code> marks a command and a variable name.',
	),
	copyCommand: message(
		'website.shell.copyCommand',
		'Accessible name of the button that copies a command.',
	),
	footerName: message(
		'website.shell.footer.name',
		'Where the name comes from; <b> marks the two parts of the word.',
	),
	footerReleases: message(
		'website.shell.footer.releases',
		'Footer link to the release list; shown in lowercase.',
	),
	footerLicense: message(
		'website.shell.footer.license',
		'Footer link to the license; shown in lowercase.',
	),
});
