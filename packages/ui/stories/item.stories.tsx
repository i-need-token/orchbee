import {
	Badge,
	Button,
	Item,
	ItemActions,
	ItemContent,
	ItemDescription,
	ItemGroup,
	ItemMedia,
	ItemTitle,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Data Display/Item',
	component: Item,
} satisfies Meta<typeof Item>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SessionList: Story = {
	render: () => (
		<ItemGroup>
			<Item>
				<ItemMedia variant='icon'>⌘</ItemMedia>
				<ItemContent>
					<ItemTitle>Local agent</ItemTitle>
					<ItemDescription>Running in the current workspace.</ItemDescription>
				</ItemContent>
				<ItemActions>
					<Badge variant='secondary'>Active</Badge>
					<Button size='sm' variant='outline'>
						Open
					</Button>
				</ItemActions>
			</Item>
		</ItemGroup>
	),
};
