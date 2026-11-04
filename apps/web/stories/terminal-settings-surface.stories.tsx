import { DEFAULT_TERMINAL_APPEARANCE_PREFERENCES } from '../src/terminal-settings/model/terminal-appearance-preferences';
import {
	UserLaunchProfileIdSchema,
	type UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { TerminalSettingsController } from '../src/terminal-settings';
import { KeyboardShortcutSettingsController } from '../src/keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../src/macos-terminal-input-settings';
import {
	TerminalFeedbackSettingsController,
	type TerminalNotificationPermission,
} from '../src/terminal-feedback-settings';
import { TerminalSettingsSurface } from '../src/terminal-settings-surface';
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
import type { UserPreferencesControl } from '../src/user-preferences';
import { memoryUserPreferences } from '../src/__tests__/memoryUserPreferences';

const englishPreferences = memoryUserPreferences({ locale: 'en-US' });
const englishApplicationLocaleController = createStoryApplicationLocaleController(
	'en-US',
	englishPreferences,
);
const englishController = createStoryController(englishPreferences);
const englishKeyboardController = createKeyboardStoryController('apple');
const englishMacosInputController = createMacosInputStoryController();
const englishFeedbackController = createFeedbackStoryController(englishPreferences);
const englishTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(true);
const englishInteractionController = new TerminalInteractionSettingsController(englishPreferences);
const englishTerminalDefaults = createTerminalDefaultSettingsStoryControl();
const englishAgentIntegrations = createAgentIntegrationStoryControl();

const chinesePreferences = memoryUserPreferences({
	applicationTheme: 'dark',
	customBackground: '#14161b',
	customForeground: '#e6e8ef',
	locale: 'zh-CN',
	terminalDisplayTheme: 'custom',
});
const chineseApplicationLocaleController = createStoryApplicationLocaleController(
	'zh-CN',
	chinesePreferences,
);

const chineseController = createStoryController(
	chinesePreferences,
	JSON.stringify({
		backgroundOpacityPercent: 72,
		fontFamily: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontFamily,
		fontLigatures: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontLigatures,
		fontSize: 15,
		fontWeight: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontWeight,
		fontWeightBold: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontWeightBold,
		terminalPadding: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.terminalPadding,
	}),
);
const chineseKeyboardController = createKeyboardStoryController('standard');
const chineseFeedbackController = createFeedbackStoryController(chinesePreferences);
const chineseTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(false);
const chineseInteractionController = new TerminalInteractionSettingsController(chinesePreferences);
const chineseTerminalDefaults = createTerminalDefaultSettingsStoryControl(24_000_000);
const chineseAgentIntegrations = createAgentIntegrationStoryControl();

const meta = {
	title: 'Terminal Settings Surface',
	component: TerminalSettingsSurface,
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof TerminalSettingsSurface>;

export default meta;

type Story = StoryObj<typeof meta>;

export const EnglishLight: Story = {
	args: {
		agentIntegrationManagementControl: englishAgentIntegrations,
		applicationLocaleController: englishApplicationLocaleController,
		controller: englishController,
		keyboardShortcutController: englishKeyboardController,
		keyboardShortcutPlatform: 'apple',
		macosTerminalInputSettingsController: englishMacosInputController,
		onRequestClose: () => englishController.close(),

		terminalFeedbackSettingsController: englishFeedbackController,
		terminalLaunchProfileClient: englishTerminalLaunchProfiles,
		terminalInteractionSettingsController: englishInteractionController,
		terminalDefaultSettingsControl: englishTerminalDefaults,
	},
	parameters: {
		docs: {
			description: {
				story:
					'Inspect the centered bounded shell, quiet grouped rail with subdued localized labels, neutral selection, compact content measure, direct setting rows with hairline separators, and stable trailing controls. Switch Display language to verify immediate mounted-copy replacement, then inspect live padding, weights, ligatures, copy-on-select, and the Desktop scrollback default with its future-terminal notice. Open Profiles to manage local system and Fish launches, literal ordered arguments, the future-terminal boundary, and confirmed destructive actions. Type language or font to see global General or Text results without blanking the current page, then activate a setting to move focus to its stable destination. Resize below the medium breakpoint to verify one-pane category/content navigation. Open Keyboard shortcuts to search localized action copy or current Apple chords and use Record keys as an exact discovery-only lookup. Capability-backed categories remain visible while roadmap-only categories and controls are absent; Shell integration explains automatic Desktop scope and exact manual fallbacks, and Remote development remains read-only.',
			},
		},
	},
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

export const ChineseDarkCustom: Story = {
	args: {
		agentIntegrationManagementControl: chineseAgentIntegrations,
		applicationLocaleController: chineseApplicationLocaleController,
		controller: chineseController,
		keyboardShortcutController: chineseKeyboardController,
		keyboardShortcutPlatform: 'standard',
		onRequestClose: () => chineseController.close(),

		terminalFeedbackSettingsController: chineseFeedbackController,
		terminalLaunchProfileClient: chineseTerminalLaunchProfiles,
		terminalInteractionSettingsController: chineseInteractionController,
		terminalDefaultSettingsControl: chineseTerminalDefaults,
	},
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

function createStoryApplicationLocaleController(
	locale: 'en-US' | 'zh-CN',
	userPreferences: UserPreferencesControl,
): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: { dir: '', lang: '' },
		preferredLocales: [locale],
		userPreferences,
	});
}

/** The host record holds the appearance this device keeps; the shared preferences hold the rest. */
function createStoryController(
	userPreferences: UserPreferencesControl,
	hostRecord: string | null = null,
): TerminalSettingsController {
	const controller = new TerminalSettingsController({
		storage: {
			getItem: () => hostRecord,
			setItem: () => undefined,
		},
		themeTarget: { dataset: {} },
		userPreferences,
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
	userPreferences: UserPreferencesControl,
): TerminalFeedbackSettingsController {
	return new TerminalFeedbackSettingsController({
		// Stories ask for no notification permission.
		notificationCapability: {
			permission: () => 'unsupported',
			requestPermission: (): Promise<TerminalNotificationPermission> =>
				Promise.resolve('unsupported'),
			show: () => false,
		},
		userPreferences,
	});
}

function createMacosInputStoryController(): MacosTerminalInputSettingsController {
	return new MacosTerminalInputSettingsController({
		getItem: () => null,
		setItem: () => undefined,
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
			persistenceIssue: null,
			source: 'stored',
		}),
		failureCode: null,
		pending: false,
	};
	const listeners = new Set<() => void>();
	const commit = (catalog: UserLaunchProfileCatalog) => {
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
			const id = UserLaunchProfileIdSchema.parse(`story-profile-${++profileSequence}`);
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
