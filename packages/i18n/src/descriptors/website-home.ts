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
	beta: message('website.home.beta', 'Label above the headline: the beta status of the product.'),
	facts: message(
		'website.home.facts',
		'Line under the Download button while only Windows is published; <repo> marks the words that link to the source repository.',
	),
	factsMacos: message(
		'website.home.factsMacos',
		'Line under the Download button once a release offers macOS; <repo> marks the words that link to the source repository.',
	),
	factsLinux: message(
		'website.home.factsLinux',
		'Line under the Download button once a release offers Linux but not macOS; <repo> marks the words that link to the source repository.',
	),
	factsAll: message(
		'website.home.factsAll',
		'Line under the Download button once a release offers macOS and Linux; <repo> marks the words that link to the source repository.',
	),
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
	benefitsKicker: message(
		'website.home.benefits.kicker',
		'Lowercase label above the heading of the section that sums up why to use Orchbee.',
	),
	benefitsTitle: message(
		'website.home.benefits.title',
		'Heading of the section that sums up why to use Orchbee.',
	),
	benefitsLead: message(
		'website.home.benefits.lead',
		'Paragraph beside the benefits heading; <b> marks the two named roles.',
	),
	benefitsStatusTitle: message(
		'website.home.benefits.statusTitle',
		'Title of the benefit card about seeing the state of every agent.',
	),
	benefitsStatusBody: message(
		'website.home.benefits.statusBody',
		'Text of the benefit card about seeing the state of every agent.',
	),
	benefitsPersistTitle: message(
		'website.home.benefits.persistTitle',
		'Title of the benefit card about Sessions that keep running after the window closes.',
	),
	benefitsPersistBody: message(
		'website.home.benefits.persistBody',
		'Text of the benefit card about Sessions that keep running after the window closes.',
	),
	benefitsDelegateTitle: message(
		'website.home.benefits.delegateTitle',
		'Title of the benefit card about agents that start and drive other agents.',
	),
	benefitsDelegateBody: message(
		'website.home.benefits.delegateBody',
		'Text of the benefit card about agents that start and drive other agents; <code> marks the command name.',
	),
	benefitsTerminalTitle: message(
		'website.home.benefits.terminalTitle',
		'Title of the benefit card about the terminal emulation.',
	),
	benefitsTerminalBody: message(
		'website.home.benefits.terminalBody',
		'Text of the benefit card about the terminal emulation.',
	),
	agentsKicker: message('website.home.agents.kicker', 'Lowercase label above the agents heading.'),
	agentsTitle: message('website.home.agents.title', 'Heading of the agents section.'),
	agentsLead: message('website.home.agents.lead', 'Paragraph beside the agents heading.'),
	agentsAll: message(
		'website.home.agents.all',
		'Toggle that shows every agent CLI Orchbee recognizes; count is how many there are.',
	),
	clientsKicker: message(
		'website.home.clients.kicker',
		'Lowercase label above the client index heading.',
	),
	clientsTitle: message('website.home.clients.title', 'Heading of the client index.'),
	clientsLead: message('website.home.clients.lead', 'Paragraph beside the client index heading.'),
	faqKicker: message('website.home.faq.kicker', 'Lowercase label above the questions heading.'),
	faqTitle: message('website.home.faq.title', 'Heading of the frequently asked questions.'),
	faqFreeQuestion: message(
		'website.home.faq.freeQuestion',
		'Question: whether Orchbee costs money.',
	),
	faqFreeAnswer: message(
		'website.home.faq.freeAnswer',
		'Answer to whether Orchbee costs money; <repo> marks the words that link to the source repository.',
	),
	faqReplaceQuestion: message(
		'website.home.faq.replaceQuestion',
		'Question: whether Orchbee replaces the agent CLIs people already use.',
	),
	faqReplaceAnswer: message(
		'website.home.faq.replaceAnswer',
		'Answer to whether Orchbee replaces the agent CLIs people already use.',
	),
	faqTmuxQuestion: message(
		'website.home.faq.tmuxQuestion',
		'Question: how Orchbee differs from the terminal multiplexer tmux.',
	),
	faqTmuxAnswer: message(
		'website.home.faq.tmuxAnswer',
		'Answer to how Orchbee differs from the terminal multiplexer tmux.',
	),
	faqPlatformsQuestion: message(
		'website.home.faq.platformsQuestion',
		'Question: which operating systems Orchbee runs on.',
	),
	faqPlatformsAnswer: message(
		'website.home.faq.platformsAnswer',
		'Answer to which operating systems Orchbee runs on, while only Windows is published.',
	),
	faqPlatformsAnswerMacos: message(
		'website.home.faq.platformsAnswerMacos',
		'Answer to which operating systems Orchbee runs on, once a release offers macOS.',
	),
	faqPlatformsAnswerLinux: message(
		'website.home.faq.platformsAnswerLinux',
		'Answer to which operating systems Orchbee runs on, once a release offers Linux but not macOS.',
	),
	faqPlatformsAnswerAll: message(
		'website.home.faq.platformsAnswerAll',
		'Answer to which operating systems Orchbee runs on, once a release offers macOS and Linux.',
	),
	faqDataQuestion: message(
		'website.home.faq.dataQuestion',
		'Question: whether Orchbee collects data about its users.',
	),
	faqDataAnswer: message(
		'website.home.faq.dataAnswer',
		'Answer to whether Orchbee collects data: it has no telemetry of any kind.',
	),
	faqNextQuestion: message('website.home.faq.nextQuestion', 'Question: what is being built next.'),
	faqNextAnswer: message(
		'website.home.faq.nextAnswer',
		'Answer to what is being built next: remote machines, still in development.',
	),
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
	installApp: message(
		'website.home.install.app',
		'Install section title when one platform is published; it names no platform.',
	),
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
	installLeadMacos: message(
		'website.home.install.leadMacos',
		'Paragraph under the install heading once a release offers macOS downloads.',
	),
	installLeadLinux: message(
		'website.home.install.leadLinux',
		'Paragraph under the install heading once a release offers Linux downloads but no macOS ones.',
	),
	installLeadAll: message(
		'website.home.install.leadAll',
		'Paragraph under the install heading once a release offers macOS and Linux downloads.',
	),
	installPlatform: message(
		'website.home.install.platform',
		'Accessible label for the platform switcher in the install section.',
	),
	installTabNpm: message(
		'website.home.install.tabNpm',
		'Tab label for npm CLI-only installation instructions.',
	),
	installStartNoteMacos: message(
		'website.home.install.startNoteMacos',
		'What the start command opens, once a release offers macOS downloads.',
	),
	installStartNoteLinux: message(
		'website.home.install.startNoteLinux',
		'What the start command opens, once a release offers Linux downloads but no macOS ones.',
	),
	installStartNoteAll: message(
		'website.home.install.startNoteAll',
		'What the start command opens, once a release offers macOS and Linux downloads.',
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
