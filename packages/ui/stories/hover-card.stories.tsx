import { HoverCard, HoverCardContent, HoverCardTrigger } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Overlays/Hover Card',
	component: HoverCard,
} satisfies Meta<typeof HoverCard>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<HoverCard>
			<HoverCardTrigger href='#'>orchbee/ui</HoverCardTrigger>
			<HoverCardContent>Reusable shadcn components backed by Base UI.</HoverCardContent>
		</HoverCard>
	),
};
