import {
	UserLaunchProfileIdSchema,
	type UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';
import type * as React from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { TerminalSettingsController } from '../src/terminal-settings';
import { KeyboardShortcutSettingsController } from '../src/keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../src/macos-terminal-input-settings';
import { TerminalFeedbackSettingsController } from '../src/terminal-feedback-settings';
import {
	TerminalSettingsSurface,
	type TerminalSettingsAbout,
} from '../src/terminal-settings-surface';
import {
	TerminalLaunchProfileSnapshotSchema,
	type TerminalLaunchProfileClientSnapshot,
	type TerminalLaunchProfileControl,
} from '../src/terminal-launch-profile-control';
import { TerminalInteractionSettingsController } from '../src/terminal-interaction-settings';
import type { TerminalDefaultSettingsControl } from '../src/terminal-default-settings-control';
import {
	AGENT_INTEGRATION_IDS,
	type AgentIntegrationId,
	type AgentIntegrationManagementClientSnapshot,
	type AgentIntegrationManagementControl,
	type AgentIntegrationSnapshotItem,
} from '../src/agent-integration-management';
import { ApplicationLocaleController } from '../src/application-locale';
import { ApplicationLocaleProvider } from '../src/application-locale-provider';
import type { UserSettingsControl } from '../src/user-settings';
import { memoryUserSettings } from '../src/__tests__/memoryUserSettings';

const englishSettings = memoryUserSettings({
	filePath: '/Users/person/.orchbee/config.toml',
	values: { 'general.language': 'en-US' },
});
const englishApplicationLocaleController = createStoryApplicationLocaleController(
	'en-US',
	englishSettings,
);
const englishController = createStoryController(englishSettings);
const fontListController = createStoryController(
	memoryUserSettings({ values: { 'terminal.font_family': '"Fira Code", Menlo, monospace' } }),
);
const englishKeyboardController = createKeyboardStoryController('apple');
const englishMacosInputController = new MacosTerminalInputSettingsController(englishSettings);
const englishFeedbackController = createFeedbackStoryController(englishSettings);
const englishTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(true);
const englishInteractionController = new TerminalInteractionSettingsController(englishSettings);
const englishTerminalDefaults = createTerminalDefaultSettingsStoryControl();
const englishAgentIntegrations = createAgentIntegrationStoryControl();

const chineseSettings = memoryUserSettings({
	values: {
		'appearance.custom_background': '#14161b',
		'appearance.custom_foreground': '#e6e8ef',
		'appearance.mode': 'dark',
		'appearance.terminal_colors': 'custom',
		'general.language': 'zh-CN',
		'terminal.background_opacity': 72,
		'terminal.font_size': 15,
	},
});
const chineseApplicationLocaleController = createStoryApplicationLocaleController(
	'zh-CN',
	chineseSettings,
);

const chineseController = createStoryController(chineseSettings);
const chineseKeyboardController = createKeyboardStoryController('standard');
const chineseFeedbackController = createFeedbackStoryController(chineseSettings);
const chineseTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(false);
const chineseInteractionController = new TerminalInteractionSettingsController(chineseSettings);
const chineseTerminalDefaults = createTerminalDefaultSettingsStoryControl(24_000_000);
const chineseAgentIntegrations = createAgentIntegrationStoryControl();

const brokenFileSettings = memoryUserSettings({
	file: { column: 14, line: 9, reason: 'invalid-toml', status: 'syntax-error' },
	filePath: '/Users/person/.orchbee/config.toml',
	values: { 'general.language': 'en-US', 'terminal.font_size': 15 },
});
const brokenFileController = createStoryController(brokenFileSettings);

/** The families a Windows device commonly lists, so the font list shows samples and symbol faces. */
const storyFontCatalog = {
	listFamilies: (): Promise<readonly string[]> =>
		Promise.resolve([
			'Arial',
			'Cascadia Code',
			'Cascadia Mono',
			'Consolas',
			'Courier New',
			'Georgia',
			'Lucida Console',
			'Segoe UI',
			'Tahoma',
			'Times New Roman',
			'Trebuchet MS',
			'Verdana',
			'Webdings',
			'Wingdings',
		]),
};

const meta = {
	title: 'Terminal Settings Surface',
	component: TerminalSettingsSurface,
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof TerminalSettingsSurface>;

export default meta;

type Story = StoryObj<typeof meta>;
type TerminalSettingsCategory = NonNullable<
	React.ComponentProps<typeof TerminalSettingsSurface>['initialCategory']
>;

const englishArgs = {
	about: createAboutStorySource({ kind: 'idle' }),
	agentIntegrationManagementControl: englishAgentIntegrations,
	applicationLocaleController: englishApplicationLocaleController,
	controller: englishController,
	fontCatalog: storyFontCatalog,
	keyboardShortcutController: englishKeyboardController,
	keyboardShortcutPlatform: 'apple',
	macosTerminalInputSettingsController: englishMacosInputController,
	onRequestClose: () => englishController.close(),
	terminalFeedbackSettingsController: englishFeedbackController,
	terminalInteractionSettingsController: englishInteractionController,
	terminalLaunchProfileClient: englishTerminalLaunchProfiles,
	terminalDefaultSettingsControl: englishTerminalDefaults,
} satisfies Story['args'];

const chineseArgs = {
	about: createAboutStorySource({ kind: 'idle' }),
	agentIntegrationManagementControl: chineseAgentIntegrations,
	applicationLocaleController: chineseApplicationLocaleController,
	controller: chineseController,
	fontCatalog: storyFontCatalog,
	keyboardShortcutController: chineseKeyboardController,
	keyboardShortcutPlatform: 'standard',
	onRequestClose: () => chineseController.close(),
	terminalFeedbackSettingsController: chineseFeedbackController,
	terminalInteractionSettingsController: chineseInteractionController,
	terminalLaunchProfileClient: chineseTerminalLaunchProfiles,
	terminalDefaultSettingsControl: chineseTerminalDefaults,
} satisfies Story['args'];

function englishLight(initialCategory: TerminalSettingsCategory): Story {
	return {
		args: { ...englishArgs, initialCategory },
		decorators: [
			(Story) => (
				<div data-theme='light' style={{ height: '100vh', position: 'relative' }}>
					<ApplicationLocaleProvider source={englishApplicationLocaleController}>
						<Story />
					</ApplicationLocaleProvider>
				</div>
			),
		],
	};
}

function chineseDark(initialCategory: TerminalSettingsCategory): Story {
	return {
		args: { ...chineseArgs, initialCategory },
		decorators: [
			(Story) => (
				<div data-theme='dark' style={{ height: '100vh', position: 'relative' }}>
					<ApplicationLocaleProvider source={chineseApplicationLocaleController}>
						<Story />
					</ApplicationLocaleProvider>
				</div>
			),
		],
	};
}

export const EnglishGeneral: Story = {
	...englishLight('general'),
	parameters: {
		docs: {
			description: {
				story:
					'Settings is a large dialog over the workspace: the close button, Escape, and a press outside it return to the workbench, search filters every category, and the eight categories open centered cards of rows with one control each. Change a setting to see its dot and Restore default action; type in search to see matches grouped under category names.',
			},
		},
	},
};
export const EnglishAppearance: Story = englishLight('appearance');
/** A font list of several families in the Font field: the first is drawn and the rest are fallbacks. */
export const EnglishAppearanceFontList: Story = {
	...englishLight('appearance'),
	args: {
		...englishArgs,
		controller: fontListController,
		initialCategory: 'appearance',
		onRequestClose: () => fontListController.close(),
	},
};
export const EnglishThemes: Story = englishLight('themes');
export const EnglishTerminal: Story = englishLight('terminal');
export const EnglishNotifications: Story = englishLight('notifications');
export const EnglishKeyboard: Story = {
	...englishLight('keyboard'),
	args: {
		...englishArgs,
		initialCategory: 'keyboard',
		keyboardShortcutsTakenByConfiguredCommands: new Map([
			['workbench.search.actions.open', 'deploy'],
		]),
	},
};
export const EnglishAgents: Story = englishLight('agents');
export const EnglishAbout: Story = {
	...englishLight('about'),
	parameters: {
		docs: {
			description: {
				story:
					'About shows the version this window runs with its build, and Updates with Check for updates. Press it to see Checking…, then Orchbee is up to date; the other About stories hold each state still.',
			},
		},
	},
};
export const EnglishAboutChecking: Story = aboutStory(englishLight, { kind: 'checking' });
export const EnglishAboutUpToDate: Story = aboutStory(englishLight, { kind: 'up-to-date' });
export const EnglishAboutFailed: Story = aboutStory(englishLight, { kind: 'failed' });
export const EnglishAboutUnavailable: Story = aboutStory(englishLight, {
	kind: 'unavailable',
	reason: 'session',
});
export const EnglishAboutReady: Story = aboutStory(englishLight, { kind: 'idle' }, '1.4.0');
export const EnglishSettingsFileError: Story = {
	...englishLight('appearance'),
	args: {
		applicationLocaleController: englishApplicationLocaleController,
		controller: brokenFileController,
		initialCategory: 'appearance',
		keyboardShortcutController: createKeyboardStoryController('apple'),
		keyboardShortcutPlatform: 'apple',
		onRequestClose: () => brokenFileController.close(),
	},
};
export const ChineseGeneral: Story = chineseDark('general');
export const ChineseAppearanceCustomColors: Story = chineseDark('appearance');
export const ChineseThemes: Story = chineseDark('themes');
export const ChineseTerminal: Story = chineseDark('terminal');
export const ChineseNotifications: Story = chineseDark('notifications');
export const ChineseKeyboard: Story = chineseDark('keyboard');
export const ChineseAgents: Story = chineseDark('agents');
export const ChineseAbout: Story = chineseDark('about');
export const ChineseAboutChecking: Story = aboutStory(chineseDark, { kind: 'checking' });
export const ChineseAboutUpToDate: Story = aboutStory(chineseDark, { kind: 'up-to-date' });
export const ChineseAboutFailed: Story = aboutStory(chineseDark, { kind: 'failed' });
export const ChineseAboutUnavailable: Story = aboutStory(chineseDark, {
	kind: 'unavailable',
	reason: 'session',
});
export const ChineseAboutReady: Story = aboutStory(chineseDark, { kind: 'idle' }, '1.4.0');

type AboutStatus = ReturnType<NonNullable<TerminalSettingsAbout['check']>['getSnapshot']>;

/** About held in `status`, with `offer` as the newer version the window can switch to. */
function aboutStory(
	theme: (initialCategory: TerminalSettingsCategory) => Story,
	status: AboutStatus,
	offer: string | null = null,
): Story {
	const story = theme('about');
	return {
		...story,
		args: { ...story.args, about: createAboutStorySource(status, offer, false) },
	};
}

/**
 * About for a release window at 1.3.0. A live source answers Check for updates with Checking…, then
 * Up to date a moment later; a held one stays in `status`.
 */
function createAboutStorySource(
	status: AboutStatus,
	offer: string | null = null,
	live = true,
): TerminalSettingsAbout {
	let current = status;
	const listeners = new Set<() => void>();
	const set = (next: AboutStatus): void => {
		current = next;
		for (const listener of listeners) listener();
	};
	return {
		build: 'release',
		check: {
			getSnapshot: () => current,
			show: () => () => undefined,
			start: () => {
				if (!live) return;
				set({ kind: 'checking' });
				setTimeout(() => set({ kind: 'up-to-date' }), 1500);
			},
			subscribe: (listener) => {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
		},
		downloadInstaller: () => undefined,
		offer: { getSnapshot: () => offer, subscribe: () => () => undefined },
		update: () => undefined,
		version: '1.3.0',
	};
}

function createStoryApplicationLocaleController(
	locale: 'en-US' | 'zh-CN',
	userSettings: UserSettingsControl,
): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: { dir: '', lang: '' },
		preferredLocales: [locale],
		userSettings,
	});
}

