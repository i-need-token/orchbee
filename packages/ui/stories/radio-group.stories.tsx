import { Label, RadioGroup, RadioGroupItem } from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Forms/Radio Group',
	component: RadioGroup,
} satisfies Meta<typeof RadioGroup>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<RadioGroup defaultValue='balanced' aria-label='Agent autonomy'>
			<label>
				<RadioGroupItem value='guided' />
				<Label>Guided</Label>
			</label>
			<label>
				<RadioGroupItem value='balanced' />
				<Label>Balanced</Label>
			</label>
			<label>
				<RadioGroupItem value='autonomous' />
				<Label>Autonomous</Label>
			</label>
		</RadioGroup>
	),
};
