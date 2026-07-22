import { OrchbeeI18nProvider } from '@orchbee/i18n';
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

export const EmptyPanels: Story = {
	args: {
		onSearch: () => undefined,
		onSettings: () => undefined,
		onNewTerminal: () => undefined,
		shortcutPlatform: 'apple',
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
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
