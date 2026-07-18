import {
	Accordion,
	AccordionContent,
	AccordionItem,
	AccordionTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Layout/Accordion',
	component: Accordion,
} satisfies Meta<typeof Accordion>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Accordion defaultValue={['item-1']}>
			<AccordionItem value='item-1'>
				<AccordionTrigger>What is Orchbee?</AccordionTrigger>
				<AccordionContent>
					A terminal for running and orchestrating AI agents across devices.
				</AccordionContent>
			</AccordionItem>
			<AccordionItem value='item-2'>
				<AccordionTrigger>Can sections be collapsed?</AccordionTrigger>
				<AccordionContent>Each item exposes its own interactive content region.</AccordionContent>
			</AccordionItem>
		</Accordion>
	),
};
