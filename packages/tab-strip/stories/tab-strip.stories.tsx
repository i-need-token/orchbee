import { TabModel, TabStrip, TabStripOrientation } from '@orchbee/tab-strip';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const model = new TabModel({
	groups: [
		{ id: 'research', title: 'Research', color: 'blue', collapsed: false },
		{ id: 'delivery', title: 'Delivery', color: 'green', collapsed: false },
	],
	tabs: [
		{
			id: 'overview',
			title: 'Project overview',
			url: 'https://orchbee.dev/overview',
			pinned: true,
			group: null,
		},
		{
			id: 'architecture',
			title: 'Architecture',
			url: 'https://orchbee.dev/architecture',
			pinned: false,
			group: 'research',
		},
		{
			id: 'terminal',
			title: 'Terminal session',
			url: 'https://orchbee.dev/terminal',
			pinned: false,
			group: 'research',
			alertState: 'audio-playing',
		},
		{
			id: 'release',
			title: 'Release checklist',
			url: 'https://orchbee.dev/release',
			pinned: false,
			group: 'delivery',
		},
	],
	activeTabId: 'terminal',
	selectedTabIds: ['terminal'],
});

const compactControlsModel = new TabModel({
	tabs: [
		{
			id: 'terminal-one',
			title: 'Terminal one',
			url: 'https://orchbee.dev/terminal/one',
			pinned: false,
			group: null,
		},
		{
			id: 'terminal-two',
			title: 'Terminal two',
			url: 'https://orchbee.dev/terminal/two',
			pinned: false,
			group: null,
		},
	],
	activeTabId: 'terminal-two',
	selectedTabIds: ['terminal-two'],
});

const meta = {
	title: 'TabStrip',
	component: TabStrip,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof TabStrip>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	args: {
		model,
		orientation: TabStripOrientation.Horizontal,
		ariaLabel: 'Browser tabs',
		showNewTabButton: true,
	},
};

export const CompactControls: Story = {
	args: {
		model: compactControlsModel,
		orientation: TabStripOrientation.Horizontal,
		ariaLabel: 'Terminal tabs',
		showNewTabButton: true,
	},
};
