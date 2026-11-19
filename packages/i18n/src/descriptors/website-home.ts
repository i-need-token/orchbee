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
	statement: message(
		'website.home.statement',
		'Large paragraph under the home stage; <quiet> marks the second sentence, set in a quieter color.',
	),
	sheetClients: message(
		'website.home.sheet.clients',
		'Lowercase label of the data sheet row that lists the clients.',
	),
	sheetClientsValue: message(
		'website.home.sheet.clientsValue',
		'Value of the clients row of the data sheet.',
	),
	sheetAgents: message(
		'website.home.sheet.agents',
		'Lowercase label of the data sheet row about agents.',
	),
	sheetAgentsValue: message(
		'website.home.sheet.agentsValue',
		'Value of the agents row of the data sheet.',
	),
	sheetTerminal: message(
		'website.home.sheet.terminal',
		'Lowercase label of the data sheet row about the terminal.',
	),
	sheetTerminalValue: message(
		'website.home.sheet.terminalValue',
		'Value of the terminal row of the data sheet.',
	),
	sheetPlatform: message(
		'website.home.sheet.platform',
		'Lowercase label of the data sheet row about platforms.',
	),
	sheetPlatformValue: message(
		'website.home.sheet.platformValue',
		'Value of the platform row of the data sheet.',
	),
	sheetPlatformValueMacos: message(
		'website.home.sheet.platformValueMacos',
		'Value of the platform row of the data sheet once a release offers macOS downloads.',
	),
	sheetLicense: message(
		'website.home.sheet.license',
		'Lowercase label of the data sheet row that names the license.',
	),
	sheetSource: message(
		'website.home.sheet.source',
		'Lowercase label of the data sheet row that links the source repository.',
	),
	multiplexingKicker: message(
		'website.home.multiplexing.kicker',
		'Lowercase label above the multiplexing heading.',
	),
	multiplexingTitle: message(
		'website.home.multiplexing.title',
		'Heading of the multiplexing section.',
	),
	multiplexingLead: message(
		'website.home.multiplexing.lead',
		'Paragraph beside the multiplexing heading.',
	),
	multiplexingInput: message(
		'website.home.multiplexing.input',
		'Label of the field where the visitor types a command into the drawn shell.',
	),
	multiplexingSession: message(
		'website.home.multiplexing.session',
		'Label on the line that joins the two drawn clients: both show the same Session.',
	),
	multiplexingReader: message(
		'website.home.multiplexing.reader',
		'Note after the client name above the drawn console that reads the same shell.',
	),
	collaborationKicker: message(
		'website.home.collaboration.kicker',
		'Lowercase label above the collaboration heading.',
	),
	collaborationTitle: message(
		'website.home.collaboration.title',
		'Heading of the collaboration section.',
	),
	collaborationLead: message(
		'website.home.collaboration.lead',
		'Paragraph beside the collaboration heading.',
	),
	collaborationWindow: message(
		'website.home.collaboration.window',
		'Accessible name of the drawn terminal interface where one agent directs another.',
	),
	collaborationCaption: message(
		'website.home.collaboration.caption',
		'Caption under the collaboration figure; <b> marks its label, <code> a command name.',
	),
	collaborationCaptionStacked: message(
		'website.home.collaboration.captionStacked',
		'Caption under the collaboration figure while its Panels stand one above the other; <b> marks its label, <code> a command name.',
	),
	collaborationStep1: message(
		'website.home.collaboration.step1',
		'What the first command of the collaboration figure does.',
	),
	collaborationStep2: message(
		'website.home.collaboration.step2',
		'What the second command does; <code> marks an agent handle.',
	),
	collaborationStep3: message('website.home.collaboration.step3', 'What the third command does.'),
	collaborationStep4: message('website.home.collaboration.step4', 'What the fourth command does.'),
	collaborationStep5: message('website.home.collaboration.step5', 'What the fifth command does.'),
	collaborationNote: message(
		'website.home.collaboration.note',
		'Note under the collaboration figure; <b> marks the two named roles.',
	),
	agentsKicker: message('website.home.agents.kicker', 'Lowercase label above the agents heading.'),
	agentsTitle: message('website.home.agents.title', 'Heading of the agents section.'),
	agentsLead: message('website.home.agents.lead', 'Paragraph beside the agents heading.'),
	agentsRecognized: message(
		'website.home.agents.recognized',
		'Label above the list of agent CLIs Orchbee recognizes; shown in lowercase.',
	),
	agentsSidebar: message(
		'website.home.agents.sidebar',
		'Paragraph beside the drawn sidebars, above their numbered notes.',
	),
	agentsProjectTitle: message(
		'website.home.agents.projectTitle',
		'Title of the note on the Project row of the drawn sidebar.',
	),
	agentsProject: message('website.home.agents.project', 'What a Project row is.'),
	agentsWorkspaceTitle: message(
		'website.home.agents.workspaceTitle',
		'Title of the note on a Workspace row of the drawn sidebar.',
	),
	agentsWorkspace: message('website.home.agents.workspace', 'What a Workspace row is.'),
	agentsAgentTitle: message(
		'website.home.agents.agentTitle',
		'Title of the note on an Agent row of the drawn sidebar.',
	),
	agentsAgent: message('website.home.agents.agent', 'What an Agent row shows.'),
	agentsUnreadTitle: message(
		'website.home.agents.unreadTitle',
		'Title of the note on the unread dot of the drawn sidebar.',
	),
	agentsUnread: message('website.home.agents.unread', 'What the unread dot means.'),
	terminalKicker: message(
		'website.home.terminal.kicker',
		'Lowercase label above the terminal section heading.',
	),
	terminalTitle: message('website.home.terminal.title', 'Heading of the terminal section.'),
	terminalLead: message(
		'website.home.terminal.lead',
		'Paragraph beside the terminal section heading.',
	),
	terminalThemes: message(
		'website.home.terminal.themes',
		'Line above the list of terminal themes; count is how many themes the product ships.',
	),
	terminalDark: message(
		'website.home.terminal.dark',
		'Heading of the list of dark terminal themes.',
	),
	terminalLight: message(
		'website.home.terminal.light',
		'Heading of the list of light terminal themes.',
	),
	terminalFact1: message(
		'website.home.terminal.fact1',
		'Terminal fact: programs can use 24-bit color.',
	),
	terminalFact2: message('website.home.terminal.fact2', 'Terminal fact: font ligatures are drawn.'),
	terminalFact3: message(
		'website.home.terminal.fact3',
		'Terminal fact: images of the Kitty graphics protocol are drawn.',
	),
	terminalFact4: message(
		'website.home.terminal.fact4',
		'Terminal fact: links in terminal output open.',
	),
	terminalFact5: message(
		'website.home.terminal.fact5',
		'Terminal fact: input methods compose text in place.',
	),
	terminalFact6: message(
		'website.home.terminal.fact6',
		'Terminal fact: a theme is chosen for the light and for the dark appearance separately.',
	),
	remoteTag: message(
		'website.home.remote.tag',
		'Tag on the remote machines band: the feature is not released yet.',
	),
	remoteTitle: message('website.home.remote.title', 'Heading of the remote machines band.'),
	remoteLead: message('website.home.remote.lead', 'Paragraph of the remote machines band.'),
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
	installLeadMacos: message(
		'website.home.install.leadMacos',
		'Paragraph under the install heading once a release offers macOS downloads.',
	),
	installAppWindows: message(
		'website.home.install.appWindows',
		'What the Windows installer contains, beside the macOS rows.',
	),
	installMacosApp: message(
		'website.home.install.macosApp',
		'Row of the macOS disk images: what they contain.',
	),
	installMacosAppNote: message(
		'website.home.install.macosAppNote',
		'How to install from the macOS disk image.',
	),
	installDownloadAppleSilicon: message(
		'website.home.install.downloadAppleSilicon',
		'Button that downloads the disk image for Macs with Apple silicon.',
	),
	installDownloadIntel: message(
		'website.home.install.downloadIntel',
		'Button that downloads the disk image for Macs with an Intel processor.',
	),
	installMacosScript: message(
		'website.home.install.macosScript',
		'Row of the install script command for macOS.',
	),
	installMacosScriptNote: message(
		'website.home.install.macosScriptNote',
		'What the two install script commands do.',
	),
	installMacosScriptDesktop: message(
		'website.home.install.macosScriptDesktop',
		'Label above the macOS install script command that installs the desktop app, terminal interface, and CLI.',
	),
	installMacosScriptCli: message(
		'website.home.install.macosScriptCli',
		'Label above the macOS install script command that installs only the terminal interface and CLI.',
	),
	installPlatform: message(
		'website.home.install.platform',
		'Accessible label for the platform switcher in the install section.',
	),
	installTabWindows: message(
		'website.home.install.tabWindows',
		'Tab label for Windows installation instructions.',
	),
	installTabMacos: message(
		'website.home.install.tabMacos',
		'Tab label for macOS installation instructions.',
	),
	installTabNpm: message(
		'website.home.install.tabNpm',
		'Tab label for npm CLI-only installation instructions.',
	),
	installStartNoteMacos: message(
		'website.home.install.startNoteMacos',
		'What the start command opens, once a release offers macOS downloads.',
	),
	installMacosNote: message(
		'website.home.install.macosNote',
		'Steps under the install rows to open the macOS app that Apple has not notarized.',
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
