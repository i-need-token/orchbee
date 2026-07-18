import { ChartContainer, ChartTooltip, ChartTooltipContent } from '@orchbee/ui/components';
import { Bar, BarChart, CartesianGrid, XAxis } from 'recharts';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const data = [
	{ day: 'Mon', sessions: 4 },
	{ day: 'Tue', sessions: 7 },
	{ day: 'Wed', sessions: 5 },
	{ day: 'Thu', sessions: 9 },
	{ day: 'Fri', sessions: 6 },
];

const config = {
	sessions: {
		label: 'Agent sessions',
		color: 'var(--primary)',
	},
};

const meta = {
	title: 'Data Display/Chart',
	component: ChartContainer,
} satisfies Meta<typeof ChartContainer>;

export default meta;

type Story = StoryObj<typeof meta>;

export const BarOverview: Story = {
	args: {
		config,
		children: (
			<BarChart data={data}>
				<CartesianGrid vertical={false} />
				<XAxis dataKey='day' tickLine={false} axisLine={false} />
				<ChartTooltip content={<ChartTooltipContent />} />
				<Bar dataKey='sessions' fill='var(--color-sessions)' radius={4} />
			</BarChart>
		),
		style: { width: 480, height: 240 },
	},
};
