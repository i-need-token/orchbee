import {
	Button,
	Sheet,
	SheetContent,
	SheetDescription,
	SheetHeader,
	SheetTitle,
	SheetTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Overlays/Sheet',
	component: Sheet,
} satisfies Meta<typeof Sheet>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Sheet>
			<SheetTrigger render={<Button />}>Open session details</SheetTrigger>
			<SheetContent>
				<SheetHeader>
					<SheetTitle>Session details</SheetTitle>
					<SheetDescription>Inspect the active agent and its runtime.</SheetDescription>
				</SheetHeader>
			</SheetContent>
		</Sheet>
	),
};
