import { Label, Switch } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Switch',
	component: Switch,
} satisfies Meta<typeof Switch>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<label>
			<Switch defaultChecked />
			<Label>Notify when the agent finishes</Label>
		</label>
	),
};
