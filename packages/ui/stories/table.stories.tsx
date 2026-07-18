import {
	Table,
	TableBody,
	TableCaption,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Data Display/Table',
	component: Table,
} satisfies Meta<typeof Table>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Sessions: Story = {
	render: () => (
		<Table>
			<TableCaption>Recent agent sessions</TableCaption>
			<TableHeader>
				<TableRow>
					<TableHead>Agent</TableHead>
					<TableHead>Status</TableHead>
					<TableHead>Runtime</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				<TableRow>
					<TableCell>Codex</TableCell>
					<TableCell>Running</TableCell>
					<TableCell>Local</TableCell>
				</TableRow>
				<TableRow>
					<TableCell>Claude</TableCell>
					<TableCell>Idle</TableCell>
					<TableCell>Remote</TableCell>
				</TableRow>
			</TableBody>
		</Table>
	),
};
