import { ToggleGroup, ToggleGroupItem } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Actions/Toggle Group',
	component: ToggleGroup,
} satisfies Meta<typeof ToggleGroup>;

export default meta;

type Story = StoryObj<typeof meta>;

export const ViewOptions: Story = {
	render: () => (
		<ToggleGroup defaultValue={['terminal']} variant='outline' spacing={0}>
			<ToggleGroupItem value='terminal'>Terminal</ToggleGroupItem>
			<ToggleGroupItem value='activity'>Activity</ToggleGroupItem>
			<ToggleGroupItem value='files'>Files</ToggleGroupItem>
		</ToggleGroup>
	),
};
