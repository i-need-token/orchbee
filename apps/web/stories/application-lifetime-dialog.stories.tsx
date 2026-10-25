import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { DesktopApplicationExitRequestIdSchema } from '@orchbee/desktop-client-protocol';
import {
	DesktopApplicationLifetimeDialog,
	type DesktopApplicationLifetimeDialogController,
} from '../src/desktop-client/desktop-application-lifetime-dialog';

class StoryController implements DesktopApplicationLifetimeDialogController {
	readonly state: ReturnType<DesktopApplicationLifetimeDialogController['getSnapshot']>;

	constructor(state: ReturnType<DesktopApplicationLifetimeDialogController['getSnapshot']>) {
		this.state = state;
	}

	choose(): void {}
	getSnapshot = (): ReturnType<DesktopApplicationLifetimeDialogController['getSnapshot']> =>
		this.state;
	quitAnyway(): void {}
	retry(): void {}
	subscribe = (): (() => void) => () => undefined;
}

const requestId = DesktopApplicationExitRequestIdSchema.parse('story-exit');

const meta = {
	title: 'Desktop/Application Lifetime Dialog',
	component: DesktopApplicationLifetimeDialog,
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
	parameters: { layout: 'centered' },
} satisfies Meta<typeof DesktopApplicationLifetimeDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const AwaitingChoice: Story = {
	args: {
		controller: new StoryController({
			kind: 'awaiting-choice',
			requestId,
			sessionCount: 12,
		}),
	},
};

export const Stopping: Story = {
	args: {
		controller: new StoryController({
			kind: 'working',
			progress: {
				completedCount: 7,
				failedCount: 0,
				phase: 'stopping-terminals',
				totalCount: 12,
			},
			requestId,
		}),
	},
};

export const StopFailure: Story = {
	args: {
		controller: new StoryController({
			failures: [{ code: 'terminal-stop-failed', count: 2, retryable: true }],
			kind: 'failed',
			requestId,
			retry: 'terminal',
		}),
	},
};
