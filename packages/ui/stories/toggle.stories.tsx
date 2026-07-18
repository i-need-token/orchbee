import { Toggle } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Actions/Toggle',
	component: Toggle,
	args: {
		children: 'Wrap output',
	},
} satisfies Meta<typeof Toggle>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Pressed: Story = {
	args: {
		defaultPressed: true,
		variant: 'outline',
	},
};
