import {
	MessageScroller,
	MessageScrollerButton,
	MessageScrollerContent,
	MessageScrollerItem,
	MessageScrollerProvider,
	MessageScrollerViewport,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'AI Interface/Message Scroller',
	component: MessageScroller,
	parameters: {
		layout: 'padded',
	},
} satisfies Meta<typeof MessageScroller>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<MessageScrollerProvider>
			<MessageScroller style={{ height: 240, width: 360 }}>
				<MessageScrollerViewport>
					<MessageScrollerContent>
						{Array.from({ length: 8 }, (_value, index) => (
							<MessageScrollerItem key={index} scrollAnchor={index === 7}>
								Message {index + 1}: deterministic agent output.
							</MessageScrollerItem>
						))}
					</MessageScrollerContent>
				</MessageScrollerViewport>
				<MessageScrollerButton />
			</MessageScroller>
		</MessageScrollerProvider>
	),
};
