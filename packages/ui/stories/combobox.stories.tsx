import {
	Combobox,
	ComboboxContent,
	ComboboxEmpty,
	ComboboxInput,
	ComboboxItem,
	ComboboxList,
} from '@orchbee/ui/components';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const options = ['Local', 'Development', 'Production'];

const meta = {
	title: 'Forms/Combobox',
	component: Combobox,
} satisfies Meta<typeof Combobox>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
	render: () => (
		<Combobox items={options} defaultValue='Development'>
			<ComboboxInput placeholder='Select an environment' />
			<ComboboxContent>
				<ComboboxEmpty>No environment found.</ComboboxEmpty>
				<ComboboxList>
					{options.map((option) => (
						<ComboboxItem key={option} value={option}>
							{option}
						</ComboboxItem>
					))}
				</ComboboxList>
			</ComboboxContent>
		</Combobox>
	),
};
