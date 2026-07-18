import { Button, ButtonGroup, ButtonGroupSeparator, ButtonGroupText } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Actions/Button Group',
	component: ButtonGroup,
} satisfies Meta<typeof ButtonGroup>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<ButtonGroup>
			<ButtonGroupText>View</ButtonGroupText>
			<ButtonGroupSeparator />
			<Button variant='outline'>Terminal</Button>
			<Button variant='outline'>Changes</Button>
		</ButtonGroup>
	),
};
