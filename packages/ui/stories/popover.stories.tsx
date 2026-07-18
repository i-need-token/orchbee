import {
	Button,
	Popover,
	PopoverContent,
	PopoverDescription,
	PopoverHeader,
	PopoverTitle,
	PopoverTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Overlays/Popover',
	component: Popover,
} satisfies Meta<typeof Popover>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Popover>
			<PopoverTrigger render={<Button variant='outline' />}>Connection</PopoverTrigger>
			<PopoverContent>
				<PopoverHeader>
					<PopoverTitle>Local runtime</PopoverTitle>
					<PopoverDescription>Connected with 24 ms latency.</PopoverDescription>
				</PopoverHeader>
			</PopoverContent>
		</Popover>
	),
};
