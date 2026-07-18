import {
	Button,
	Collapsible,
	CollapsibleContent,
	CollapsibleTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Collapsible',
	component: Collapsible,
} satisfies Meta<typeof Collapsible>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Collapsible defaultOpen>
			<CollapsibleTrigger render={<Button variant='outline' />}>Toggle details</CollapsibleTrigger>
			<CollapsibleContent>Agent output and execution metadata.</CollapsibleContent>
		</Collapsible>
	),
};
