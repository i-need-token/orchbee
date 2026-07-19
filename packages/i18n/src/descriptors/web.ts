import { defineMessages } from 'react-intl';

import { enUSWebMessages } from '../locales';

export const webMessages = defineMessages({
	agentOutputIntroduction: {
		id: 'web.inMemory.sessions.agentOutput.introduction',
		defaultMessage: enUSWebMessages['web.inMemory.sessions.agentOutput.introduction'],
		description: 'Initial transcript shown in the demonstration agent-output session.',
	},
	agentOutputTitle: {
		id: 'web.inMemory.sessions.agentOutput.title',
		defaultMessage: enUSWebMessages['web.inMemory.sessions.agentOutput.title'],
		description: 'Title of the demonstration agent-output session.',
	},
	helpOutput: {
		id: 'web.inMemory.commands.help.output',
		defaultMessage: enUSWebMessages['web.inMemory.commands.help.output'],
		description: 'Output of the help command in the in-memory demonstration terminal.',
	},
	localShellIntroduction: {
		id: 'web.inMemory.sessions.localShell.introduction',
		defaultMessage: enUSWebMessages['web.inMemory.sessions.localShell.introduction'],
		description: 'Initial transcript shown in the demonstration local-shell session.',
	},
	localShellTitle: {
		id: 'web.inMemory.sessions.localShell.title',
		defaultMessage: enUSWebMessages['web.inMemory.sessions.localShell.title'],
		description: 'Title of the demonstration local-shell session.',
	},
	newSessionIntroduction: {
		id: 'web.inMemory.sessions.new.introduction',
		defaultMessage: enUSWebMessages['web.inMemory.sessions.new.introduction'],
		description: 'Initial transcript shown in a newly created demonstration terminal session.',
	},
	newSessionTitle: {
		id: 'web.inMemory.sessions.new.title',
		defaultMessage: enUSWebMessages['web.inMemory.sessions.new.title'],
		description: 'Generated title for a new demonstration terminal session.',
	},
	simulatedCommandOutput: {
		id: 'web.inMemory.commands.simulated.output',
		defaultMessage: enUSWebMessages['web.inMemory.commands.simulated.output'],
		description: 'Output shown after an unknown command in the demonstration terminal.',
	},
	statusOutput: {
		id: 'web.inMemory.commands.status.output',
		defaultMessage: enUSWebMessages['web.inMemory.commands.status.output'],
		description: 'Terminal-size output of the status command in the demonstration terminal.',
	},
});
