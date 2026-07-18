import { Badge } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Data Display/Badge',
	component: Badge,
	args: {
		children: 'Connected',
	},
} satisfies Meta<typeof Badge>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Variants: Story = {
	render: () => (
		<>
			<Badge>Default</Badge>
			<Badge variant='secondary'>Secondary</Badge>
			<Badge variant='outline'>Outline</Badge>
			<Badge variant='destructive'>Destructive</Badge>
		</>
	),
};
