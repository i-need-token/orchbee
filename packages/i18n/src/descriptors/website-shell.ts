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
		'Button that opens the download dialog once a release is published, in the navigation bar and the home hero.',
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
	downloadForMac: message(
		'website.shell.downloadForMac',
		'Button that leads to the macOS downloads in the install section once a release offers macOS.',
	),
	downloadForMacAppleSilicon: message(
		'website.shell.downloadForMacAppleSilicon',
		'Button that downloads the macOS disk image for Apple silicon.',
	),
	downloadForMacIntel: message(
		'website.shell.downloadForMacIntel',
		'Link that downloads the macOS disk image for Intel Macs.',
	),
	downloadMacIntel: message(
		'website.shell.download.macIntel',
		'Button that downloads the macOS disk image for Intel Macs, in the download dialog.',
	),
	downloadTitle: message('website.shell.download.title', 'Heading of the download dialog.'),
	downloadClose: message(
		'website.shell.download.close',
		'Accessible name of the button that closes the download dialog.',
	),
	downloadOther: message(
		'website.shell.download.other',
		'Heading above the other ways to install in the download dialog; the dialog shows it in lowercase.',
	),
	downloadNpm: message(
		'website.shell.download.npm',
		'Label above the npm command in the download dialog; the package is for the CLI only.',
	),
	downloadScriptDesktop: message(
		'website.shell.download.scriptDesktop',
		'Label above the macOS install script command that installs the desktop app, terminal interface, and CLI.',
	),
	downloadScriptCli: message(
		'website.shell.download.scriptCli',
		'Label above the macOS install script command that installs only the terminal interface and CLI.',
	),
	downloadReleaseNotes: message(
		'website.shell.download.releaseNotes',
		'Link in the download dialog to the notes of the published release; {version} is its version.',
	),
	allInstallOptions: message(
		'website.shell.allInstallOptions',
		'Link to the install section showing all platforms and command-line installation methods.',
	),
	getSource: message(
		'website.shell.getSource',
		'Button to the build instructions while no release is published.',
	),
	installerNote: message(
		'website.shell.installerNote',
		'What the Windows installer does; <code> marks a command and a variable name.',
	),
	installerNoteWindows: message(
		'website.shell.installerNoteWindows',
		'What the Windows installer does, led by the platform name once a release also offers macOS; <code> marks a command and a variable name.',
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
	detailHighlights: message(
		'website.shell.detail.highlights',
		'Lowercase label above the highlights of a client page.',
	),
	detailReference: message(
		'website.shell.detail.reference',
		'Lowercase label above the key or command reference of a client page.',
	),
	panelPlayLabel: message(
		'website.shell.panelPlay.label',
		'Accessible name of the drawn window whose Panels the visitor splits, focuses, resizes and closes.',
	),
	panelPlayHelp: message(
		'website.shell.panelPlay.help',
		'Accessible description of that window: what keys do while it has focus and how to leave it.',
	),
	panelPlaySplitRight: message(
		'website.shell.panelPlay.splitRight',
		'Button and notice: the Panel action that splits the focused Panel to the right.',
	),
	panelPlaySplitDown: message(
		'website.shell.panelPlay.splitDown',
		'Button and notice: the Panel action that splits the focused Panel downward.',
	),
	panelPlaySplitLeft: message(
		'website.shell.panelPlay.splitLeft',
		'Button and notice: the Panel action that splits the focused Panel to the left.',
	),
	panelPlaySplitUp: message(
		'website.shell.panelPlay.splitUp',
		'Button and notice: the Panel action that splits the focused Panel upward.',
	),
	panelPlayMaximize: message(
		'website.shell.panelPlay.maximize',
		'Button and notice: the Panel action that maximizes the focused Panel or restores the layout.',
	),
	panelPlayEqualize: message(
		'website.shell.panelPlay.equalize',
		'Button and notice: the Panel action that gives every Panel an equal share.',
	),
	panelPlayClose: message(
		'website.shell.panelPlay.close',
		'Button and notice: the Panel action that closes the focused Panel.',
	),
	panelPlayResize: message(
		'website.shell.panelPlay.resize',
		'Notice after the visitor moved a Panel divider with the keyboard.',
	),
	panelPlayIdle: message(
		'website.shell.panelPlay.idle',
		'Notice before the visitor has used the window: how to start.',
	),
	panelPlayPrefix: message(
		'website.shell.panelPlay.prefix',
		'Notice after the visitor pressed the prefix key; <b> marks the key.',
	),
	panelPlayFull: message(
		'website.shell.panelPlay.full',
		'Notice when a split is refused because the window holds the most Panels it shows.',
	),
	panelPlaySmall: message(
		'website.shell.panelPlay.small',
		'Notice when a split is refused because the focused Panel is too small in that direction.',
	),
	panelPlayLast: message(
		'website.shell.panelPlay.last',
		'Notice when closing is refused because only one Panel is left.',
	),
	panelPlayOne: message(
		'website.shell.panelPlay.one',
		'Notice when maximizing is refused because the window has a single Panel.',
	),
	panelPlayNone: message(
		'website.shell.panelPlay.none',
		'Notice when a resize is refused because no divider lies in that direction.',
	),
});
