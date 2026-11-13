import { defineMessages } from 'react-intl';

import { enUSWebsiteMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteMessages);

export const websiteMessages = defineMessages({
	metaTitle: message(
		'website.metaTitle',
		'Document title and search result title of the home page.',
	),
	metaDescription: message('website.metaDescription', 'Search and link-preview description.'),
	notFoundMetaTitle: message('website.notFoundMetaTitle', 'Document title of the 404 page.'),
	skip: message('website.skip', 'Keyboard link that jumps past the header.'),
	navigation: message('website.navigation', 'Accessible name of the header navigation.'),
	footerNavigation: message('website.footerNavigation', 'Accessible name of the footer links.'),
	languageSwitch: message(
		'website.languageSwitch',
		'Header link to the same page in the other language, written in that language.',
	),
	languageSwitchLabel: message(
		'website.languageSwitchLabel',
		'Accessible name of that link, written in the language it switches to.',
	),
	themeSystem: message(
		'website.themeSystem',
		'Name of the theme button while the page follows the system appearance.',
	),
	themeLight: message(
		'website.themeLight',
		'Name of the theme button while the light theme is on.',
	),
	themeDark: message('website.themeDark', 'Name of the theme button while the dark theme is on.'),
	docs: message('website.docs', 'Header and footer link to the project documentation.'),
	navInstall: message(
		'website.navInstall',
		'Header button to the install section while no release exists.',
	),
	navDownload: message(
		'website.navDownload',
		'Header button to the install section once a release exists.',
	),
	sceneOf: message(
		'website.sceneOf',
		'Total shown after the current section number in the header chip.',
	),
	sceneAnywhere: message(
		'website.sceneAnywhere',
		'Label of the section about attaching every client to one session.',
	),
	sceneAgents: message('website.sceneAgents', 'Label of the section about Agent status.'),
	sceneOrchestrate: message(
		'website.sceneOrchestrate',
		'Label of the section about the command line driving Agents.',
	),
	scenePrivate: message('website.scenePrivate', 'Label of the privacy section.'),
	sceneTerminal: message('website.sceneTerminal', 'Label of the section about terminal emulation.'),
	sceneInstall: message(
		'website.sceneInstall',
		'Label of the install section and its footer link.',
	),
	liveWorking: message('website.liveWorking', 'Hero status pill while the demo session runs.'),
	liveClosed: message('website.liveClosed', 'Hero status pill after the demo window closes.'),
	heroTitleLead: message('website.heroTitleLead', 'First line of the page heading.'),
	heroTitleAccent: message(
		'website.heroTitleAccent',
		'Second, highlighted line of the page heading.',
	),
	heroLead: message('website.heroLead', 'Paragraph under the page heading.'),
	getSource: message(
		'website.getSource',
		'Primary button while no release exists; it leads to the build steps.',
	),
	downloadForWindows: message(
		'website.downloadForWindows',
		'Primary button once a Windows release exists.',
	),
	copy: message('website.copy', 'Copy button text.'),
	copied: message('website.copied', 'Copy button text after copying.'),
	copyCommand: message(
		'website.copyCommand',
		'Accessible name of a copy button next to a command.',
	),
	platformsNow: message('website.platformsNow', 'Platforms Orchbee runs on today.'),
	platformsNext: message('website.platformsNext', 'Platforms still in development.'),
	tryClose: message(
		'website.tryClose',
		'Hint above the demo window inviting a click on its close button.',
	),
	closeWindow: message('website.closeWindow', 'Accessible name of the demo window close button.'),
	heroAlt: message(
		'website.heroAlt',
		'Description of the terminal interface screenshot in the hero.',
	),
	hiveTitle: message('website.hiveTitle', 'Heading shown after the demo window closes.'),
	closedFor: message(
		'website.closedFor',
		'How long the demo window has been closed, as minutes:seconds.',
	),
	reopenWindow: message('website.reopenWindow', 'Button that reopens the demo window.'),
	replay: message('website.replay', 'Button that restarts the demo.'),
	pauseRecording: message(
		'website.pauseRecording',
		'Accessible name of the button that pauses a screen recording.',
	),
	playRecording: message(
		'website.playRecording',
		'Accessible name of the button that plays a paused screen recording.',
	),
	anywhereTitleLead: message('website.anywhereTitleLead', 'First line of the Anywhere heading.'),
	anywhereTitleAccent: message(
		'website.anywhereTitleAccent',
		'Second line of the Anywhere heading.',
	),
	anywhereBody: message('website.anywhereBody', 'Paragraph of the Anywhere section.'),
	multiplexAlt: message(
		'website.multiplexAlt',
		'Description of the recording of one session in the terminal interface and the desktop app.',
	),
	multiplexTuiAlt: message(
		'website.multiplexTuiAlt',
		'Description of the terminal interface half of that recording, shown alone on narrow screens.',
	),
	multiplexDesktopAlt: message(
		'website.multiplexDesktopAlt',
		'Description of the desktop app half of that recording, shown alone on narrow screens.',
	),
	captionDesktop: message('website.captionDesktop', 'Caption of the desktop app picture.'),
	captionTerminal: message('website.captionTerminal', 'Caption of the terminal picture.'),
	tagInDevelopment: message('website.tagInDevelopment', 'Tag on a feature under development.'),
	agentsTitleLead: message('website.agentsTitleLead', 'First line of the Agents heading.'),
	agentsTitleAccent: message('website.agentsTitleAccent', 'Second line of the Agents heading.'),
	agentsBody: message('website.agentsBody', 'Paragraph of the Agents section.'),
	desktopAlt: message('website.desktopAlt', 'Description of the desktop app screenshot.'),
	sidebarAlt: message(
		'website.sidebarAlt',
		'Description of the screenshot of the terminal interface sidebar.',
	),
	stateNeedsYou: message(
		'website.stateNeedsYou',
		'Legend name and hero state of the needs-attention status.',
	),
	stateWorking: message(
		'website.stateWorking',
		'Legend name and hero state of the working status.',
	),
	stateDone: message('website.stateDone', 'Legend name and hero state of the done status.'),
	stateFailed: message('website.stateFailed', 'Legend name and hero state of the failed status.'),
	stateIdle: message('website.stateIdle', 'Legend name and hero state of the idle status.'),
	stateUnread: message('website.stateUnread', 'Legend name of the unread mark.'),
	chimeLine: message('website.chimeLine', 'What the Agent reminder and its sound do.'),
	chimeDefault: message('website.chimeDefault', 'Default of the Agent reminder setting.'),
	kindsLine: message(
		'website.kindsLine',
		'Line above the list of Agents Orchbee can start; <code> marks the command.',
	),
	orchestrateTitleLead: message(
		'website.orchestrateTitleLead',
		'First line of the Orchestrate heading.',
	),
	orchestrateTitleAccent: message(
		'website.orchestrateTitleAccent',
		'Second line of the Orchestrate heading.',
	),
	orchestrateBody: message('website.orchestrateBody', 'Paragraph of the Orchestrate section.'),
	agentsCliAlt: message(
		'website.agentsCliAlt',
		'Description of the recording of orchbee agent commands run from a shell Panel.',
	),
	orchestrateNext: message(
		'website.orchestrateNext',
		'Feature in development, shown under the Orchestrate recording.',
	),
	privateTitleLead: message('website.privateTitleLead', 'First line of the privacy heading.'),
	privateTitleAccent: message('website.privateTitleAccent', 'Second line of the privacy heading.'),
	noAccountTitle: message('website.noAccountTitle', 'Privacy fact heading.'),
	noAccountBody: message('website.noAccountBody', 'Privacy fact text.'),
	noTelemetryTitle: message('website.noTelemetryTitle', 'Privacy fact heading.'),
	noTelemetryBody: message('website.noTelemetryBody', 'Privacy fact text.'),
	offlineTitle: message('website.offlineTitle', 'Privacy fact heading.'),
	offlineBody: message('website.offlineBody', 'Privacy fact text.'),
	terminalTitleLead: message('website.terminalTitleLead', 'First line of the Terminal heading.'),
	terminalTitleAccent: message(
		'website.terminalTitleAccent',
		'Second line of the Terminal heading.',
	),
	terminalBody: message('website.terminalBody', 'Paragraph of the Terminal section.'),
	chipGraphics: message('website.chipGraphics', 'Terminal capability.'),
	chipKeyboard: message('website.chipKeyboard', 'Terminal capability.'),
	chipClipboard: message('website.chipClipboard', 'Terminal capability.'),
	chipMouse: message('website.chipMouse', 'Terminal capability.'),
	chipPaste: message('website.chipPaste', 'Terminal capability.'),
	chipFocus: message('website.chipFocus', 'Terminal capability.'),
	chipColor: message('website.chipColor', 'Terminal capability.'),
	chipGpu: message('website.chipGpu', 'Terminal capability.'),
	mouseTitle: message('website.mouseTitle', 'Terminal point heading.'),
	mouseBody: message('website.mouseBody', 'Terminal point text; <kbd> marks the key.'),
	learnTitle: message('website.learnTitle', 'Terminal point heading.'),
	learnBody: message('website.learnBody', 'Terminal point text.'),
	whereTitle: message('website.whereTitle', 'Terminal point heading.'),
	whereBody: message('website.whereBody', 'Terminal point text.'),
	installTitleLeadReleased: message(
		'website.installTitleLeadReleased',
		'First line of the install heading once a release exists.',
	),
	installTitleAccentReleased: message(
		'website.installTitleAccentReleased',
		'Second line of the install heading once a release exists.',
	),
	installTitleLeadSource: message(
		'website.installTitleLeadSource',
		'First line of the install heading while no release exists.',
	),
	installTitleAccentSource: message(
		'website.installTitleAccentSource',
		'Second line of the install heading while no release exists.',
	),
	installBodyReleased: message(
		'website.installBodyReleased',
		'Install paragraph once a release exists.',
	),
	installBodySource: message(
		'website.installBodySource',
		'Install paragraph while no release exists.',
	),
	desktopApp: message('website.desktopApp', 'Heading of the desktop app card.'),
	buildGuide: message('website.buildGuide', 'Link to the build instructions in the repository.'),
	requirementsSource: message('website.requirementsSource', 'Build requirements.'),
	requirementsReleased: message(
		'website.requirementsReleased',
		'System requirements of the released app.',
	),
	releaseNotes: message('website.releaseNotes', 'Link to the notes of the current release.'),
	operatingSystem: message(
		'website.operatingSystem',
		'Accessible name of the operating system tabs.',
	),
	commentInstall: message('website.commentInstall', 'Shell comment above the install command.'),
	commentOpen: message(
		'website.commentOpen',
		'Shell comment above the command that opens Orchbee.',
	),
	commentStartAgent: message(
		'website.commentStartAgent',
		'Shell comment above the command that starts an Agent.',
	),
	commentSource: message('website.commentSource', 'Shell comment above the source commands.'),
	commentDesktop: message(
		'website.commentDesktop',
		'Shell comment above the command that starts the desktop app.',
	),
	commentInProgress: message(
		'website.commentInProgress',
		'Shell comment on a tab for a platform still in development.',
	),
	githubLink: message('website.githubLink', 'Link to the repository.'),
	outroTitleLead: message('website.outroTitleLead', 'First line of the closing heading.'),
	outroTitleAccent: message('website.outroTitleAccent', 'Second line of the closing heading.'),
	tagline: message('website.tagline', 'Footer tagline.'),
	footerPrivacy: message('website.footerPrivacy', 'Footer note.'),
	notFoundTitle: message('website.notFoundTitle', 'Heading of the 404 page.'),
	notFoundBody: message('website.notFoundBody', 'Text of the 404 page.'),
	notFoundHome: message('website.notFoundHome', 'Link from the 404 page to the home page.'),
});
