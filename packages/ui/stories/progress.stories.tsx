import { Progress, ProgressLabel, ProgressValue } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Feedback/Progress',
	component: Progress,
} satisfies Meta<typeof Progress>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	args: {
		value: 68,
	},
	render: () => (
		<Progress value={68}>
			<ProgressLabel>Repository scan</ProgressLabel>
			<ProgressValue />
		</Progress>
	),
};
