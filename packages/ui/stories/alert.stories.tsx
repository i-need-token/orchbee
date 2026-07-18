import { Alert, AlertDescription, AlertTitle } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Feedback/Alert',
	component: Alert,
} satisfies Meta<typeof Alert>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Alert>
			<AlertTitle>Agent connected</AlertTitle>
			<AlertDescription>The session is ready to receive commands.</AlertDescription>
		</Alert>
	),
};

export const Destructive: Story = {
	render: () => (
		<Alert variant='destructive'>
			<AlertTitle>Connection lost</AlertTitle>
			<AlertDescription>Reconnect before sending another command.</AlertDescription>
		</Alert>
	),
};
