import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationId,
} from '@orchbee/desktop-client-protocol';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import type {
	AgentIntegrationManagementClientSnapshot,
	AgentIntegrationManagementControl,
} from '../src/agent-integration-management';
import { AgentIntegrationReminderController } from '../src/agent-integration-management';
import { AgentIntegrationReminder } from '../src/agent-integration-reminder';

const englishControl = createStoryControl('codex', 'not-installed');
const englishController = new AgentIntegrationReminderController(storyStorage());
const chineseControl = createStoryControl('grok', 'disabled');
const chineseController = new AgentIntegrationReminderController(storyStorage());

const meta = {
	title: 'Agent Integration Reminder',
	component: AgentIntegrationReminder,
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof AgentIntegrationReminder>;

export default meta;

type Story = StoryObj<typeof meta>;

export const EnglishNotInstalled: Story = {
	args: {
		control: englishControl,
		controller: englishController,
		onReview: () => undefined,
	},
	decorators: [
		(Story) => (
			<div data-theme='light' style={{ height: '100vh', position: 'relative' }}>
				<OrchbeeI18nProvider locale='en-US'>
					<Story />
				</OrchbeeI18nProvider>
			</div>
		),
	],
};

export const ChineseDisabled: Story = {
	args: {
		control: chineseControl,
		controller: chineseController,
		onReview: () => undefined,
	},
	decorators: [
		(Story) => (
			<div data-theme='dark' style={{ height: '100vh', position: 'relative' }}>
				<OrchbeeI18nProvider locale='zh-CN'>
					<Story />
				</OrchbeeI18nProvider>
			</div>
		),
	],
};

function createStoryControl(
	integrationId: DesktopAgentIntegrationId,
	state: 'disabled' | 'not-installed',
): AgentIntegrationManagementControl {
	const snapshot: AgentIntegrationManagementClientSnapshot = {
		failureCode: null,
		failureIntegrationId: null,
		integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((candidateId) => ({
			availability: candidateId === integrationId ? 'available' : 'not-found',
			integrationId: candidateId,
			issue: null,
			managementMode: 'managed-local',
			state: candidateId === integrationId ? state : 'unknown',
		})),
		pending: false,
		pendingIntegrationId: null,
		revision: 1,
	};
	return {
		disable: () => Promise.resolve({ kind: 'unavailable' }),
		enable: () => Promise.resolve({ kind: 'unavailable' }),
		getSnapshot: () => snapshot,
		refresh: () => Promise.resolve({ kind: 'unavailable' }),
		subscribe: () => () => undefined,
	};
}

function storyStorage() {
	return {
		getItem: () => null,
		setItem: () => undefined,
	};
}
