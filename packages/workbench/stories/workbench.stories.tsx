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

export const EmptyPanels: Story = {};
