import {
	Button,
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Overlays/Tooltip',
	component: Tooltip,
} satisfies Meta<typeof Tooltip>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<TooltipProvider>
			<Tooltip>
				<TooltipTrigger render={<Button variant='outline' />}>Hover for details</TooltipTrigger>
				<TooltipContent>Open the current session in a new pane.</TooltipContent>
			</Tooltip>
		</TooltipProvider>
	),
};
