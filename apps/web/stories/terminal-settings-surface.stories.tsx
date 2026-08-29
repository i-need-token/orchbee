import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
	DesktopTerminalDefaultSettingsSnapshotSchema,
	type DesktopTerminalLaunchProfileCatalog,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationSnapshotItem,
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
import { TerminalInteractionSettingsController } from '../src/terminal-interaction-settings';
import type {
	DesktopTerminalDefaultSettingsClientSnapshot,
	DesktopTerminalDefaultSettingsControl,
} from '../src/terminal-default-settings-control';
import type {
	AgentIntegrationManagementClientSnapshot,
	AgentIntegrationManagementControl,
} from '../src/agent-integration-management';

const englishController = createStoryController();
const englishKeyboardController = createKeyboardStoryController('apple');
const englishMacosInputController = createMacosInputStoryController();
const englishFeedbackController = createFeedbackStoryController();
const englishTerminalLaunchProfiles = createTerminalLaunchProfileStoryControl(true);
const englishInteractionController = createInteractionStoryController();
const englishTerminalDefaults = createTerminalDefaultSettingsStoryControl();
const englishAgentIntegrations = createAgentIntegrationStoryControl();
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
const chineseInteractionController = createInteractionStoryController();
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
					'Inspect the grouped Application, Terminal, Workspace, Integrations, and Data & Access rail; inspect live padding, weights, ligatures, copy-on-select, and the Desktop scrollback default with its future-terminal notice. Open Profiles to manage the local system and Fish launches, literal ordered arguments, future-terminal boundary, and confirmed destructive actions. Type font to see global Text results without blanking General, then activate Terminal font family to restore the rail and reveal the row. Open Keyboard shortcuts to search localized action copy or current Apple chords and use Record keys as an exact, discovery-only lookup. Neutral setting groups have no outer stroke, future controls are clearly marked Planned and disabled, Shell integration explains automatic Desktop scope and exact manual fallbacks, Remote development remains read-only, and Ghostty migration remains a paste-only dry-run.',
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
		agentIntegrationManagementControl: chineseAgentIntegrations,
		controller: chineseController,
		keyboardShortcutController: chineseKeyboardController,
		keyboardShortcutPlatform: 'standard',
		onRequestClose: () => chineseController.close(),
		requestBrowserAccess: () => undefined,
		terminalFeedbackSettingsController: chineseFeedbackController,
		terminalLaunchProfileClient: chineseTerminalLaunchProfiles,
		terminalInteractionSettingsController: chineseInteractionController,
		terminalDefaultSettingsControl: chineseTerminalDefaults,
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

function createInteractionStoryController(): TerminalInteractionSettingsController {
	return new TerminalInteractionSettingsController({
		getItem: () => null,
		setItem: () => undefined,
	});
}

function createTerminalDefaultSettingsStoryControl(
	scrollbackBytes = 10_000_000,
): DesktopTerminalDefaultSettingsControl {
	let snapshot: DesktopTerminalDefaultSettingsClientSnapshot = {
		...DesktopTerminalDefaultSettingsSnapshotSchema.parse({
			configuration: { scrollbackBytes },
			persistenceIssue: null,
			source: 'stored',
		}),
		failureCode: null,
		pending: false,
	};
	const listeners = new Set<() => void>();
	return {
		getSnapshot: () => snapshot,
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		update: (configuration) => {
			snapshot = { ...snapshot, configuration, persistenceIssue: null, source: 'stored' };
			for (const listener of listeners) listener();
			return Promise.resolve({ kind: 'completed', snapshot });
		},
	};
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

function createAgentIntegrationStoryControl(): AgentIntegrationManagementControl {
	let snapshot: AgentIntegrationManagementClientSnapshot = createAgentIntegrationStorySnapshot();
	const listeners = new Set<() => void>();
	const update = (integrationId: DesktopAgentIntegrationId, enabled: boolean) => {
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
			snapshot: { integrations: snapshot.integrations, revision: snapshot.revision },
		});
	};
	return {
		disable: (integrationId) => update(integrationId, false),
		enable: (integrationId) => update(integrationId, true),
		getSnapshot: () => snapshot,
		refresh: () =>
			Promise.resolve({
				kind: 'completed' as const,
				snapshot: { integrations: snapshot.integrations, revision: snapshot.revision },
			}),
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

function createAgentIntegrationStorySnapshot(): AgentIntegrationManagementClientSnapshot {
	const examples: Partial<
		Readonly<Record<DesktopAgentIntegrationId, Partial<DesktopAgentIntegrationSnapshotItem>>>
	> = {
		'claude-code': { availability: 'available', state: 'enabled' },
		codex: { availability: 'available', state: 'not-installed' },
		grok: { availability: 'available', state: 'disabled' },
		opencode: { availability: 'available', state: 'enabled' },
	};
	return {
		failureCode: null,
		failureIntegrationId: null,
		integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
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
