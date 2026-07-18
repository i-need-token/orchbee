import { Textarea } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Textarea',
	component: Textarea,
	args: {
		'aria-label': 'Agent instructions',
		placeholder: 'Describe the task for the agent…',
	},
} satisfies Meta<typeof Textarea>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};
