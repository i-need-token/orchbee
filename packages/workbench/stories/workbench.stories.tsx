import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import { TabModel } from '@orchbee/tab-strip';
import { Workbench } from '@orchbee/workbench';
import type { WorkbenchSearchProvider } from '@orchbee/workbench';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Workbench',
	component: Workbench,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof Workbench>;

export default meta;

type Story = StoryObj<typeof meta>;

const unavailableSearchProvider: WorkbenchSearchProvider = {
	filters: ['all', 'agents'],
	getDefaultSections: () => Promise.reject(new Error('Story provider unavailable')),
	id: 'remote-agents',
	label: 'Remote agents',
	priority: 20,
	resultLimit: 5,
	search: () => Promise.reject(new Error('Story provider unavailable')),
};

const createTabModel = (titles: readonly string[]) =>
	new TabModel({
		tabs: titles.map((title, index) => ({
			id: `terminal-${index + 1}`,
			title,
			pinned: false,
			group: null,
		})),
		activeTabId: 'terminal-1',
	});

export const PanelWorkspace: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		onNewTerminal: () => ({ kind: 'unchanged' }),
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['Terminal 1', 'Terminal 2', 'Terminal 3']),
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const SimplifiedChineseSidebar: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		onNewTerminal: () => ({ kind: 'unchanged' }),
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['终端 1', '终端 2', '终端 3']),
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const HiddenSidebar: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		layoutStorage: {
			getItem: (key) =>
				key === 'orchbee:workbench:left-panel-visibility:v1' ? '{"visible":false}' : null,
			setItem: () => undefined,
		},
		onSettings: () => ({ kind: 'unchanged' }),
		onNewTerminal: () => ({ kind: 'unchanged' }),
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['Terminal 1', 'Terminal 2', 'Terminal 3']),
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const UnifiedSearchAll: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		onNewTerminal: () => ({ kind: 'unchanged' }),
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['Terminal 1', 'Terminal 2', 'Terminal 3']),
	},
	play: () => {
		window.dispatchEvent(
			new KeyboardEvent('keydown', {
				code: 'KeyK',
				metaKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const UnifiedSearchActionsChinese: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		onNewTerminal: () => ({ kind: 'unchanged' }),
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['终端 1', '终端 2', '终端 3']),
	},
	play: () => {
		window.dispatchEvent(
			new KeyboardEvent('keydown', {
				code: 'KeyP',
				metaKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const UnifiedSearchProviderFailure: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		onNewTerminal: () => ({ kind: 'unchanged' }),
		searchProviders: [unavailableSearchProvider],
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['Terminal 1', 'Terminal 2', 'Terminal 3']),
	},
	play: () => {
		window.dispatchEvent(
			new KeyboardEvent('keydown', {
				code: 'KeyK',
				metaKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
