import { Bubble, BubbleContent, BubbleGroup, BubbleReactions } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'AI Interface/Bubble',
	component: Bubble,
} satisfies Meta<typeof Bubble>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Conversation: Story = {
	render: () => (
		<BubbleGroup>
			<Bubble variant='muted'>
				<BubbleContent>Inspect the current repository state.</BubbleContent>
			</Bubble>
			<Bubble variant='default' align='end'>
				<BubbleContent>The UI package is ready for visual review.</BubbleContent>
				<BubbleReactions>✓</BubbleReactions>
			</Bubble>
			<Bubble variant='destructive'>
				<BubbleContent>The command failed with exit code 1.</BubbleContent>
			</Bubble>
		</BubbleGroup>
	),
};
