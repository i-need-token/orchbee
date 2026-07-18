import { Button, DirectionProvider } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Utilities/Direction',
	component: DirectionProvider,
} satisfies Meta<typeof DirectionProvider>;

export default meta;

type Story = StoryObj<typeof meta>;

export const RightToLeft: Story = {
	render: () => (
		<DirectionProvider direction='rtl'>
			<Button>اتصال الوكيل</Button>
		</DirectionProvider>
	),
};
