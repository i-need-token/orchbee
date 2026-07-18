import { Checkbox, Label } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Checkbox',
	component: Checkbox,
} satisfies Meta<typeof Checkbox>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Checked: Story = {
	render: () => (
		<>
			<Checkbox id='checkbox-story' defaultChecked />
			<Label htmlFor='checkbox-story'>Allow agent commands</Label>
		</>
	),
};
