import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { SessionUnavailable } from '../src/desktop-client/session-unavailable';

const meta = {
	title: 'Desktop/Session Unavailable',
	component: SessionUnavailable,
	args: { onTryAgain: () => undefined, reason: 'unavailable' },
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof SessionUnavailable>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Unavailable: Story = {
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='en-US'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};

export const RecoveryRequired: Story = {
	args: { reason: 'recovery-required' },
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='en-US'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};

export const SimplifiedChinese: Story = {
	args: { reason: 'recovery-required' },
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='zh-CN'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};
