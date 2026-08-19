import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
	type DesktopTerminalLaunchProfileCatalog,
} from '@orchbee/desktop-client-protocol';
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
import type {
	DesktopTerminalLaunchProfileClientSnapshot,
	DesktopTerminalLaunchProfileControl,
} from '../src/terminal-launch-profile-control';

const englishController = createStoryController();
const englishKeyboardController = createKeyboardStoryController('apple');
const englishMacosInputController = createMacosInputStoryController();
const englishFeedbackController = createFeedbackStoryController();
const englishTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(true);
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
const chineseTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(false);

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
		terminalLaunchProfileClient: englishTerminalLaunchProfiles,
	},
	parameters: {
		docs: {
			description: {
				story:
					'Inspect the grouped Application, Terminal, Workspace, Integrations, and Data & Access rail; open Profiles to manage the local system and Fish launches, literal ordered arguments, future-terminal boundary, and confirmed destructive actions. Type font to see global Text results without blanking General, then activate Terminal font family to restore the rail and reveal the row. Neutral setting groups have no outer stroke, future controls are clearly marked Planned and disabled, Shell integration remains manual-only, Remote development remains read-only, and Ghostty migration remains a paste-only dry-run.',
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
		terminalLaunchProfileClient: chineseTerminalLaunchProfiles,
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

function createTerminalLaunchProfileStoryControl(
	loginShell: boolean,
): DesktopTerminalLaunchProfileControl {
	const builtInProfileId = DesktopTerminalLaunchProfileIdSchema.parse('system-default');
	let profileSequence = 1;
	let snapshot: DesktopTerminalLaunchProfileClientSnapshot = {
		...DesktopTerminalLaunchProfileSnapshotSchema.parse({
			capabilities: { loginShell },
			catalog: {
				defaultProfileId: builtInProfileId,
				profiles: [
					{
						command: { kind: 'system-shell', login: false },
						id: builtInProfileId,
						name: null,
					},
					{
						command: {
							arguments: ['--login'],
							executable: '/opt/homebrew/bin/fish',
							kind: 'custom-executable',
						},
						id: 'story-fish',
						name: 'Fish',
					},
				],
				version: 1,
			},
			persistenceIssue: null,
			source: 'stored',
		}),
		failureCode: null,
		pending: false,
	};
	const listeners = new Set<() => void>();
	const commit = (catalog: DesktopTerminalLaunchProfileCatalog) => {
		snapshot = { ...snapshot, catalog };
		for (const listener of listeners) listener();
		return Promise.resolve({ kind: 'completed' as const, snapshot });
	};
	return {
		createProfile: (name, sourceProfileId) => {
			const sourceIndex = snapshot.catalog.profiles.findIndex(
				(profile) => profile.id === sourceProfileId,
			);
			const source = snapshot.catalog.profiles[sourceIndex];
			if (source === undefined) return Promise.resolve({ kind: 'unavailable' });
			const id = DesktopTerminalLaunchProfileIdSchema.parse(`story-profile-${++profileSequence}`);
			const profiles = [...snapshot.catalog.profiles];
			profiles.splice(sourceIndex + 1, 0, { ...source, id, name });
			return commit({
				...snapshot.catalog,
				profiles,
			});
		},
		deleteProfile: (profileId) =>
			commit({
				...snapshot.catalog,
				profiles: snapshot.catalog.profiles.filter((profile) => profile.id !== profileId),
			}),
		getSnapshot: () => snapshot,
		reset: () =>
			commit({
				defaultProfileId: builtInProfileId,
				profiles: [
					{
						command: { kind: 'system-shell', login: false },
						id: builtInProfileId,
						name: null,
					},
				],
				version: 1,
			}),
		selectDefault: (profileId) => commit({ ...snapshot.catalog, defaultProfileId: profileId }),
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		updateProfile: (profile) =>
			commit({
				...snapshot.catalog,
				profiles: snapshot.catalog.profiles.map((candidate) =>
					candidate.id === profile.id ? profile : candidate,
				),
			}),
	};
}
