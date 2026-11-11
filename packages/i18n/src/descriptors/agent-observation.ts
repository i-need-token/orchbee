import { defineMessages } from 'react-intl';

import { enUSAgentObservationMessages } from '../locales';

export const agentObservationMessages = defineMessages({
	statusUnread: {
		id: 'agent.observation.status.unread',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.unread'],
		description: 'Client-local unread Agent activity, independent of the actual status.',
	},
	notificationBody: {
		id: 'agent.observation.notification.body',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.body'],
		description: 'Agent system notification body naming the Workspace and the terminal location.',
	},
	notificationDoneTitle: {
		id: 'agent.observation.notification.done.title',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.done.title'],
		description:
			'System notification title when a terminal Agent stops working; it does not claim success.',
	},
	notificationRequestTitle: {
		id: 'agent.observation.notification.request.title',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.request.title'],
		description: 'System notification title when a terminal Agent waits for the person.',
	},
	providerAgents: {
		id: 'agent.observation.provider.agents',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.agents'],
		description: 'Generic name for several different Agents shown together.',
	},
	providerClaudeCode: {
		id: 'agent.observation.provider.claudeCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.claudeCode'],
		description: 'Curated product name for the Claude Code Agent provider.',
	},
	providerCodex: {
		id: 'agent.observation.provider.codex',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.codex'],
		description: 'Curated product name for the Codex Agent provider.',
	},
	providerGrok: {
		id: 'agent.observation.provider.grok',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.grok'],
		description: 'Curated product name for the Grok Agent provider.',
	},
	providerGeminiCli: {
		id: 'agent.observation.provider.geminiCli',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.geminiCli'],
		description: 'Curated product name for the Gemini CLI Agent.',
	},
	providerGitHubCopilotCli: {
		id: 'agent.observation.provider.githubCopilotCli',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.githubCopilotCli'],
		description: 'Curated product name for the GitHub Copilot CLI Agent.',
	},
	providerCline: {
		id: 'agent.observation.provider.cline',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.cline'],
		description: 'Curated product name for the Cline CLI Agent.',
	},
	providerOmp: {
		id: 'agent.observation.provider.omp',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.omp'],
		description: 'Curated product name for the OMP Agent provider.',
	},
	providerOpenCode: {
		id: 'agent.observation.provider.openCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.openCode'],
		description: 'Curated product name for the OpenCode Agent provider.',
	},
	providerPi: {
		id: 'agent.observation.provider.pi',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.pi'],
		description: 'Curated product name for the Pi Agent provider.',
	},
	providerQwenCode: {
		id: 'agent.observation.provider.qwenCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.qwenCode'],
		description: 'Curated product name for the Qwen Code Agent.',
	},
	providerKilo: {
		id: 'agent.observation.provider.kilo',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.kilo'],
		description: 'Curated product name for the Kilo Agent provider.',
	},
	providerCursor: {
		id: 'agent.observation.provider.cursor',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.cursor'],
		description: 'Curated product name for the Cursor Agent provider.',
	},
	providerQoderCli: {
		id: 'agent.observation.provider.qoderCli',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.qoderCli'],
		description: 'Curated product name for the Qoder CLI Agent.',
	},
	providerAntigravityCli: {
		id: 'agent.observation.provider.antigravityCli',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.antigravityCli'],
		description: 'Curated product name for the Antigravity CLI Agent.',
	},
	providerMastracode: {
		id: 'agent.observation.provider.mastracode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.mastracode'],
		description: 'Curated product name for the Mastracode Agent provider.',
	},
	providerDroid: {
		id: 'agent.observation.provider.droid',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.droid'],
		description: 'Curated product name for the Droid Agent provider.',
	},
	providerKimi: {
		id: 'agent.observation.provider.kimi',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.kimi'],
		description: 'Curated product name for the Kimi Agent provider.',
	},
	providerLetta: {
		id: 'agent.observation.provider.letta',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.letta'],
		description: 'Curated product name for the Letta Agent provider.',
	},
	providerDevin: {
		id: 'agent.observation.provider.devin',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.devin'],
		description: 'Curated product name for the Devin Agent provider.',
	},
	providerHermes: {
		id: 'agent.observation.provider.hermes',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.hermes'],
		description: 'Curated product name for the Hermes Agent provider.',
	},
	providerKiroCli: {
		id: 'agent.observation.provider.kiroCli',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.kiroCli'],
		description: 'Curated product name for the Kiro CLI Agent.',
	},
	providerAmp: {
		id: 'agent.observation.provider.amp',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.amp'],
		description: 'Curated product name for the Amp Agent provider.',
	},
	providerMaki: {
		id: 'agent.observation.provider.maki',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.maki'],
		description: 'Curated product name for the Maki Agent provider.',
	},
	providerMuse: {
		id: 'agent.observation.provider.muse',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.muse'],
		description: 'Curated product name for the Muse Agent provider.',
	},
	providerAtomCode: {
		id: 'agent.observation.provider.atomCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.atomCode'],
		description: 'Curated product name for the AtomCode Agent provider.',
	},
	providerCodeBuddy: {
		id: 'agent.observation.provider.codeBuddy',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.codeBuddy'],
		description: 'Curated product name for the CodeBuddy Code Agent provider.',
	},
	providerCommandCode: {
		id: 'agent.observation.provider.commandCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.commandCode'],
		description: 'Curated product name for the Command Code Agent provider.',
	},
	providerDevEco: {
		id: 'agent.observation.provider.devEco',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.devEco'],
		description: 'Curated product name for the DevEco Code Agent provider.',
	},
	providerMimo: {
		id: 'agent.observation.provider.mimo',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.mimo'],
		description: 'Curated product name for the MiMo Code Agent provider.',
	},
	providerReasonix: {
		id: 'agent.observation.provider.reasonix',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.reasonix'],
		description: 'Curated product name for the Reasonix Agent provider.',
	},
	providerSnow: {
		id: 'agent.observation.provider.snow',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.snow'],
		description: 'Curated product name for the Snow CLI Agent provider.',
	},
	providerMinimaxCode: {
		id: 'agent.observation.provider.minimaxCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.minimaxCode'],
		description: 'Curated product name for the MiniMax Code Agent provider.',
	},
	providerMothx: {
		id: 'agent.observation.provider.mothx',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.mothx'],
		description: 'Curated product name for the MothX Agent provider.',
	},
	providerJcode: {
		id: 'agent.observation.provider.jcode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.jcode'],
		description: 'Curated product name for the J-Code Agent provider.',
	},
	providerOpenClaw: {
		id: 'agent.observation.provider.openClaw',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.openClaw'],
		description: 'Curated product name for the OpenClaw Agent provider.',
	},
	providerPrimeAgent: {
		id: 'agent.observation.provider.primeAgent',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.primeAgent'],
		description: 'Curated product name for the Prime Agent Agent provider.',
	},
	statusCanceled: {
		id: 'agent.observation.status.canceled',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.canceled'],
		description: 'Agent session status: its last turn was canceled.',
	},
	statusEnded: {
		id: 'agent.observation.status.ended',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.ended'],
		description: 'Agent session status: the session has ended.',
	},
	statusFailed: {
		id: 'agent.observation.status.failed',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.failed'],
		description: 'Terminal Agent state: the Agent reported that its work failed.',
	},
	statusInterrupted: {
		id: 'agent.observation.status.interrupted',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.interrupted'],
		description: 'Agent session status: its last turn was interrupted.',
	},
	statusNeedsAttention: {
		id: 'agent.observation.status.needsAttention',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.needsAttention'],
		description: 'Terminal Agent state: the Agent waits for the person.',
	},
	statusNeedsAttentionCount: {
		id: 'agent.observation.status.needsAttentionCount',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.needsAttentionCount'],
		description: 'Agent session status: it waits for the person on this many requests.',
	},
	statusReady: {
		id: 'agent.observation.status.ready',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.ready'],
		description: 'Terminal Agent state: the Agent is idle and ready for input.',
	},
	statusSucceeded: {
		id: 'agent.observation.status.succeeded',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.succeeded'],
		description: 'Terminal Agent state: the Agent finished its work.',
	},
	statusUnknown: {
		id: 'agent.observation.status.unknown',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.unknown'],
		description: 'Terminal Agent state that is not known; it does not guess idle or work.',
	},
	statusWorking: {
		id: 'agent.observation.status.working',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.working'],
		description: 'Terminal Agent state: the Agent is working.',
	},
});