function createStoryController(userSettings: UserSettingsControl): TerminalSettingsController {
	const controller = new TerminalSettingsController({
		themeTarget: { dataset: {} },
		userSettings,
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

function createFeedbackStoryController(
	userSettings: UserSettingsControl,
): TerminalFeedbackSettingsController {
	return new TerminalFeedbackSettingsController({
		// Stories ask for no notification permission.
		notificationCapability: {
			permission: () => 'unsupported',
			requestPermission: () => Promise.resolve<'unsupported'>('unsupported'),
			show: () => false,
		},
		userSettings,
	});
}

function createTerminalDefaultSettingsStoryControl(
	scrollbackBytes = 10_000_000,
): TerminalDefaultSettingsControl {
	const snapshot = { configuration: { scrollbackBytes }, failureCode: null };
	return { getSnapshot: () => snapshot, subscribe: () => () => undefined };
}

function createTerminalLaunchProfileStoryControl(
	loginShell: boolean,
): TerminalLaunchProfileControl {
	const builtInProfileId = UserLaunchProfileIdSchema.parse('system-default');
	let profileSequence = 1;
	let snapshot: TerminalLaunchProfileClientSnapshot = {
		...TerminalLaunchProfileSnapshotSchema.parse({
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
			source: 'stored',
		}),
		failureCode: null,
		pending: false,
		readOnly: false,
		refusal: null,
	};
	const listeners = new Set<() => void>();
	const commit = (catalog: UserLaunchProfileCatalog) => {
		snapshot = { ...snapshot, catalog };
		for (const listener of listeners) listener();
		return Promise.resolve({ kind: 'completed' as const, snapshot });
	};
	return {
		createProfile: (name) => {
			const id = UserLaunchProfileIdSchema.parse(`story-profile-${++profileSequence}`);
			return commit({
				...snapshot.catalog,
				profiles: [
					...snapshot.catalog.profiles,
					{ command: { kind: 'system-shell', login: false }, id, name },
				],
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

function createAgentIntegrationStoryControl(): AgentIntegrationManagementControl {
	let snapshot: AgentIntegrationManagementClientSnapshot = createAgentIntegrationStorySnapshot();
	const listeners = new Set<() => void>();
	const update = (integrationId: AgentIntegrationId, enabled: boolean) => {
		snapshot = {
			...snapshot,
			integrations: snapshot.integrations.map((integration) =>
				integration.integrationId === integrationId
					? { ...integration, state: enabled ? 'enabled' : 'disabled' }
					: integration,
			),
			revision: snapshot.revision + 1,
		};
		for (const listener of listeners) listener();
		return Promise.resolve({
			kind: 'completed' as const,
			snapshot: {
				integrations: snapshot.integrations,
				revision: snapshot.revision,
			},
		});
	};
	return {
		disable: (integrationId) => update(integrationId, false),
		enable: (integrationId) => update(integrationId, true),
		explain: (integrationId) => {
			const item = snapshot.integrations.find(
				(integration) => integration.integrationId === integrationId,
			);
			if (item === undefined) return Promise.resolve({ kind: 'unavailable' as const });
			snapshot = {
				...snapshot,
				diagnosis: {
					explanation: {
						adapter: 'ready',
						detail: { entry: 'missing', kind: 'guarded', sourceReady: true },
						item,
						plan: [{ kind: 'publish-adapter' }, { kind: 'write-entry' }],
						timedOut: false,
					},
					integrationId,
					kind: 'explained',
				},
				revision: snapshot.revision + 1,
			};
			for (const listener of listeners) listener();
			return Promise.resolve({
				kind: 'completed' as const,
				snapshot: { integrations: snapshot.integrations, revision: snapshot.revision },
			});
		},
		getSnapshot: () => snapshot,
		refresh: () =>
			Promise.resolve({
				kind: 'completed' as const,
				snapshot: {
					integrations: snapshot.integrations,
					revision: snapshot.revision,
				},
			}),
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

function createAgentIntegrationStorySnapshot(): AgentIntegrationManagementClientSnapshot {
	const examples: Partial<
		Readonly<Record<AgentIntegrationId, Partial<AgentIntegrationSnapshotItem>>>
	> = {
		'claude-code': { availability: 'available', state: 'enabled' },
		codex: { availability: 'available', state: 'not-installed' },
		grok: { availability: 'available', state: 'disabled' },
		opencode: { availability: 'available', state: 'enabled' },
	};
	return {
		diagnosis: null,
		failureCode: null,
		failureIntegrationId: null,
		integrations: AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: 'not-found',
			integrationId,
			issue: null,
			managementMode: 'managed-local',
			state: 'unknown',
			...examples[integrationId],
		})),
		pending: false,
		pendingIntegrationId: null,
		revision: 1,
	};
}
