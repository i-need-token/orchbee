import {
	Bubble,
	BubbleContent,
	Message,
	MessageAvatar,
	MessageContent,
	MessageFooter,
	MessageGroup,
	MessageHeader,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'AI Interface/Message',
	component: Message,
} satisfies Meta<typeof Message>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Conversation: Story = {
	render: () => (
		<MessageGroup>
			<Message>
				<MessageAvatar>AI</MessageAvatar>
				<MessageContent>
					<MessageHeader>Orchbee Agent</MessageHeader>
					<Bubble variant='muted'>
						<BubbleContent>I inspected the UI package exports.</BubbleContent>
					</Bubble>
					<MessageFooter>Completed</MessageFooter>
				</MessageContent>
			</Message>
			<Message align='end'>
				<MessageContent>
					<Bubble align='end'>
						<BubbleContent>Show me the component catalog.</BubbleContent>
					</Bubble>
				</MessageContent>
			</Message>
		</MessageGroup>
	),
};
