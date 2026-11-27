import { defineMessages } from 'react-intl';

import { enUSWebsiteCliMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteCliMessages);

export const websiteCliMessages = defineMessages({
	metaTitle: message('website.cli.metaTitle', 'Document title of the command line page.'),
	metaDescription: message(
		'website.cli.metaDescription',
		'Search and link-preview description of the command line page.',
	),
	lead: message(
		'website.cli.lead',
		'Paragraph beside the page headline; <code> marks the command name.',
	),
	stageCaption: message(
		'website.cli.stage.caption',
		'Caption under the drawn console that asks the Session about its agents; <b> marks the word that labels it a demo.',
	),
	collaborationKicker: message(
		'website.cli.collaboration.kicker',
		'Lowercase label above the section where one agent directs another.',
	),
	collaborationTitle: message(
		'website.cli.collaboration.title',
		'Heading of the section where one agent directs another.',
	),
	collaborationLead: message(
		'website.cli.collaboration.lead',
		'Paragraph under the collaboration heading.',
	),
	collaborationWindow: message(
		'website.cli.collaboration.window',
		'Accessible name of the drawn terminal interface where one agent directs another.',
	),
	collaborationCaption: message(
		'website.cli.collaboration.caption',
		'Caption under the collaboration figure; <b> marks its label, <code> a command name.',
	),
	collaborationCaptionStacked: message(
		'website.cli.collaboration.captionStacked',
		'Caption under the collaboration figure while its Panels stand one above the other; <b> marks its label, <code> a command name.',
	),
	collaborationStep1: message(
		'website.cli.collaboration.step1',
		'What the first command of the collaboration figure does.',
	),
	collaborationStep2: message(
		'website.cli.collaboration.step2',
		'What the second command does; <code> marks an agent handle.',
	),
	collaborationStep3: message('website.cli.collaboration.step3', 'What the third command does.'),
	collaborationStep4: message('website.cli.collaboration.step4', 'What the fourth command does.'),
	collaborationStep5: message('website.cli.collaboration.step5', 'What the fifth command does.'),
	grammarKicker: message(
		'website.cli.grammar.kicker',
		'Lowercase label above the section that takes one command line apart.',
	),
	grammarTitle: message(
		'website.cli.grammar.title',
		'Heading of the section that takes one command line apart.',
	),
	grammarLead: message('website.cli.grammar.lead', 'Paragraph under the grammar heading.'),
	grammarFamilyTitle: message(
		'website.cli.grammar.family.title',
		'Name of the numbered part of the command line that is the command family.',
	),
	grammarFamilyBody: message(
		'website.cli.grammar.family.body',
		'Note on the numbered part that is the command family; <code> marks a command word.',
	),
	grammarActionTitle: message(
		'website.cli.grammar.action.title',
		'Name of the numbered part of the command line that is the action.',
	),
	grammarActionBody: message(
		'website.cli.grammar.action.body',
		'Note on the numbered part that is the action; <code> marks a command word.',
	),
	grammarTargetTitle: message(
		'website.cli.grammar.target.title',
		'Name of the numbered part of the command line that is the target.',
	),
	grammarTargetBody: message(
		'website.cli.grammar.target.body',
		'Note on the numbered part that is the target; <code> marks a command word.',
	),
	grammarArgumentTitle: message(
		'website.cli.grammar.argument.title',
		'Name of the numbered part of the command line that is the argument.',
	),
	grammarArgumentBody: message(
		'website.cli.grammar.argument.body',
		'Note on the numbered part that is the argument; <code> marks a command word.',
	),
	grammarOptionTitle: message(
		'website.cli.grammar.option.title',
		'Name of the numbered part of the command line that is the option.',
	),
	grammarOptionBody: message(
		'website.cli.grammar.option.body',
		'Note on the numbered part that is the option; <code> marks a command word.',
	),
	outputKicker: message(
		'website.cli.output.kicker',
		'Lowercase label above the section that shows what commands print.',
	),
	outputTitle: message(
		'website.cli.output.title',
		'Heading of the section that shows what commands print.',
	),
	outputLead: message(
		'website.cli.output.lead',
		'Paragraph under the output heading, inviting the visitor to choose a command.',
	),
	outputList: message(
		'website.cli.output.list',
		'Accessible name of the list of commands the visitor chooses from.',
	),
	outputNote: message(
		'website.cli.output.note',
		'Line under the output figure saying the JSON document is shortened.',
	),
	highlightsTitle: message('website.cli.highlights.title', 'Heading of the highlights.'),
	highlightHandlesTitle: message(
		'website.cli.highlight.handles.title',
		'Title of the highlight "Handles".',
	),
	highlightHandlesBody: message(
		'website.cli.highlight.handles.body',
		'Explanation of the highlight "Handles"; <code> marks a command, an option or a value.',
	),
	highlightFormatsTitle: message(
		'website.cli.highlight.formats.title',
		'Title of the highlight "Text or JSON".',
	),
	highlightFormatsBody: message(
		'website.cli.highlight.formats.body',
		'Explanation of the highlight "Text or JSON"; <code> marks a command, an option or a value.',
	),
	highlightWaitingTitle: message(
		'website.cli.highlight.waiting.title',
		'Title of the highlight "Waiting is a command".',
	),
	highlightWaitingBody: message(
		'website.cli.highlight.waiting.body',
		'Explanation of the highlight "Waiting is a command"; <code> marks a command, an option or a value.',
	),
	highlightGuideTitle: message(
		'website.cli.highlight.guide.title',
		'Title of the highlight "A guide for agents, built in".',
	),
	highlightGuideBody: message(
		'website.cli.highlight.guide.body',
		'Explanation of the highlight "A guide for agents, built in"; <code> marks a command, an option or a value.',
	),
	highlightCompletionTitle: message(
		'website.cli.highlight.completion.title',
		'Title of the highlight "Completion for five shells".',
	),
	highlightCompletionBody: message(
		'website.cli.highlight.completion.body',
		'Explanation of the highlight "Completion for five shells"; <code> marks a command, an option or a value.',
	),
	highlightExitCodesTitle: message(
		'website.cli.highlight.exitCodes.title',
		'Title of the highlight "Exit codes that mean something".',
	),
	highlightExitCodesBody: message(
		'website.cli.highlight.exitCodes.body',
		'Explanation of the highlight "Exit codes that mean something"; <code> marks a command, an option or a value.',
	),
	familiesTitle: message('website.cli.families.title', 'Heading of the table of command families.'),
	familiesLead: message(
		'website.cli.families.lead',
		'Paragraph under the command families heading; <code> marks the help command.',
	),
	familiesTerminals: message(
		'website.cli.families.terminals',
		'Title of the command family group "Terminals and layout"; the command line\'s own help heading in this language.',
	),
	familiesAgents: message(
		'website.cli.families.agents',
		'Title of the command family group "Agents and workflows"; the command line\'s own help heading in this language.',
	),
	familiesSessions: message(
		'website.cli.families.sessions',
		'Title of the command family group "Sessions and connections"; the command line\'s own help heading in this language.',
	),
	familiesSetup: message(
		'website.cli.families.setup',
		'Title of the command family group "Setup and integration"; the command line\'s own help heading in this language.',
	),
	familyTui: message(
		'website.cli.family.tui',
		"One-line help of the tui command family; the command line's own help string in this language.",
	),
	familyWorkspace: message(
		'website.cli.family.workspace',
		"One-line help of the workspace command family; the command line's own help string in this language.",
	),
	familyTab: message(
		'website.cli.family.tab',
		"One-line help of the tab command family; the command line's own help string in this language.",
	),
	familyPanel: message(
		'website.cli.family.panel',
		"One-line help of the panel command family; the command line's own help string in this language.",
	),
	familyTerminal: message(
		'website.cli.family.terminal',
		"One-line help of the terminal command family; the command line's own help string in this language.",
	),
	familyLayout: message(
		'website.cli.family.layout',
		"One-line help of the layout command family; the command line's own help string in this language.",
	),
	familyClient: message(
		'website.cli.family.client',
		"One-line help of the client command family; the command line's own help string in this language.",
	),
	familyAgent: message(
		'website.cli.family.agent',
		"One-line help of the agent command family; the command line's own help string in this language.",
	),
	familyProject: message(
		'website.cli.family.project',
		"One-line help of the project command family; the command line's own help string in this language.",
	),
	familyWorktree: message(
		'website.cli.family.worktree',
		"One-line help of the worktree command family; the command line's own help string in this language.",
	),
	familyLaunchProfile: message(
		'website.cli.family.launchProfile',
		"One-line help of the launch-profile command family; the command line's own help string in this language.",
	),
	familyCommand: message(
		'website.cli.family.command',
		"One-line help of the command command family; the command line's own help string in this language.",
	),
	familyEvents: message(
		'website.cli.family.events',
		"One-line help of the events command family; the command line's own help string in this language.",
	),
	familyNotification: message(
		'website.cli.family.notification',
		"One-line help of the notification command family; the command line's own help string in this language.",
	),
	familyStatus: message(
		'website.cli.family.status',
		"One-line help of the status command family; the command line's own help string in this language.",
	),
	familyDoctor: message(
		'website.cli.family.doctor',
		"One-line help of the doctor command family; the command line's own help string in this language.",
	),
	familyOperation: message(
		'website.cli.family.operation',
		"One-line help of the operation command family; the command line's own help string in this language.",
	),
	familyServer: message(
		'website.cli.family.server',
		"One-line help of the server command family; the command line's own help string in this language.",
	),
	familySession: message(
		'website.cli.family.session',
		"One-line help of the session command family; the command line's own help string in this language.",
	),
	familyMachine: message(
		'website.cli.family.machine',
		"One-line help of the machine command family; the command line's own help string in this language.",
	),
	familyConfig: message(
		'website.cli.family.config',
		"One-line help of the config command family; the command line's own help string in this language.",
	),
	familyDiagnostics: message(
		'website.cli.family.diagnostics',
		"One-line help of the diagnostics command family; the command line's own help string in this language.",
	),
	familyUpdate: message(
		'website.cli.family.update',
		"One-line help of the update command family; the command line's own help string in this language.",
	),
	familyUninstall: message(
		'website.cli.family.uninstall',
		"One-line help of the uninstall command family; the command line's own help string in this language.",
	),
	familyApi: message(
		'website.cli.family.api',
		"One-line help of the api command family; the command line's own help string in this language.",
	),
	familyCompletion: message(
		'website.cli.family.completion',
		"One-line help of the completion command family; the command line's own help string in this language.",
	),
});
