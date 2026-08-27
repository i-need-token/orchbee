import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	BrowserAuthenticationPage,
	type BrowserAuthenticationControllerPort,
	type BrowserAuthenticationState,
} from '../src/browser-authentication-page';

class StoryController implements BrowserAuthenticationControllerPort {
	readonly #state: BrowserAuthenticationState;

	constructor(state: BrowserAuthenticationState) {
		this.#state = state;
	}

	dispose(): void {}
	getSnapshot = (): BrowserAuthenticationState => this.#state;
	setPersonalAccessToken(): void {}
	submit(): void {}
	subscribe = (): (() => void) => () => undefined;
}

const meta = {
	title: 'Browser Authentication',
	component: BrowserAuthenticationPage,
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof BrowserAuthenticationPage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const English: Story = {
	args: {
		controller: new StoryController({ error: null, pending: false, personalAccessToken: '' }),
	},
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='en-US'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};

export const SimplifiedChineseInvalidToken: Story = {
	args: {
		controller: new StoryController({
			error: 'invalid-personal-access-token',
			pending: false,
			personalAccessToken: '•••••••••••••••••••••••••••••••••••••••••••',
		}),
	},
	decorators: [
		(StoryComponent) => (
			<OrchbeeI18nProvider locale='zh-CN'>
				<StoryComponent />
			</OrchbeeI18nProvider>
		),
	],
};
