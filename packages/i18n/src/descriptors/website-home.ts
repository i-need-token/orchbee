import { defineMessages } from 'react-intl';

import { enUSWebsiteHomeMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteHomeMessages);

export const websiteHomeMessages = defineMessages({
	metaTitle: message(
		'website.home.metaTitle',
		'Document title and search result title of the home page.',
	),
	metaDescription: message(
		'website.home.metaDescription',
		'Search and link-preview description of the home page.',
	),
	title: message('website.home.title', 'Headline of the home page.'),
	lead: message('website.home.lead', 'Paragraph beside the headline.'),
	beta: message('website.home.beta', 'Line under the download button.'),
	stageView: message(
		'website.home.stage.view',
		'Label before the buttons that switch the drawn Session between the clients.',
	),
	stageCloseHint: message(
		'website.home.stage.closeHint',
		'Note beside the button that closes the drawn client window.',
	),
	stageClose: message('website.home.stage.close', 'Button that closes the drawn client window.'),
	stageAwayTitle: message(
		'website.home.stage.awayTitle',
		'Heading shown where the drawn window was, after it is closed.',
	),
	stageAwayBody: message(
		'website.home.stage.awayBody',
		'Paragraph under the closed-window heading.',
	),
	stageClosedFor: message(
		'website.home.stage.closedFor',
		'Label before the minutes and seconds since the drawn window was closed.',
	),
	stageReattach: message(
		'website.home.stage.reattach',
		'Button that opens the drawn client window again.',
	),
	stageCaption: message(
		'website.home.stage.caption',
		'Caption under the drawn Session; <b> marks the word that labels it a demo.',
	),
	clientsKicker: message(
		'website.home.clients.kicker',
		'Lowercase label above the client index heading.',
	),
	clientsTitle: message('website.home.clients.title', 'Heading of the client index.'),
	clientsLead: message('website.home.clients.lead', 'Paragraph beside the client index heading.'),
	installKicker: message(
		'website.home.install.kicker',
		'Lowercase label above the install heading.',
	),
	installTitle: message('website.home.install.title', 'Heading of the install section.'),
	installLead: message('website.home.install.lead', 'Paragraph under the install heading.'),
	installReleaseNotes: message(
		'website.home.install.releaseNotes',
		'Link to the notes of the published release.',
	),
	installApp: message('website.home.install.app', 'What the Windows installer contains.'),
	installNpm: message('website.home.install.npm', 'What the npm package contains.'),
	installNpmNote: message(
		'website.home.install.npmNote',
		'Requirement and contents of the npm package.',
	),
	installStart: message(
		'website.home.install.start',
		'Row that shows how to start Orchbee after installing.',
	),
	installStartNote: message('website.home.install.startNote', 'What the start command opens.'),
	installNote: message(
		'website.home.install.note',
		'Warning under the install rows about the unsigned installer.',
	),
	installSource: message(
		'website.home.install.source',
		'Row of the source build shown while no release is published.',
	),
	installSourceRequirements: message(
		'website.home.install.sourceRequirements',
		'What the source build needs.',
	),
	installSourceStart: message(
		'website.home.install.sourceStart',
		'Row that shows how to start the desktop app from the source build.',
	),
	installSourceNote: message(
		'website.home.install.sourceNote',
		'Note under the source build rows.',
	),
});
