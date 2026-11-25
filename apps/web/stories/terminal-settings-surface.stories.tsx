import {
	UserLaunchProfileIdSchema,
	type UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';
import * as React from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { TerminalSettingsController } from '../src/terminal-settings';
import { KeyboardShortcutSettingsController } from '../src/keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../src/macos-terminal-input-settings';
import { TerminalFeedbackSettingsController } from '../src/terminal-feedback-settings';
import {
	ColorThemeQuickPick,
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

/** The Server catalog's names for Launch options on the Agents page: a few installed Agents. */
const storyAgentSnapshot = {
	agents: [
		{ command: 'claude', productName: 'Claude Code', providerId: 'claude-code', state: 'found' },
		{ command: 'codex', productName: 'Codex', providerId: 'codex', state: 'found' },
		{ command: 'gemini', productName: 'Gemini CLI', providerId: 'gemini', state: 'found' },
		{ command: 'opencode', productName: 'OpenCode', providerId: 'opencode', state: 'found' },
		{ command: 'pi', productName: 'Pi', providerId: 'pi', state: 'found' },
	],
} as const;
const storyAgentCatalog: React.ComponentProps<typeof TerminalSettingsSurface>['agentCatalog'] = {
	getSnapshot: () => storyAgentSnapshot,
	subscribe: () => () => undefined,
};

const englishArgs = {
	agentCatalog: storyAgentCatalog,
	about: createAboutStorySource({ kind: 'idle' }),
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
	agentCatalog: storyAgentCatalog,
	about: createAboutStorySource({ kind: 'idle' }),
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

function colorThemeStory(mode: 'dark' | 'light', picker: 'dark' | 'light' | null): Story {
	return {
		args: { ...englishArgs, initialCategory: 'themes' },
		play: async ({ canvasElement }) => {
			if (picker === null) return;
			const view = canvasElement.ownerDocument.defaultView;
			if (view === null) throw new Error('Expected the Story to render in a window.');
			await new Promise((resolve) => view.requestAnimationFrame(resolve));
			// Open the list through its Choose theme button, as a person does, so it mounts closed.
			canvasElement.ownerDocument
				.querySelector<HTMLButtonElement>(`[data-color-theme-trigger='${picker}']`)
				?.click();
		},
		render: () => <ColorThemeStory mode={mode} />,
	};
}

/**
 * The Themes page over a window whose theme the controller drives, with the quick pick a Themes
 * button opens. The document element carries the theme, so a preview repaints the whole page.
 */
function ColorThemeStory({ mode }: { readonly mode: 'dark' | 'light' }): React.ReactNode {
	const [story] = React.useState(() => createColorThemeStory(mode));
	React.useEffect(() => story.release, [story]);
	return (
		<ApplicationLocaleProvider source={englishApplicationLocaleController}>
			<TerminalSettingsSurface
				{...englishArgs}
				controller={story.controller}
				initialCategory='themes'
				onRequestClose={() => story.controller.close()}
			/>
			<ColorThemeQuickPick controller={story.controller} />
		</ApplicationLocaleProvider>
	);
}

interface ColorThemeStoryState {
	readonly controller: TerminalSettingsController;
	/** Disposes the controller and puts back the document's theme attributes it changed. */
	readonly release: () => void;
}

function createColorThemeStory(mode: 'dark' | 'light'): ColorThemeStoryState {
	const { dataset } = document.documentElement;
	const saved = { colorTheme: dataset['colorTheme'], theme: dataset['theme'] };
	const controller = new TerminalSettingsController({
		themeTarget: document.documentElement,
		userSettings: memoryUserSettings({
			filePath: '/Users/person/.orchbee/config.toml',
			values: { 'appearance.mode': mode, 'general.language': 'en-US' },
		}),
	});
	controller.open();
	return {
		controller,
		release: () => {
			controller.dispose();
			if (saved.colorTheme === undefined) delete dataset['colorTheme'];
			else dataset['colorTheme'] = saved.colorTheme;
			if (saved.theme === undefined) delete dataset['theme'];
			else dataset['theme'] = saved.theme;
		},
	};
}

export const EnglishColorThemeLight: Story = colorThemeStory('light', null);
export const EnglishColorThemeDark: Story = colorThemeStory('dark', null);
export const EnglishColorThemeLightList: Story = colorThemeStory('light', 'light');
export const EnglishColorThemeDarkList: Story = colorThemeStory('dark', 'dark');

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
export const EnglishMachines: Story = {
	...englishLight('machines'),
	args: {
		...englishArgs,
		initialCategory: 'machines',
		machines: createMachinesStorySource('en-US'),
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
export const ChineseMachines: Story = {
	...chineseDark('machines'),
	args: {
		...chineseArgs,
		initialCategory: 'machines',
		machines: createMachinesStorySource('zh-CN'),
	},
};
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
	const offered = offer === null ? null : ({ kind: 'client', version: offer } as const);
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
		offer: {
			getSnapshot: () => offered,
			subscribe: () => () => undefined,
		},
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
			capabilities: {
				installedShells: [
					{ arguments: [], executable: '/bin/zsh', kind: 'named', name: 'zsh' },
					{ arguments: [], executable: '/bin/bash', kind: 'named', name: 'bash' },
					{ arguments: [], executable: '/opt/homebrew/bin/fish', kind: 'named', name: 'fish' },
				],
				loginShell,
			},
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
		createProfile: (name, command = { kind: 'system-shell', login: false }) => {
			const id = UserLaunchProfileIdSchema.parse(`story-profile-${++profileSequence}`);
			return commit({
				...snapshot.catalog,
				profiles: [...snapshot.catalog.profiles, { command, id, name }],
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

/**
 * Saved machines as the machines process words them: one connected, one waiting for a login, and one
 * whose terminals are being stopped. Requests change nothing.
 */
function createMachinesStorySource(
	locale: 'en-US' | 'zh-CN',
): React.ComponentProps<typeof TerminalSettingsSurface>['machines'] {
	const english = locale === 'en-US';
	const capacity = (summary: string, long: string) => ({
		long,
		parts: summary.split(' · ').map((text) => ({ low: false, text })),
		summary,
	});
	const unread = { long: english ? 'Not read yet.' : '尚未读取。', parts: [], summary: '—' };
	const machine = {
		agents: 0,
		lines: [],
		terminals: 0,
		version: '0.1.0-beta.23',
		workspaces: 0,
	};
	const view = {
		add: {
			form: null,
			labels: {
				add: 'Add',
				addMachine: 'Add machine…',
				address: 'Address',
				cancel: 'Cancel',
				hosts: 'Hosts in SSH config',
				title: 'Add machine',
			},
		},
		added: null,
		catalog: 'read' as const,
		hub: null,
		local: capacity(
			'62% · 9.4 GB · 120 GB',
			english
				? '62% of the processor, 9.4 GB of memory and 120 GB of disk are free.'
				: '处理器空闲 62%，内存空闲 9.4 GB，磁盘空闲 120 GB。',
		),
		machines: [
			{
				...machine,
				action: { act: 'disconnect' as const, label: english ? 'Disconnect' : '断开' },
				address: 'dev@10.0.0.8',
				agents: 2,
				capacity: capacity(
					'48% · 21 GB · 310 GB',
					english
						? '48% of the processor, 21 GB of memory and 310 GB of disk are free.'
						: '处理器空闲 48%，内存空闲 21 GB，磁盘空闲 310 GB。',
				),
				color: 'cyan' as const,
				label: 'devbox',
				machine: '0123456789abcdef0123456789abcdef',
				notConnected: english ? 'devbox is not connected.' : 'devbox 没有连接。',
				card: { lines: [], notNow: english ? 'Not now' : '暂不', title: '' },
				removeDescription: english
					? 'Its 3 Workspaces leave the sidebar; their terminals keep running.'
					: '它的 3 个工作区会从侧边栏消失，上面的终端继续运行。',
				state: 'online',
				status: english ? 'Connected.' : '已连接。',
				stopConfirmation: english
					? 'Stops 5 terminals and 2 Agents on devbox.'
					: '将停止 devbox 上的 5 个终端和 2 个智能体。',
				stopping: false,
				terminals: 5,
				word: english ? 'Online' : '在线',
				workspaces: 3,
			},
			{
				...machine,
				action: { act: 'log-in' as const, label: english ? 'Log in' : '登录' },
				address: 'me@mac-mini',
				capacity: unread,
				color: 'purple' as const,
				label: 'mac-mini',
				machine: '11111111111111111111111111111111',
				notConnected: english ? 'mac-mini is not connected.' : 'mac-mini 没有连接。',
				card: { lines: [], notNow: english ? 'Not now' : '暂不', title: '' },
				removeDescription: english ? 'Nothing runs on it from here.' : '这里没有东西运行在它上面。',
				state: 'needs-login',
				status: english ? 'ssh needs your password.' : 'ssh 需要你的密码。',
				stopConfirmation: '',
				stopping: false,
				word: english ? 'Needs login' : '需要登录',
			},
			{
				...machine,
				action: null,
				address: 'admin@nas.local',
				capacity: unread,
				color: 'grey' as const,
				label: 'nas',
				machine: '22222222222222222222222222222222',
				notConnected: english ? 'nas is not connected.' : 'nas 没有连接。',
				card: { lines: [], notNow: english ? 'Not now' : '暂不', title: '' },
				removeDescription: english ? 'Nothing runs on it from here.' : '这里没有东西运行在它上面。',
				state: 'online',
				status: english ? 'Connected.' : '已连接。',
				stopConfirmation: '',
				stopping: true,
				word: english ? 'Online' : '在线',
			},
		],
		login: null,
		offer: null,
		notice: null,
		passwords: true,
		seen: [],
		change: {
			labels: {
				find: 'Find a machine',
				heading: 'Runs on',
				here: 'This computer',
				manage: 'Manage machines…',
				noMatch: 'No machine matches',
				note: 'free CPU · memory · disk',
				title: 'Change machine',
			},
			rows: [],
		},
		move: { moved: null, notice: null, pending: null, question: null },
	};
	return {
		getSnapshot: () => view,
		request: () => undefined,
		showCapacity: () => () => undefined,
		subscribe: () => () => undefined,
	};
}
