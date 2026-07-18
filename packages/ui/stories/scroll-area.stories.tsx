import { ScrollArea } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Scroll Area',
	component: ScrollArea,
} satisfies Meta<typeof ScrollArea>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SessionLog: Story = {
	render: () => (
		<ScrollArea style={{ width: 360, height: 220 }}>
			{Array.from({ length: 24 }, (_, index) => (
				<p key={index}>Agent event {index + 1}: command completed successfully.</p>
			))}
		</ScrollArea>
	),
};
