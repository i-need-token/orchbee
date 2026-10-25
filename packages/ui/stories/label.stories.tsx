import { Input } from '@orchbee/ui/components';
import { Label } from '../src/components/ui/label';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Label',
	component: Label,
} satisfies Meta<typeof Label>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<>
			<Label htmlFor='label-story'>Workspace</Label>
			<Input id='label-story' defaultValue='/Users/orchbee' />
		</>
	),
};
