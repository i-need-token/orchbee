import { OrchbeeI18nProvider } from '@orchbee/i18n';
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

	close(): void {}
	copyAccessLink(): void {}
	dispose(): void {}
	getSnapshot = (): DesktopBrowserAccessDialogState => this.#state;
	open(): void {}
	openLocalBrowser(): void {}
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
			error: null,
			open: true,
			pendingAction: null,
			runtime: { phase: 'stopped' },
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
			error: null,
			open: true,
			pendingAction: null,
			runtime: {
				localOrigin: 'http://127.0.0.1:43110',
				mode: 'local-network',
				networkOrigins: ['http://10.1.2.3:43110', 'http://192.168.50.12:43110'],
				phase: 'ready',
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
