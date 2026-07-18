import {
	Button,
	Empty,
	EmptyContent,
	EmptyDescription,
	EmptyHeader,
	EmptyMedia,
	EmptyTitle,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Feedback/Empty',
	component: Empty,
} satisfies Meta<typeof Empty>;

export default meta;

type Story = StoryObj<typeof meta>;

export const NoSessions: Story = {
	render: () => (
		<Empty>
			<EmptyHeader>
				<EmptyMedia variant='icon'>⌘</EmptyMedia>
				<EmptyTitle>No sessions yet</EmptyTitle>
				<EmptyDescription>Create a session to start an agent workspace.</EmptyDescription>
			</EmptyHeader>
			<EmptyContent>
				<Button>New session</Button>
			</EmptyContent>
		</Empty>
	),
};
