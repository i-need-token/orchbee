import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Select',
	component: Select,
} satisfies Meta<typeof Select>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Select defaultValue='local'>
			<SelectTrigger aria-label='Runtime'>
				<SelectValue />
			</SelectTrigger>
			<SelectContent>
				<SelectItem value='local'>Local</SelectItem>
				<SelectItem value='remote'>Remote</SelectItem>
				<SelectItem value='sandbox'>Sandbox</SelectItem>
			</SelectContent>
		</Select>
	),
};
