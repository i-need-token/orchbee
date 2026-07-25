import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { Workbench } from '@orchbee/workbench';
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

export const EmptyPanels: Story = {
	args: {
		onSearch: () => undefined,
		onSettings: () => undefined,
		onNewTerminal: () => undefined,
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
		onSearch: () => undefined,
		onSettings: () => undefined,
		onNewTerminal: () => undefined,
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
		layoutStorage: {
			getItem: (key) =>
				key === 'orchbee:workbench:left-panel-visibility:v1' ? '{"visible":false}' : null,
			setItem: () => undefined,
		},
		onSearch: () => undefined,
		onSettings: () => undefined,
		onNewTerminal: () => undefined,
		shortcutPlatform: 'apple',
		tabModel: createTabModel(['Terminal 1', 'Terminal 2', 'Terminal 3']),
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
