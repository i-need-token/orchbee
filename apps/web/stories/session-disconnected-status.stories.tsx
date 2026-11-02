import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { SessionDisconnectedStatus } from '../src/desktop-client/session-disconnected-status';

const ended = { getSnapshot: () => true, subscribe: () => () => undefined };

const meta = {
	title: 'Desktop/Session Disconnected Status',
	component: SessionDisconnectedStatus,
	args: { ended, onReconnect: () => undefined },
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof SessionDisconnectedStatus>;

export default meta;

type Story = StoryObj<typeof meta>;

export const English: Story = {
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='en-US'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};

export const SimplifiedChinese: Story = {
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='zh-CN'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};
