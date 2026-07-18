import { Calendar } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Data Display/Calendar',
	component: Calendar,
} satisfies Meta<typeof Calendar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SingleSelection: Story = {
	args: {
		mode: 'single',
		defaultMonth: new Date(2026, 6, 1),
		selected: new Date(2026, 6, 17),
	},
};
