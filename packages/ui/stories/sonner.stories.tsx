import { Button, Toaster } from '@orchbee/ui/components';
import { toast } from 'sonner';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Feedback/Sonner',
	component: Toaster,
} satisfies Meta<typeof Toaster>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SuccessToast: Story = {
	render: () => (
		<>
			<Button onClick={() => toast.success('Agent session started')}>Show toast</Button>
			<Toaster />
		</>
	),
};
