import { Skeleton } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Feedback/Skeleton',
	component: Skeleton,
} satisfies Meta<typeof Skeleton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<div>
			<Skeleton style={{ width: 320, height: 20 }} />
			<Skeleton style={{ width: 220, height: 16 }} />
		</div>
	),
};
