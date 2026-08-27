import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { PersonalAccessTokenSchema } from '@orchbee/remote-client-protocol';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	DesktopBrowserAccessDialog,
	type DesktopBrowserAccessDialogController,
	type DesktopBrowserAccessDialogState,
} from '../src/desktop-client/desktop-browser-access-dialog';

class StoryController implements DesktopBrowserAccessDialogController {
	readonly #state: DesktopBrowserAccessDialogState;

	constructor(state: DesktopBrowserAccessDialogState) {
		this.#state = state;
	}

	cancelPersonalAccessTokenReset(): void {}
	close(): void {}
	confirmPersonalAccessTokenReset(): void {}
	copyAccessLink(): void {}
	copyPersonalAccessToken(): void {}
	dispose(): void {}
	getSnapshot = (): DesktopBrowserAccessDialogState => this.#state;
	open(): void {}
	openLocalBrowser(): void {}
	requestPersonalAccessTokenIssue(): void {}
	start(): void {}
	stop(): void {}
	subscribe = (): (() => void) => () => undefined;
}

const meta = {
	title: 'Desktop/Browser Access Dialog',
	component: DesktopBrowserAccessDialog,
	parameters: { layout: 'centered' },
} satisfies Meta<typeof DesktopBrowserAccessDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Stopped: Story = {
	args: {
		controller: new StoryController({
			copiedOrigin: null,
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: null,
			open: true,
			pendingAction: null,
			resetConfirmationOpen: false,
			snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'missing' },
		}),
	},
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
};

export const LocalNetworkReady: Story = {
	args: {
		controller: new StoryController({
			copiedOrigin: 'http://192.168.50.12:43110',
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: null,
			open: true,
			pendingAction: null,
			resetConfirmationOpen: false,
			snapshot: {
				access: {
					localOrigin: 'http://127.0.0.1:43110',
					mode: 'local-network',
					networkOrigins: ['http://10.1.2.3:43110', 'http://192.168.50.12:43110'],
					phase: 'ready',
				},
				personalAccessToken: 'configured',
			},
		}),
	},
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='zh-CN'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
};

export const NewlyIssuedPersonalAccessToken: Story = {
	args: {
		controller: new StoryController({
			copiedOrigin: null,
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: PersonalAccessTokenSchema.parse('p'.repeat(43)),
			open: true,
			pendingAction: null,
			resetConfirmationOpen: false,
			snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'configured' },
		}),
	},
	decorators: [
		(Story) => (
			<OrchbeeI18nProvider locale='en-US'>
				<Story />
			</OrchbeeI18nProvider>
		),
	],
};
