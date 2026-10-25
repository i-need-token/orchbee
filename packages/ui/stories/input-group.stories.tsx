import {
	InputGroup,
	InputGroupAddon,
	InputGroupButton,
	InputGroupInput,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Input Group',
	component: InputGroup,
} satisfies Meta<typeof InputGroup>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Command: Story = {
	render: () => (
		<InputGroup>
			<InputGroupAddon>$</InputGroupAddon>
			<InputGroupInput defaultValue='pnpm verify' aria-label='Command' />
			<InputGroupAddon align='inline-end'>
				<InputGroupButton>Run</InputGroupButton>
			</InputGroupAddon>
		</InputGroup>
	),
};
