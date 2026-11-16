import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const RUNTIMES = [
	{ label: 'Local', value: 'local' },
	{ label: 'Remote', value: 'remote' },
] as const;

const meta = {
	title: 'Forms/Select',
	component: Select,
} satisfies Meta<typeof Select>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Select defaultValue='local' items={RUNTIMES}>
			<SelectTrigger aria-label='Runtime' size='sm'>
				<SelectValue />
			</SelectTrigger>
			<SelectContent>
				{RUNTIMES.map((runtime) => (
					<SelectItem key={runtime.value} value={runtime.value}>
						{runtime.label}
					</SelectItem>
				))}
			</SelectContent>
		</Select>
	),
};

export const Open: Story = {
	render: () => (
		<Select defaultOpen defaultValue='local' items={RUNTIMES}>
			<SelectTrigger aria-label='Runtime' size='sm'>
				<SelectValue />
			</SelectTrigger>
			<SelectContent>
				{RUNTIMES.map((runtime) => (
					<SelectItem key={runtime.value} value={runtime.value}>
						{runtime.label}
					</SelectItem>
				))}
			</SelectContent>
		</Select>
	),
};
