import {
	Tabs,
	TabsBar,
	TabsContent,
	TabsDivider,
	TabsItem,
	TabsList,
	TabsTrigger,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Navigation/Tabs',
	component: Tabs,
} satisfies Meta<typeof Tabs>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Tabs defaultValue='terminal' style={{ width: 420 }}>
			<TabsList>
				<TabsTrigger value='terminal'>Terminal</TabsTrigger>
				<TabsTrigger value='activity'>Activity</TabsTrigger>
			</TabsList>
			<TabsContent value='terminal'>Live terminal output appears here.</TabsContent>
			<TabsContent value='activity'>Agent activity appears here.</TabsContent>
		</Tabs>
	),
};

export const Workspace: Story = {
	render: () => (
		<Tabs defaultValue='terminal' variant='workspace' style={{ height: 320, width: 960 }}>
			<TabsBar>
				<TabsList aria-label='Workspace tabs' variant='workspace'>
					<TabsItem>
						<TabsTrigger value='terminal'>Terminal</TabsTrigger>
					</TabsItem>
					<TabsDivider />
					<TabsItem>
						<TabsTrigger value='activity'>Agent activity</TabsTrigger>
					</TabsItem>
					<TabsDivider />
					<TabsItem>
						<TabsTrigger value='repository'>Repository changes</TabsTrigger>
					</TabsItem>
				</TabsList>
			</TabsBar>
			<TabsContent value='terminal' />
			<TabsContent value='activity' />
			<TabsContent value='repository' />
		</Tabs>
	),
};
