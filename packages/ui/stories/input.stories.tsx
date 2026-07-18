import { Input } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Input',
	component: Input,
	args: {
		placeholder: 'Enter a session name',
	},
} satisfies Meta<typeof Input>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Invalid: Story = {
	args: {
		'aria-invalid': true,
		defaultValue: 'Invalid value',
	},
};
