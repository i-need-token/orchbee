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
	downloadForLinux: message(
		'website.shell.downloadForLinux',
		'Button that downloads the Linux AppImage for x86_64 processors.',
	),
	downloadForLinuxDeb: message(
		'website.shell.downloadForLinuxDeb',
		'Link that downloads the Debian package for Linux on x86_64 processors.',
	),
	downloadForLinuxArmAppImage: message(
		'website.shell.downloadForLinuxArmAppImage',
		'Link that downloads the Linux AppImage for ARM64 processors.',
	),
	downloadForLinuxArmDeb: message(
		'website.shell.downloadForLinuxArmDeb',
		'Link that downloads the Debian package for Linux on ARM64 processors.',
	),
	platformWindows: message(
		'website.shell.platform.windows',
		'Name of the Windows platform: the heading over its installers in the download dialog.',
	),
	platformMacos: message(
		'website.shell.platform.macos',
		'Name of the macOS platform: the heading over its disk images in the download dialog.',
	),
	platformLinux: message(
		'website.shell.platform.linux',
		'Name of the Linux platform: the heading over its AppImages and Debian packages in the download dialog.',
	),
	downloadAction: message(
		'website.shell.download.action',
		'Button in the download dialog that downloads the only installer of a platform.',
	),
	downloadAppleSilicon: message(
		'website.shell.download.appleSilicon',
		'Button in the download dialog that downloads the macOS disk image for Macs with Apple silicon.',
	),
	downloadIntel: message(
		'website.shell.download.intel',
		'Button in the download dialog that downloads the macOS disk image for Macs with an Intel processor.',
	),
	downloadAppImageX64: message(
		'website.shell.download.appImageX64',
		'Button in the download dialog that downloads the Linux AppImage for x86_64 processors.',
	),
	downloadDebX64: message(
		'website.shell.download.debX64',
		'Button in the download dialog that downloads the Debian package for x86_64 processors.',
	),
	downloadAppImageArm: message(
		'website.shell.download.appImageArm',
		'Button in the download dialog that downloads the Linux AppImage for ARM64 processors.',
	),
	downloadDebArm: message(
		'website.shell.download.debArm',
		'Button in the download dialog that downloads the Debian package for ARM64 processors.',
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
	downloadScriptDesktopLinux: message(
		'website.shell.download.scriptDesktopLinux',
		'Label above the Linux install script command that installs the desktop app, terminal interface, and CLI.',
	),
	downloadScriptCliLinux: message(
		'website.shell.download.scriptCliLinux',
		'Label above the Linux install script command that installs only the terminal interface and CLI.',
	),
	downloadScriptDesktopShared: message(
		'website.shell.download.scriptDesktopShared',
		'Label above the install script command that installs the desktop app, terminal interface, and CLI, when a release offers it for both macOS and Linux.',
	),
	downloadScriptCliShared: message(
		'website.shell.download.scriptCliShared',
		'Label above the install script command that installs only the terminal interface and CLI, when a release offers it for both macOS and Linux.',
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
	installWindowsTitle: message(
		'website.shell.install.windows.title',
		'What the Windows installer contains, beside the macOS rows.',
	),
	installWindowsDetail: message(
		'website.shell.install.windows.detail',
		'What the Windows installer does; <code> marks a command and a variable name.',
	),
	installWindowsNote: message(
		'website.shell.install.windows.note',
		'Warning under the install rows about the unsigned installer.',
	),
	installMacosTitle: message(
		'website.shell.install.macos.title',
		'Row of the macOS disk images: what they contain.',
	),
	installMacosDetail: message(
		'website.shell.install.macos.detail',
		'How to install from the macOS disk image.',
	),
	installMacosScriptTitle: message(
		'website.shell.install.macos.scriptTitle',
		'Row of the install script command for macOS.',
	),
	installMacosScriptDetail: message(
		'website.shell.install.macos.scriptDetail',
		'What the two install script commands do.',
	),
	installScriptDesktop: message(
		'website.shell.install.scriptDesktop',
		'Label above the install script command that installs the desktop app, terminal interface, and CLI, under the macOS or the Linux tab.',
	),
	installScriptCli: message(
		'website.shell.install.scriptCli',
		'Label above the install script command that installs only the terminal interface and CLI, under the macOS or the Linux tab.',
	),
	installDownloadAppleSilicon: message(
		'website.shell.install.downloadAppleSilicon',
		'Button that downloads the disk image for Macs with Apple silicon.',
	),
	installDownloadIntel: message(
		'website.shell.install.downloadIntel',
		'Button that downloads the disk image for Macs with an Intel processor.',
	),
	installLinuxTitle: message(
		'website.shell.install.linux.title',
		'Row of the Linux AppImages and Debian packages: what they contain.',
	),
	installLinuxDetail: message(
		'website.shell.install.linux.detail',
		'How to install from the Linux AppImage or Debian package.',
	),
	installLinuxScriptTitle: message(
		'website.shell.install.linux.scriptTitle',
		'Row of the install script command for Linux.',
	),
	installLinuxScriptDetail: message(
		'website.shell.install.linux.scriptDetail',
		'What the two install script commands do on Linux.',
	),
	installDownloadAppImageX64: message(
		'website.shell.install.downloadAppImageX64',
		'Button that downloads the Linux AppImage for x86_64 processors.',
	),
	installDownloadDebX64: message(
		'website.shell.install.downloadDebX64',
		'Button that downloads the Debian package for x86_64 processors.',
	),
	installDownloadAppImageArm: message(
		'website.shell.install.downloadAppImageArm',
		'Button that downloads the Linux AppImage for ARM64 processors.',
	),
	installDownloadDebArm: message(
		'website.shell.install.downloadDebArm',
		'Button that downloads the Debian package for ARM64 processors.',
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
