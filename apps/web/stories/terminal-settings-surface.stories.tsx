import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	TerminalSettingsController,
} from '../src/terminal-settings';
import { KeyboardShortcutSettingsController } from '../src/keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../src/macos-terminal-input-settings';
import {
	TerminalFeedbackSettingsController,
	UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY,
} from '../src/terminal-feedback-settings';
import { TerminalSettingsSurface } from '../src/terminal-settings-surface';

const englishController = createStoryController();
const englishKeyboardController = createKeyboardStoryController('apple');
const englishMacosInputController = createMacosInputStoryController();
const englishFeedbackController = createFeedbackStoryController();
const chineseController = createStoryController({
	...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	applicationTheme: 'dark',
	backgroundOpacityPercent: 72,
	customBackground: '#14161b',
	customForeground: '#e6e8ef',
	fontSize: 15,
	terminalDisplayTheme: 'custom',
});
const chineseKeyboardController = createKeyboardStoryController('standard');
const chineseFeedbackController = createFeedbackStoryController();

const meta = {
	title: 'Terminal Settings Surface',
	component: TerminalSettingsSurface,
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof TerminalSettingsSurface>;

export default meta;

type Story = StoryObj<typeof meta>;

export const EnglishLight: Story = {
	args: {
		controller: englishController,
		keyboardShortcutController: englishKeyboardController,
		keyboardShortcutPlatform: 'apple',
		macosTerminalInputSettingsController: englishMacosInputController,
		onRequestClose: () => englishController.close(),
		terminalFeedbackSettingsController: englishFeedbackController,
	},
	parameters: {
		docs: {
			description: {
				story:
					'Inspect the grouped Application, Terminal, Workspace, Integrations, and Data & Access rail; type font to see global Text results without blanking General, then activate Terminal font family to restore the rail and reveal the row. Neutral setting groups have no outer stroke, current controls remain interactive, and the comprehensive future catalog is clearly marked Planned and disabled. Shell integration remains manual-only, Remote development remains read-only, and Ghostty migration remains a paste-only dry-run.',
			},
		},
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

export const ChineseDarkCustom: Story = {
	args: {
		controller: chineseController,
		keyboardShortcutController: chineseKeyboardController,
		keyboardShortcutPlatform: 'standard',
		onRequestClose: () => chineseController.close(),
		requestBrowserAccess: () => undefined,
		terminalFeedbackSettingsController: chineseFeedbackController,
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

function createStoryController(
	preferences = DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
): TerminalSettingsController {
	const controller = new TerminalSettingsController({
		storage: {
			getItem: () => JSON.stringify(preferences),
			setItem: () => undefined,
		},
		themeTarget: { dataset: {} },
	});
	controller.open();
	return controller;
}

function createKeyboardStoryController(
	platform: 'apple' | 'standard',
): KeyboardShortcutSettingsController {
	return new KeyboardShortcutSettingsController({
		platform,
		storage: { getItem: () => null, setItem: () => undefined },
	});
}

function createFeedbackStoryController(): TerminalFeedbackSettingsController {
	return new TerminalFeedbackSettingsController({
		notificationCapability: UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY,
		storage: { getItem: () => null, setItem: () => undefined },
	});
}

function createMacosInputStoryController(): MacosTerminalInputSettingsController {
	return new MacosTerminalInputSettingsController({
		getItem: () => null,
		setItem: () => undefined,
	});
}
