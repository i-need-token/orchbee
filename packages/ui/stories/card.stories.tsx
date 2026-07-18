import {
	Button,
	Card,
	CardAction,
	CardContent,
	CardDescription,
	CardFooter,
	CardHeader,
	CardTitle,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Data Display/Card',
	component: Card,
} satisfies Meta<typeof Card>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Card>
			<CardHeader>
				<CardTitle>Agent session</CardTitle>
				<CardDescription>Connected to the local runtime.</CardDescription>
				<CardAction>
					<Button variant='outline' size='sm'>
						Open
					</Button>
				</CardAction>
			</CardHeader>
			<CardContent>Three agents are active.</CardContent>
			<CardFooter>Updated just now</CardFooter>
		</Card>
	),
};
