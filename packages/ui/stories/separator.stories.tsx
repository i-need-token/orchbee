import { Separator } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Separator',
	component: Separator,
} satisfies Meta<typeof Separator>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Horizontal: Story = {
	render: () => (
		<div style={{ width: 360 }}>
			<p>Terminal output</p>
			<Separator />
			<p>Agent activity</p>
		</div>
	),
};
