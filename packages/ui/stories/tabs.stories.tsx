import { Tabs, TabsContent, TabsList, TabsTrigger } from '@orchbee/ui/components';
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
